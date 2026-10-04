use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};
use serde::Serialize;

use crate::cpx::{CPX_BLOCK_ALIGNMENT, encode_cpx_block};
use crate::cpx_patch::{FileRecordStoredLengthPatch, patch_file_record_stored_length};
use crate::disc_patch::{
    ComponentBuildMode, ComponentBuildOutput, IsoFileCandidate, SectorPatch, hash_sector_patches,
    plan_sector_patches, verify_iso_readback, write_mode2_cue, write_patched_bin,
};
use crate::executable::read_executable_symbol;
use crate::graphics_translation::raster::read_rgba_surface;
use crate::graphics_translation::{GraphicsTranslationStatus, load_graphics_translation_set};
use crate::indexed_texture::{
    colorize_psmt8, map_rgba_to_source_palette, parse_file_records, read_decoded_image,
    read_palette, sha256, swizzle_psmt8_uploaded_as_psmct32, unswizzle_psmt8_uploaded_as_psmct32,
    write_rgba_png,
};
use crate::rank_texture::render_rank_textures;
use crate::source::{DiscInventory, find_inventory_file};
use crate::texture_source::TextureSourceInputs;

const EXECUTABLE_ISO_PATH: &str = "/SLPM_624.00;1";
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_G.BIN;1";
const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image2";
const RANKING_SEMANTIC_ASSET_ID: &str = "ranking/interface-label-sheet";
const RANKING_IMAGE_FILE_RECORD: usize = 120;
const RANKING_PALETTE_FILE_RECORD: usize = 240;

pub struct RankingTranslationBuildInputs<'a> {
    pub inventory: &'a DiscInventory,
    pub source_bin_path: &'a Path,
    pub executable: &'a [u8],
    pub image_container: &'a [u8],
    pub translation_path: &'a Path,
    pub asset_path: &'a Path,
    pub output_bin_path: &'a Path,
    pub output_cue_path: &'a Path,
    pub preview_path: &'a Path,
}

#[derive(Clone, Debug, Serialize)]
pub struct RankingTranslationBuildReport {
    pub schema: u32,
    pub status: &'static str,
    pub release_candidate_eligible: bool,
    pub source_bin_sha256: String,
    pub output_bin_path: String,
    pub output_bin_byte_length: u64,
    pub output_bin_sha256: String,
    pub output_cue_path: String,
    pub preview_path: String,
    pub preview_png_sha256: String,
    pub image_file_record: usize,
    pub palette_file_record: usize,
    pub source_linear_indices_sha256: String,
    pub replacement_linear_indices_sha256: String,
    pub replacement_rgba_sha256: String,
    pub changed_rgba_pixels: usize,
    pub changed_palette_indices: usize,
    pub preserved_palette_indices: usize,
    pub protected_pixels: usize,
    pub changed_protected_pixels: usize,
    pub source_cpx_byte_length: usize,
    pub replacement_cpx_byte_length: usize,
    pub cpx_slot_byte_length: usize,
    pub cpx_padding_byte_length: usize,
    pub image_container_expected_writes: usize,
    pub executable_expected_writes: usize,
    pub raw_sector_plans: usize,
    pub raw_sector_data_writes: usize,
    pub modified_lbas: Vec<u32>,
    pub verification: RankingTranslationBuildVerification,
}

#[derive(Clone, Debug, Serialize)]
pub struct RankingTranslationBuildVerification {
    pub replacement_source: &'static str,
    pub consumer_rectangles: &'static str,
    pub palette_mapping: &'static str,
    pub cpx_roundtrip: &'static str,
    pub output_iso_readback: &'static str,
    pub protected_pixels: &'static str,
    pub human_review: &'static str,
    pub runtime_status: &'static str,
}

pub fn build_ranking_translation_development(
    inputs: RankingTranslationBuildInputs<'_>,
) -> Result<RankingTranslationBuildReport> {
    match build_ranking_translation(inputs, ComponentBuildMode::WriteArtifact)? {
        ComponentBuildOutput::Artifact(report) => Ok(report),
        ComponentBuildOutput::SectorPatches(_) => unreachable!("artifact mode returns a report"),
    }
}

