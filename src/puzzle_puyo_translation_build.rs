use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::cpx::{CPX_BLOCK_ALIGNMENT, encode_cpx_block};
use crate::disc_patch::{
    ComponentBuildMode, ComponentBuildOutput, IsoFileCandidate, SectorPatch, plan_sector_patches,
    verify_iso_readback, write_mode2_cue, write_patched_bin,
};
use crate::executable_text_translation::{
    ExecutableTextTranslationBundle, ExecutableTextTranslationInputs,
    plan_puzzle_puyo_halfwidth_glyphs, plan_puzzle_puyo_text_glyphs,
    validate_executable_text_translation_bundle,
};
use crate::font_atlas::FontAtlasInputs;
use crate::font_atlas::nazo::render_nazo_font_atlas;
use crate::indexed_texture::{
    colorize_psmt8, sha256, swizzle_psmt8_uploaded_as_psmct32, write_rgba_png,
};
use crate::memory_card_metadata::{MemoryCardMetadataInputs, survey_memory_card_metadata};
use crate::message_corpus::{MessageCorpusInputs, survey_nazo_messages, survey_tutorial_messages};
use crate::source::{find_inventory_file, read_json_file};

mod executable_patch;
mod font;
mod menu_layout;
mod model;

use executable_patch::{encode_puzzle_puyo_messages, patch_executable, verify_patched_executable};
use font::{
    choose_background_index, choose_foreground_index, count_changed_pixels_outside_assignments,
    load_font, load_font_resources, render_assigned_glyphs,
};
use model::{FontConfiguration, PuzzlePuyoTranslationBuildVerification};

pub use model::{PuzzlePuyoTranslationBuildInputs, PuzzlePuyoTranslationBuildReport};

const EXECUTABLE_ISO_PATH: &str = "/SLPM_624.00;1";
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_G.BIN;1";

pub fn build_puzzle_puyo_translation_development(
    inputs: PuzzlePuyoTranslationBuildInputs<'_>,
) -> Result<PuzzlePuyoTranslationBuildReport> {
    match build_puzzle_puyo_translation(inputs, ComponentBuildMode::WriteArtifact)? {
        ComponentBuildOutput::Artifact(report) => Ok(report),
        ComponentBuildOutput::SectorPatches(_) => unreachable!("artifact mode returns a report"),
    }
}

pub(crate) fn plan_puzzle_puyo_translation(
    inputs: PuzzlePuyoTranslationBuildInputs<'_>,
) -> Result<Vec<SectorPatch>> {
    match build_puzzle_puyo_translation(inputs, ComponentBuildMode::ReturnSectorPatches)? {
        ComponentBuildOutput::Artifact(_) => {
            unreachable!("sector-patch mode returns planned sectors")
        }
        ComponentBuildOutput::SectorPatches(patches) => Ok(patches),
    }
}

