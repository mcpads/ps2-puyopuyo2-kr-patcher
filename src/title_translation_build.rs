use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};
use serde::Serialize;

use crate::cpx::{CPX_BLOCK_ALIGNMENT, encode_cpx_block};
use crate::cpx_patch::{FileRecordStoredLengthPatch, patch_file_record_stored_length};
use crate::disc_patch::{
    IsoFileCandidate, plan_sector_patches, verify_iso_readback, write_mode2_cue, write_patched_bin,
};
use crate::executable::read_executable_symbol;
use crate::graphics_translation::title_draft::{TitleDraftAsset, render_title_translation_draft};
use crate::graphics_translation::{GraphicsTranslationStatus, load_graphics_translation_set};
use crate::indexed_texture::{
    colorize_psmt8, map_rgba_to_source_palette, parse_file_records, read_decoded_image,
    read_palette, sha256, swizzle_psmt8_uploaded_as_psmct32, unswizzle_psmt8_uploaded_as_psmct32,
};
use crate::source::{DiscInventory, find_inventory_file, read_json_file};
use crate::texture_source::TextureSourceInputs;
use crate::title_texture::render_title_textures;

const EXECUTABLE_ISO_PATH: &str = "/SLPM_624.00;1";
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_D.BIN;1";
const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image";
const TITLE_SEMANTIC_ASSET_ID: &str = "title/primary-branding-sheet";
const TITLE_IMAGE_FILE_RECORD: usize = 19;
const TITLE_PALETTE_FILE_RECORD: usize = 79;

pub struct TitleTranslationBuildInputs<'a> {
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
pub struct TitleTranslationBuildReport {
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
    pub source_cpx_byte_length: usize,
    pub replacement_cpx_byte_length: usize,
    pub cpx_slot_byte_length: usize,
    pub cpx_padding_byte_length: usize,
    pub image_container_expected_writes: usize,
    pub executable_expected_writes: usize,
    pub raw_sector_plans: usize,
    pub raw_sector_data_writes: usize,
    pub modified_lbas: Vec<u32>,
    pub verification: TitleTranslationBuildVerification,
}

#[derive(Clone, Debug, Serialize)]
pub struct TitleTranslationBuildVerification {
    pub draft_source: &'static str,
    pub palette_mapping: &'static str,
    pub cpx_roundtrip: &'static str,
    pub output_iso_readback: &'static str,
    pub protected_pixels: &'static str,
    pub human_review: &'static str,
    pub runtime_status: &'static str,
}