pub(crate) fn plan_ranking_translation(
    inputs: RankingTranslationBuildInputs<'_>,
) -> Result<Vec<SectorPatch>> {
    match build_ranking_translation(inputs, ComponentBuildMode::ReturnSectorPatches)? {
        ComponentBuildOutput::Artifact(_) => {
            unreachable!("sector-patch mode returns planned sectors")
        }
        ComponentBuildOutput::SectorPatches(patches) => Ok(patches),
    }
}

fn build_ranking_translation(
    inputs: RankingTranslationBuildInputs<'_>,
    mode: ComponentBuildMode,
) -> Result<ComponentBuildOutput<RankingTranslationBuildReport>> {
    let executable_entry = find_inventory_file(inputs.inventory, EXECUTABLE_ISO_PATH)?;
    let image_entry = find_inventory_file(inputs.inventory, IMAGE_CONTAINER_ISO_PATH)?;
    ensure!(
        executable_entry.byte_length as usize == inputs.executable.len()
            && image_entry.byte_length as usize == inputs.image_container.len(),
        "verified ISO file lengths changed before ranking build"
    );
    let executable_sha256 = executable_entry
        .sha256
        .as_deref()
        .context("executable inventory entry has no SHA-256")?;
    let image_container_sha256 = image_entry
        .sha256
        .as_deref()
        .context("image-container inventory entry has no SHA-256")?;

    let bundle = load_graphics_translation_set(inputs.translation_path)?;
    ensure!(
        bundle.source.source_bin_sha256 == inputs.inventory.source.bin.sha256
            && bundle.source.executable_iso_path == EXECUTABLE_ISO_PATH
            && bundle.source.executable_sha256 == executable_sha256,
        "graphics translation bundle differs from the verified ranking source"
    );
    let target = bundle
        .entries
        .iter()
        .find(|entry| {
            entry.authoring.semantic_asset_id.as_deref() == Some(RANKING_SEMANTIC_ASSET_ID)
        })
        .context("graphics translation bundle has no semantic ranking target")?;
    ensure!(
        target.authoring.status == GraphicsTranslationStatus::NeedsHumanReview
            && target.source.container_iso_path == IMAGE_CONTAINER_ISO_PATH
            && target.source.image_file_record == RANKING_IMAGE_FILE_RECORD,
        "ranking target is not a bounded review-ready source surface"
    );
    ensure!(
        target.authoring.human_approval.is_none() && target.authoring.independent_review.is_none(),
        "ranking development build expects pending human review"
    );
    let declared_replacement = target
        .authoring
        .replacement_image
        .as_ref()
        .context("review-ready ranking target has no replacement image")?;
    ensure!(
        declared_replacement.path == inputs.asset_path.to_string_lossy(),
        "ranking build asset path differs from the translation ledger"
    );
    let asset_bytes = fs::read(inputs.asset_path)
        .with_context(|| format!("failed to read {}", inputs.asset_path.display()))?;
    ensure!(
        sha256(&asset_bytes) == declared_replacement.sha256,
        "ranking replacement image SHA-256 differs from the translation ledger"
    );
    let replacement = read_rgba_surface(inputs.asset_path)?;
    ensure!(
        replacement.width == usize::try_from(target.source.width)?
            && replacement.height == usize::try_from(target.source.height)?,
        "ranking replacement dimensions differ from the source surface"
    );

    let temporary = tempfile::tempdir().context("failed to create ranking build directory")?;
    let source_inputs = TextureSourceInputs {
        source_bin_sha256: inputs.inventory.source.bin.sha256.clone(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_sha256.to_string(),
        executable: inputs.executable,
        image_container_iso_path: IMAGE_CONTAINER_ISO_PATH.to_string(),
        image_container_sha256: image_container_sha256.to_string(),
        image_container: inputs.image_container,
    };
    let ranking_corpus = render_rank_textures(source_inputs, &temporary.path().join("source"))?;
    let rendered_source = ranking_corpus
        .textures
        .iter()
        .find(|texture| texture.image_file_record == RANKING_IMAGE_FILE_RECORD)
        .context("ranking renderer did not reproduce the semantic ranking target")?;
    ensure!(
        rendered_source.palette_file_record == RANKING_PALETTE_FILE_RECORD
            && rendered_source.width == target.source.width
            && rendered_source.height == target.source.height
            && rendered_source.linear_indices_sha256 == target.source.linear_indices_sha256,
        "rendered ranking source differs from the graphics translation ledger"
    );
    verify_ranking_label_patterns(&ranking_corpus.patterns.records)?;

    let file_list = read_executable_symbol(inputs.executable, IMAGE_FILE_LIST_SYMBOL)?;
    let records = parse_file_records(&file_list.bytes)?;
    let image_record = *records
        .get(RANKING_IMAGE_FILE_RECORD)
        .context("ranking image file record is out of range")?;
    let palette_record = *records
        .get(RANKING_PALETTE_FILE_RECORD)
        .context("ranking palette file record is out of range")?;
    let width = usize::try_from(image_record.width)?;
    let height = usize::try_from(image_record.height)?;
    let source_swizzled = read_decoded_image(inputs.image_container, image_record)?;
    let source_linear = unswizzle_psmt8_uploaded_as_psmct32(&source_swizzled, width, height)?;
    ensure!(
        sha256(&source_linear) == target.source.linear_indices_sha256,
        "ranking source indices differ from the graphics translation ledger"
    );
    let palette = read_palette(inputs.image_container, palette_record, true)?;
    let source_rgba = colorize_psmt8(&source_linear, &palette);
    ensure!(
        sha256(&source_rgba) == rendered_source.rgba_sha256,
        "ranking source RGBA differs from the consumer-linked render"
    );

    let protected_asset = target
        .authoring
        .protected_english_mask
        .as_ref()
        .context("ranking target has no protected complement")?;
    let protected_bytes = fs::read(&protected_asset.path)
        .with_context(|| format!("failed to read {}", protected_asset.path))?;
    ensure!(
        sha256(&protected_bytes) == protected_asset.sha256,
        "ranking protected complement SHA-256 differs from the ledger"
    );
    let protected_mask = read_rgba_surface(Path::new(&protected_asset.path))?;
    let (protected_pixels, changed_protected_pixels, changed_rgba_pixels) =
        verify_protected_pixels(&source_rgba, &replacement.pixels, &protected_mask)?;
    ensure!(
        changed_protected_pixels == 0 && changed_rgba_pixels > 0,
        "ranking replacement changed protected pixels or changed no Japanese pixels"
    );
    let (replacement_linear, changed_palette_indices, preserved_palette_indices) =
        map_rgba_to_source_palette(&source_linear, &source_rgba, &replacement.pixels, &palette)?;
    ensure!(
        changed_palette_indices == changed_rgba_pixels
            && colorize_psmt8(&replacement_linear, &palette) == replacement.pixels,
        "replacement palette indices do not reproduce the ranking asset"
    );
    let replacement_swizzled =
        swizzle_psmt8_uploaded_as_psmct32(&replacement_linear, width, height)?;
    let replacement_cpx = encode_cpx_block(&replacement_swizzled)?;
    let slot_offset = usize::try_from(image_record.sector_offset)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("ranking CPX slot offset overflow")?;
    let slot_length = usize::try_from(image_record.sector_count)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("ranking CPX slot length overflow")?;
    ensure!(
        replacement_cpx.len() <= slot_length,
        "replacement ranking CPX is {} bytes but its fixed slot is only {slot_length} bytes",
        replacement_cpx.len()
    );
    let source_cpx_length = usize::try_from(image_record.stored_byte_length)? + 1;
    let mut replacement_slot = replacement_cpx.clone();
    replacement_slot.resize(slot_length, 0);
    let image_plan = WritePlan::new()
        .region(ImageRegion {
            id: "ranking-cpx-slot".to_string(),
            range: slot_offset..slot_offset + slot_length,
            kind: RegionKind::Data,
            reason: "consumer-linked ranking-label CPX slot".to_string(),
        })
        .write(ExpectedWrite {
            id: "replace-ranking-cpx-slot".to_string(),
            owner: "ranking-translation-development".to_string(),
            purpose: "insert the bounded pre-human-review Korean ranking labels".to_string(),
            offset: slot_offset,
            expected_original: inputs.image_container[slot_offset..slot_offset + slot_length]
                .to_vec(),
            replacement: replacement_slot,
            intent: WriteIntent::Data,
        });
    let patched_image = image_plan.apply(inputs.image_container, None)?;
    let (patched_executable, executable_expected_writes) = patch_file_record_stored_length(
        inputs.executable,
        FileRecordStoredLengthPatch {
            file_list_offset: file_list.file_offset,
            record_index: RANKING_IMAGE_FILE_RECORD,
            expected_stored_length: image_record.stored_byte_length,
            replacement_cpx_length: replacement_cpx.len(),
            region_id: "ranking-cpx-stored-length",
            region_reason: "FileList_Image2 record 120 CPX stored-length field",
            write_id: "update-ranking-cpx-stored-length",
            owner: "ranking-translation-development",
            purpose: "make the source ranking loader consume the complete replacement CPX stream",
        },
    )?;

    let file_candidates = vec![
        IsoFileCandidate {
            iso_path: EXECUTABLE_ISO_PATH.to_string(),
            extent_lba: executable_entry.extent_lba,
            original: inputs.executable.to_vec(),
            replacement: patched_executable.clone(),
        },
        IsoFileCandidate {
            iso_path: IMAGE_CONTAINER_ISO_PATH.to_string(),
            extent_lba: image_entry.extent_lba,
            original: inputs.image_container.to_vec(),
            replacement: patched_image.clone(),
        },
    ];
    let sector_patches = plan_sector_patches(inputs.source_bin_path, &file_candidates)?;

    let readback_corpus = render_rank_textures(
        TextureSourceInputs {
            source_bin_sha256: hash_sector_patches(&sector_patches),
            executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
            executable_sha256: sha256(&patched_executable),
            executable: &patched_executable,
            image_container_iso_path: IMAGE_CONTAINER_ISO_PATH.to_string(),
            image_container_sha256: sha256(&patched_image),
            image_container: &patched_image,
        },
        &temporary.path().join("readback"),
    )?;
    let readback_ranking = readback_corpus
        .textures
        .iter()
        .find(|texture| texture.image_file_record == RANKING_IMAGE_FILE_RECORD)
        .context("patched ranking consumer no longer reaches record 120")?;
    ensure!(
        readback_ranking.rgba_sha256 == sha256(&replacement.pixels)
            && readback_ranking.linear_indices_sha256 == sha256(&replacement_linear),
        "patched ranking consumer readback differs from the replacement asset"
    );
    verify_ranking_label_patterns(&readback_corpus.patterns.records)?;

    if mode == ComponentBuildMode::ReturnSectorPatches {
        return Ok(ComponentBuildOutput::SectorPatches(sector_patches));
    }
    write_rgba_png(
        inputs.preview_path,
        image_record.width,
        image_record.height,
        &replacement.pixels,
    )?;
    let preview_bytes = fs::read(inputs.preview_path)
        .with_context(|| format!("failed to hash {}", inputs.preview_path.display()))?;
    ensure!(
        sha256(&preview_bytes) == declared_replacement.sha256,
        "ranking build preview differs from the declared replacement PNG"
    );
    let output_bin_sha256 = write_patched_bin(
        inputs.source_bin_path,
        inputs.output_bin_path,
        &sector_patches,
    )?;
    write_mode2_cue(inputs.output_cue_path, inputs.output_bin_path)?;
    verify_iso_readback(inputs.output_bin_path, &file_candidates)?;

    let modified_lbas = sector_patches.iter().map(|patch| patch.lba).collect();
    let raw_sector_data_writes = sector_patches.iter().map(|patch| patch.data_writes).sum();
    Ok(ComponentBuildOutput::Artifact(
        RankingTranslationBuildReport {
            schema: 1,
            status: "pre-human-review development image built; runtime verification pending",
            release_candidate_eligible: false,
            source_bin_sha256: inputs.inventory.source.bin.sha256.clone(),
            output_bin_path: path_string(inputs.output_bin_path)?,
            output_bin_byte_length: inputs.inventory.source.bin.byte_length,
            output_bin_sha256,
            output_cue_path: path_string(inputs.output_cue_path)?,
            preview_path: path_string(inputs.preview_path)?,
            preview_png_sha256: sha256(&preview_bytes),
            image_file_record: RANKING_IMAGE_FILE_RECORD,
            palette_file_record: RANKING_PALETTE_FILE_RECORD,
            source_linear_indices_sha256: sha256(&source_linear),
            replacement_linear_indices_sha256: sha256(&replacement_linear),
            replacement_rgba_sha256: sha256(&replacement.pixels),
            changed_rgba_pixels,
            changed_palette_indices,
            preserved_palette_indices,
            protected_pixels,
            changed_protected_pixels,
            source_cpx_byte_length: source_cpx_length,
            replacement_cpx_byte_length: replacement_cpx.len(),
            cpx_slot_byte_length: slot_length,
            cpx_padding_byte_length: slot_length - replacement_cpx.len(),
            image_container_expected_writes: image_plan.writes.len(),
            executable_expected_writes,
            raw_sector_plans: sector_patches.len(),
            raw_sector_data_writes,
            modified_lbas,
            verification: RankingTranslationBuildVerification {
                replacement_source: "the committed needs_human_review PNG and its ledger hash are the development-build input",
                consumer_rectangles: "RankPtn records 3 and 4 resolve registration slot 4 to image/palette pair 1 and consume the exact two 256x64 label windows",
                palette_mapping: "unchanged pixels preserve their logical PSMT8 index; changed pixels use an index already present in the exact source surface and reproduce the replacement RGBA exactly",
                cpx_roundtrip: "replacement swizzle -> CPX encode -> CPX decode is exact and fits the fixed source slot",
                output_iso_readback: "patched executable and IMAGE_G extents match the planned file candidates, and the ranking consumer rerender reaches the replacement indices",
                protected_pixels: "every pixel outside the two RankPtn Japanese label windows, including all English, remains exact in RGBA",
                human_review: "translation and art approval are pending; this artifact is not eligible as a release candidate",
                runtime_status: "pending fresh-process emulator ranking-screen verification",
            },
        },
    ))
}

fn verify_ranking_label_patterns(records: &[crate::rank_texture::RankPatternRecord]) -> Result<()> {
    let expected = [
        (3, 4, 1, 1, 0, 448, 256, 64),
        (4, 4, 1, 1, 256, 448, 256, 64),
    ];
    for (index, registration_slot, image_load, palette_slot, x, y, width, height) in expected {
        ensure!(
            records.iter().any(|record| {
                record.index == index
                    && record.registration_slot == registration_slot
                    && record.image_load_index == image_load
                    && record.palette_slot == palette_slot
                    && record.source_x == x
                    && record.source_y == y
                    && record.width == width
                    && record.height == height
            }),
            "ranking label consumer rectangle changed at RankPtn record {index}"
        );
    }
    Ok(())
}

fn verify_protected_pixels(
    source_rgba: &[u8],
    replacement_rgba: &[u8],
    protected_mask: &crate::graphics_translation::raster::RgbaSurface,
) -> Result<(usize, usize, usize)> {
    ensure!(
        source_rgba.len() == replacement_rgba.len()
            && source_rgba.len() == protected_mask.pixels.len(),
        "ranking protection inputs have different dimensions"
    );
    let mut protected_pixels = 0_usize;
    let mut changed_protected_pixels = 0_usize;
    let mut changed_pixels = 0_usize;
    for ((source, replacement), mask) in source_rgba
        .chunks_exact(4)
        .zip(replacement_rgba.chunks_exact(4))
        .zip(protected_mask.pixels.chunks_exact(4))
    {
        ensure!(
            mask[0] == mask[1]
                && mask[1] == mask[2]
                && matches!(mask[0], 0 | u8::MAX)
                && mask[3] == u8::MAX,
            "ranking protected mask is not binary opaque grayscale"
        );
        let changed = source != replacement;
        changed_pixels += usize::from(changed);
        if mask[0] == u8::MAX {
            protected_pixels += 1;
            changed_protected_pixels += usize::from(changed);
        }
    }
    Ok((protected_pixels, changed_protected_pixels, changed_pixels))
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .with_context(|| format!("path is not UTF-8: {}", path.display()))
}

#[cfg(test)]
#[path = "ranking_translation_build_tests.rs"]
mod ranking_translation_build_tests;