fn build_puzzle_puyo_translation(
    inputs: PuzzlePuyoTranslationBuildInputs<'_>,
    mode: ComponentBuildMode,
) -> Result<ComponentBuildOutput<PuzzlePuyoTranslationBuildReport>> {
    let executable_entry = find_inventory_file(inputs.inventory, EXECUTABLE_ISO_PATH)?;
    let image_entry = find_inventory_file(inputs.inventory, IMAGE_CONTAINER_ISO_PATH)?;
    ensure!(
        executable_entry.byte_length as usize == inputs.executable.len()
            && image_entry.byte_length as usize == inputs.image_container.len(),
        "verified ISO file lengths changed before Puzzle Puyo build"
    );
    let executable_sha256 = executable_entry
        .sha256
        .as_deref()
        .context("executable inventory entry has no SHA-256")?;
    let image_sha256 = image_entry
        .sha256
        .as_deref()
        .context("IMAGE_G inventory entry has no SHA-256")?;
    let source_bin_sha256 = &inputs.inventory.source.bin.sha256;

    let bundle: ExecutableTextTranslationBundle = read_json_file(inputs.translation_path)?;
    let corpus_inputs = || MessageCorpusInputs {
        source_bin_sha256: source_bin_sha256.clone(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_sha256.to_string(),
        executable: inputs.executable,
    };
    let tutorial = survey_tutorial_messages(corpus_inputs())?;
    let puzzle_puyo = survey_nazo_messages(corpus_inputs())?;
    let metadata = survey_memory_card_metadata(MemoryCardMetadataInputs {
        source_bin_sha256: source_bin_sha256.clone(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_sha256.to_string(),
        executable: inputs.executable,
    })?;
    let validation = validate_executable_text_translation_bundle(
        &bundle,
        ExecutableTextTranslationInputs {
            source_bin_sha256,
            executable_iso_path: EXECUTABLE_ISO_PATH,
            executable_sha256,
            tutorial: &tutorial,
            puzzle_puyo: &puzzle_puyo,
            memory_card_metadata: &metadata,
        },
    )?;

    let temporary = tempfile::tempdir().context("failed to create Puzzle Puyo build directory")?;
    let source_font = render_nazo_font_atlas(
        FontAtlasInputs {
            source_bin_sha256: source_bin_sha256.clone(),
            executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
            executable_sha256: executable_sha256.to_string(),
            executable: inputs.executable,
            image_container_iso_path: IMAGE_CONTAINER_ISO_PATH.to_string(),
            image_container_sha256: image_sha256.to_string(),
            image_container: inputs.image_container,
        },
        &temporary.path().join("source-font.png"),
    )?;
    let glyph_plan = plan_puzzle_puyo_text_glyphs(&bundle, &validation, &source_font)?;
    let halfwidth_glyph_plan = plan_puzzle_puyo_halfwidth_glyphs(&bundle, &source_font)?;

    let font_config: FontConfiguration = read_json_file(inputs.font_config_path)?;
    let font = load_font(&font_config, inputs.font_path)?;
    let mut font_resources =
        load_font_resources(inputs.executable, inputs.image_container, &source_font)?;
    let source_linear_indices = font_resources.linear_indices.clone();
    let source_linear_sha256 = sha256(&font_resources.linear_indices);
    let background = choose_background_index(
        &font_resources.linear_indices,
        &source_font,
        &glyph_plan.assignments,
    )?;
    let foreground = choose_foreground_index(
        &font_resources.linear_indices,
        &font_resources.palette,
        &source_font,
        background,
    )?;
    render_assigned_glyphs(
        &font,
        &font_config.rasterization,
        &source_font.fullwidth,
        &glyph_plan.assignments,
        background,
        foreground,
        &mut font_resources.linear_indices,
    )?;
    render_assigned_glyphs(
        &font,
        &font_config.rasterization,
        &source_font.halfwidth,
        &halfwidth_glyph_plan.assignments,
        background,
        foreground,
        &mut font_resources.linear_indices,
    )?;
    let (protected_halfwidth_pixels, changed_protected_halfwidth_pixels) =
        count_changed_pixels_outside_assignments(
            &source_linear_indices,
            &font_resources.linear_indices,
            &source_font.halfwidth,
            &halfwidth_glyph_plan.assignments,
        )?;
    ensure!(
        protected_halfwidth_pixels == (32 - halfwidth_glyph_plan.required_added_glyphs) * 16 * 32
            && changed_protected_halfwidth_pixels == 0,
        "Puzzle Puyo protected halfwidth ASCII atlas pixels changed"
    );
    let replacement_linear_sha256 = sha256(&font_resources.linear_indices);
    ensure!(
        replacement_linear_sha256 != source_linear_sha256,
        "Puzzle Puyo font replacement changed no palette indices"
    );

    let encoded_messages =
        encode_puzzle_puyo_messages(inputs.executable, &bundle, &glyph_plan.assignments)?;
    ensure!(
        encoded_messages.len() == 36,
        "Puzzle Puyo build does not cover all 36 translated messages"
    );
    let replacement_swizzled = swizzle_psmt8_uploaded_as_psmct32(
        &font_resources.linear_indices,
        usize::try_from(font_resources.image_record.width)?,
        usize::try_from(font_resources.image_record.height)?,
    )?;
    let replacement_cpx = encode_cpx_block(&replacement_swizzled)?;
    let slot_offset = usize::try_from(font_resources.image_record.sector_offset)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("Puzzle Puyo font CPX slot offset overflow")?;
    let slot_length = usize::try_from(font_resources.image_record.sector_count)?
        .checked_mul(CPX_BLOCK_ALIGNMENT)
        .context("Puzzle Puyo font CPX slot length overflow")?;
    ensure!(
        replacement_cpx.len() <= slot_length,
        "replacement Puzzle Puyo font CPX is {} bytes but its fixed slot is only {slot_length} bytes",
        replacement_cpx.len()
    );
    let source_cpx_length = usize::try_from(font_resources.image_record.stored_byte_length)? + 1;
    let mut replacement_slot = replacement_cpx.clone();
    replacement_slot.resize(slot_length, 0);
    let image_plan = WritePlan::new()
        .region(ImageRegion {
            id: "puzzle-puyo-font-cpx-slot".to_string(),
            range: slot_offset..slot_offset + slot_length,
            kind: RegionKind::Data,
            reason: "consumer-linked G 113 Puzzle Puyo font CPX slot".to_string(),
        })
        .write(ExpectedWrite {
            id: "replace-puzzle-puyo-font-cpx-slot".to_string(),
            owner: "puzzle-puyo-translation-development".to_string(),
            purpose: "insert the pre-human-review Korean Puzzle Puyo glyph atlas".to_string(),
            offset: slot_offset,
            expected_original: inputs.image_container[slot_offset..slot_offset + slot_length]
                .to_vec(),
            replacement: replacement_slot,
            intent: WriteIntent::Data,
        });
    let patched_image = image_plan.apply(inputs.image_container, None)?;
    let (patched_executable, executable_expected_writes) = patch_executable(
        inputs.executable,
        &glyph_plan.assignments,
        &halfwidth_glyph_plan.assignments,
        &encoded_messages,
        source_font.atlas.image_file_record,
        font_resources.image_record.stored_byte_length,
        replacement_cpx.len(),
    )?;
    verify_patched_executable(
        inputs.executable,
        &patched_executable,
        &glyph_plan.assignments,
        &halfwidth_glyph_plan.assignments,
        &encoded_messages,
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

    let readback_font = load_font_resources(&patched_executable, &patched_image, &source_font)?;
    ensure!(
        sha256(&readback_font.linear_indices) == replacement_linear_sha256,
        "Puzzle Puyo font consumer readback differs from the replacement indices"
    );
    verify_patched_executable(
        inputs.executable,
        &patched_executable,
        &glyph_plan.assignments,
        &halfwidth_glyph_plan.assignments,
        &encoded_messages,
    )?;

    if mode == ComponentBuildMode::ReturnSectorPatches {
        return Ok(ComponentBuildOutput::SectorPatches(sector_patches));
    }
    write_rgba_png(
        inputs.preview_path,
        readback_font.image_record.width,
        readback_font.image_record.height,
        &colorize_psmt8(&readback_font.linear_indices, &readback_font.palette),
    )?;
    let preview_bytes = fs::read(inputs.preview_path)
        .with_context(|| format!("failed to hash {}", inputs.preview_path.display()))?;
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
        PuzzlePuyoTranslationBuildReport {
            schema: 1,
            status: "pre-human-review Puzzle Puyo development image built; runtime verification pending",
            release_candidate_eligible: false,
            source_bin_sha256: source_bin_sha256.clone(),
            output_bin_path: path_string(inputs.output_bin_path)?,
            output_bin_byte_length: inputs.inventory.source.bin.byte_length,
            output_bin_sha256,
            output_cue_path: path_string(inputs.output_cue_path)?,
            preview_path: path_string(inputs.preview_path)?,
            preview_png_sha256: sha256(&preview_bytes),
            translated_messages: encoded_messages.len(),
            assigned_hangul_glyphs: glyph_plan.required_unique_hangul_glyphs,
            assigned_halfwidth_glyphs: halfwidth_glyph_plan.required_added_glyphs,
            remaining_halfwidth_slots: halfwidth_glyph_plan.remaining_shadowed_space_slots,
            assignable_font_slots: glyph_plan.assignable_slots,
            remaining_font_slots: glyph_plan.remaining_slots,
            reserved_fallback_slot: glyph_plan.reserved_fallback_slot,
            font_table_symbol: glyph_plan.font_table_symbol,
            font_id: font_config.id,
            font_sha256: font_config.sha256,
            font_license: font_config.license,
            font_upstream_repository: font_config.upstream_repository,
            font_pixel_size_milli: (font_config.rasterization.pixel_size * 1000.0).round() as u32,
            font_coverage_threshold: font_config.rasterization.coverage_threshold,
            font_image_file_record: source_font.atlas.image_file_record,
            font_palette_file_record: font_resources.palette_record_index,
            source_linear_indices_sha256: source_linear_sha256,
            replacement_linear_indices_sha256: replacement_linear_sha256,
            protected_halfwidth_pixels,
            changed_protected_halfwidth_pixels,
            source_cpx_byte_length: source_cpx_length,
            replacement_cpx_byte_length: replacement_cpx.len(),
            cpx_slot_byte_length: slot_length,
            cpx_padding_byte_length: slot_length - replacement_cpx.len(),
            executable_expected_writes,
            image_container_expected_writes: image_plan.writes.len(),
            raw_sector_plans: sector_patches.len(),
            raw_sector_data_writes,
            modified_lbas,
            verification: PuzzlePuyoTranslationBuildVerification {
                source_consumer: "NazoMsgPrint_Act message producers and NazoTexTbl/FileList_Image2 -> G 113 are revalidated from the exact source",
                translation_scope: "all 36 Japanese Puzzle Puyo messages are replaced; English text and its existing halfwidth glyphs remain unchanged",
                lookup: "FontList_Nazo custom codes and supplemental ASCII codes first-match deterministic slots; only shadowed duplicate halfwidth spaces are reassigned, while fallback slot 80 and both final terminators remain intact",
                font_rendering: "the fixed Galmuri14 input is centered in the original 32x32 Hangul and 16x32 supplemental halfwidth cells using source palette indices",
                cpx_roundtrip: "replacement swizzle -> CPX encode -> CPX decode is exact and fits the fixed G 113 slot",
                output_iso_readback: "patched executable and IMAGE_G extents match the immutable-source Expected Write candidates; the unchanged NazoTexTbl binding decodes the replacement indices",
                human_review: "all 36 translations and their layout remain pre-human-review; this artifact is not release-candidate eligible",
                runtime_status: "pending cold-boot Puzzle Puyo screen verification",
            },
        },
    ))
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .with_context(|| format!("path is not UTF-8: {}", path.display()))
}