pub fn build_title_translation_development(
    inputs: TitleTranslationBuildInputs<'_>,
) -> Result<TitleTranslationBuildReport> {
    let executable_entry = find_inventory_file(inputs.inventory, EXECUTABLE_ISO_PATH)?;
    let image_entry = find_inventory_file(inputs.inventory, IMAGE_CONTAINER_ISO_PATH)?;
    ensure!(
        executable_entry.byte_length as usize == inputs.executable.len()
            && image_entry.byte_length as usize == inputs.image_container.len(),
        "verified ISO file lengths changed before title build"
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
        "graphics translation bundle differs from the verified title source"
    );
    let target = bundle
        .entries
        .iter()
        .find(|entry| entry.authoring.semantic_asset_id.as_deref() == Some(TITLE_SEMANTIC_ASSET_ID))
        .context("graphics translation bundle has no semantic title target")?;
    ensure!(
        target.authoring.status == GraphicsTranslationStatus::NeedsHumanReview
            && target.source.container_iso_path == IMAGE_CONTAINER_ISO_PATH
            && target.source.image_file_record == TITLE_IMAGE_FILE_RECORD,
        "title target is not a bounded review-ready source surface"
    );
    let asset_bytes = fs::read(inputs.asset_path)
        .with_context(|| format!("failed to hash {}", inputs.asset_path.display()))?;
    ensure!(
        target.authoring.design_candidates.iter().any(|candidate| {
            candidate.path == inputs.asset_path.to_string_lossy()
                && candidate.sha256 == sha256(&asset_bytes)
        }),
        "title draft asset is not declared by the graphics translation ledger"
    );
    let asset: TitleDraftAsset = read_json_file(inputs.asset_path)?;

    let temporary = tempfile::tempdir().context("failed to create title build directory")?;
    let source_inputs = TextureSourceInputs {
        source_bin_sha256: inputs.inventory.source.bin.sha256.clone(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_sha256.to_string(),
        executable: inputs.executable,
        image_container_iso_path: IMAGE_CONTAINER_ISO_PATH.to_string(),
        image_container_sha256: image_container_sha256.to_string(),
        image_container: inputs.image_container,
    };
    let title_corpus = render_title_textures(source_inputs, &temporary.path().join("source"))?;
    let rendered_source = title_corpus
        .textures
        .iter()
        .find(|texture| texture.image_file_record == TITLE_IMAGE_FILE_RECORD)
        .context("title renderer did not reproduce the semantic title target")?;
    ensure!(
        rendered_source.palette_file_record == TITLE_PALETTE_FILE_RECORD
            && rendered_source.width == target.source.width
            && rendered_source.height == target.source.height
            && rendered_source.linear_indices_sha256 == target.source.linear_indices_sha256,
        "rendered title source differs from the graphics translation ledger"
    );
    ensure!(
        title_corpus.patterns.iter().any(|pattern| {
            pattern.index == 2
                && pattern.image_load_index == 1
                && pattern.source_x == 232
                && pattern.source_y == 0
                && pattern.width == 232
                && pattern.height == 96
        }) && title_corpus.patterns.iter().any(|pattern| {
            pattern.index == 3
                && pattern.image_load_index == 1
                && pattern.source_x == 472
                && pattern.source_y == 0
                && pattern.width == 176
                && pattern.height == 144
        }),
        "title logo body and Japanese suffix source rectangles changed"
    );
    let rendered_draft = render_title_translation_draft(
        Path::new(&rendered_source.png_path),
        &asset,
        inputs.preview_path,
        &temporary.path().join("japanese.mask.png"),
        &temporary.path().join("nontext.mask.png"),
        &temporary.path().join("english.mask.png"),
    )?;
    let declared_replacement = target
        .authoring
        .replacement_image
        .as_ref()
        .context("review-ready title target has no declared full-surface replacement image")?;
    let declared_replacement_bytes = fs::read(&declared_replacement.path).with_context(|| {
        format!(
            "failed to read title replacement image {}",
            declared_replacement.path
        )
    })?;
    ensure!(
        declared_replacement.sha256 == sha256(&declared_replacement_bytes)
            && declared_replacement.sha256 == rendered_draft.report.output_png_sha256,
        "declared title replacement image differs from the deterministic draft"
    );

    let file_list = read_executable_symbol(inputs.executable, IMAGE_FILE_LIST_SYMBOL)?;
    let records = parse_file_records(&file_list.bytes)?;
    let image_record = *records
        .get(TITLE_IMAGE_FILE_RECORD)
        .context("title image file record is out of range")?;
    let palette_record = *records
        .get(TITLE_PALETTE_FILE_RECORD)
        .context("title palette file record is out of range")?;
    let width = usize::try_from(image_record.width)?;
    let height = usize::try_from(image_record.height)?;
    let source_swizzled = read_decoded_image(inputs.image_container, image_record)?;
    let source_linear = unswizzle_psmt8_uploaded_as_psmct32(&source_swizzled, width, height)?;
    ensure!(
        sha256(&source_linear) == target.source.linear_indices_sha256,
        "title source indices differ from the graphics translation ledger"
    );
    let palette = read_palette(inputs.image_container, palette_record, true)?;
    let source_rgba = colorize_psmt8(&source_linear, &palette);
    ensure!(
        sha256(&source_rgba) == rendered_source.rgba_sha256,
        "title source RGBA differs from the consumer-linked render"
    );
    let (replacement_linear, changed_palette_indices, preserved_palette_indices) =
        map_rgba_to_source_palette(&source_linear, &source_rgba, &rendered_draft.rgba, &palette)?;
    ensure!(
        colorize_psmt8(&replacement_linear, &palette) == rendered_draft.rgba,
        "replacement palette indices do not reproduce the title draft"
    );

    let replacement_swizzled =
        swizzle_psmt8_uploaded_as_psmct32(&replacement_linear, width, height)?;
    let replacement_cpx = encode_cpx_block(&replacement_swizzled)?;
    let slot_offset = usize::try_from(image_record.sector_offset)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("title CPX slot offset overflow")?;
    let slot_length = usize::try_from(image_record.sector_count)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("title CPX slot length overflow")?;
    ensure!(
        replacement_cpx.len() <= slot_length,
        "replacement title CPX is {} bytes but its fixed slot is only {slot_length} bytes",
        replacement_cpx.len()
    );
    let source_cpx_length = usize::try_from(image_record.stored_byte_length)? + 1;
    let mut replacement_slot = replacement_cpx.clone();
    replacement_slot.resize(slot_length, 0);
    let image_plan = WritePlan::new()
        .region(ImageRegion {
            id: "title-cpx-slot".to_string(),
            range: slot_offset..slot_offset + slot_length,
            kind: RegionKind::Data,
            reason: "consumer-linked title branding CPX slot".to_string(),
        })
        .write(ExpectedWrite {
            id: "replace-title-cpx-slot".to_string(),
            owner: "title-translation-development".to_string(),
            purpose: "insert the bounded pre-human-review Korean title draft".to_string(),
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
            record_index: TITLE_IMAGE_FILE_RECORD,
            expected_stored_length: image_record.stored_byte_length,
            replacement_cpx_length: replacement_cpx.len(),
            region_id: "title-cpx-stored-length",
            region_reason: "FileList_Image record 19 CPX stored-length field",
            write_id: "update-title-cpx-stored-length",
            owner: "title-translation-development",
            purpose: "make the source title loader consume the complete replacement CPX stream",
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
    let output_bin_sha256 = write_patched_bin(
        inputs.source_bin_path,
        inputs.output_bin_path,
        &sector_patches,
    )?;
    write_mode2_cue(inputs.output_cue_path, inputs.output_bin_path)?;
    verify_iso_readback(inputs.output_bin_path, &file_candidates)?;

    let readback_corpus = render_title_textures(
        TextureSourceInputs {
            source_bin_sha256: output_bin_sha256.clone(),
            executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
            executable_sha256: sha256(&patched_executable),
            executable: &patched_executable,
            image_container_iso_path: IMAGE_CONTAINER_ISO_PATH.to_string(),
            image_container_sha256: sha256(&patched_image),
            image_container: &patched_image,
        },
        &temporary.path().join("readback"),
    )?;
    let readback_title = readback_corpus
        .textures
        .iter()
        .find(|texture| texture.image_file_record == TITLE_IMAGE_FILE_RECORD)
        .context("patched title consumer no longer reaches record 19")?;
    ensure!(
        readback_title.rgba_sha256 == sha256(&rendered_draft.rgba)
            && readback_title.linear_indices_sha256 == sha256(&replacement_linear),
        "patched title consumer readback differs from the in-memory draft"
    );

    let modified_lbas = sector_patches.iter().map(|patch| patch.lba).collect();
    let raw_sector_data_writes = sector_patches.iter().map(|patch| patch.data_writes).sum();
    Ok(TitleTranslationBuildReport {
        schema: 1,
        status: "pre-human-review development image built; runtime verification pending",
        release_candidate_eligible: false,
        source_bin_sha256: inputs.inventory.source.bin.sha256.clone(),
        output_bin_path: path_string(inputs.output_bin_path)?,
        output_bin_byte_length: inputs.inventory.source.bin.byte_length,
        output_bin_sha256,
        output_cue_path: path_string(inputs.output_cue_path)?,
        preview_path: path_string(inputs.preview_path)?,
        preview_png_sha256: rendered_draft.report.output_png_sha256,
        image_file_record: TITLE_IMAGE_FILE_RECORD,
        palette_file_record: TITLE_PALETTE_FILE_RECORD,
        source_linear_indices_sha256: sha256(&source_linear),
        replacement_linear_indices_sha256: sha256(&replacement_linear),
        replacement_rgba_sha256: sha256(&rendered_draft.rgba),
        changed_rgba_pixels: rendered_draft.report.changed_pixels,
        changed_palette_indices,
        preserved_palette_indices,
        source_cpx_byte_length: source_cpx_length,
        replacement_cpx_byte_length: replacement_cpx.len(),
        cpx_slot_byte_length: slot_length,
        cpx_padding_byte_length: slot_length - replacement_cpx.len(),
        image_container_expected_writes: image_plan.writes.len(),
        executable_expected_writes,
        raw_sector_plans: sector_patches.len(),
        raw_sector_data_writes,
        modified_lbas,
        verification: TitleTranslationBuildVerification {
            draft_source: "the committed candidate manifest and exact-source masks were regenerated before insertion",
            palette_mapping: "unchanged pixels preserve their logical PSMT8 index; changed pixels use an index already present in the exact source surface and reproduce the draft RGBA exactly",
            cpx_roundtrip: "replacement swizzle -> CPX encode -> CPX decode is exact and fits the fixed source slot",
            output_iso_readback: "patched executable and IMAGE_D extents match the planned file candidates, and the title consumer rerender reaches the replacement indices",
            protected_pixels: "pixels outside the Japanese edit bounds and declared non-text and English components remain exact in RGBA",
            human_review: "translation and art approval are pending; this artifact is not eligible as a release candidate",
            runtime_status: "pending fresh-process emulator title-screen verification",
        },
    })
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .with_context(|| format!("path is not UTF-8: {}", path.display()))
}
