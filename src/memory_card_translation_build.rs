use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::cpx::encode_cpx_block;
use crate::disc_patch::{
    ComponentBuildMode, ComponentBuildOutput, IsoFileCandidate, SectorPatch, plan_sector_patches,
    verify_iso_readback, write_mode2_cue, write_patched_bin,
};
use crate::executable_font_consumers::{
    ExecutableFontConsumerInputs, survey_executable_font_consumers,
};
use crate::font_atlas::{FontAtlasInputs, nazo::render_nazo_font_atlas, render_font_atlas};
use crate::glyph_allocation::{GlyphAllocationInputs, plan_glyph_allocation};
use crate::indexed_texture::{
    colorize_psmt8, sha256, swizzle_psmt8_uploaded_as_psmct32, write_rgba_png,
};
use crate::memory_card_translation::{
    MemoryCardTranslationBundle, MemoryCardTranslationInputs,
    validate_memory_card_translation_bundle, validate_memory_card_translation_capacity,
};
use crate::message_corpus::{
    MessageCorpusInputs, survey_memory_card_messages, survey_memory_card_save_messages,
    survey_my_nazo_error_messages,
};
use crate::source::{find_inventory_file, read_json_file};

mod container_patch;
mod executable_patch;
pub(crate) mod font;
mod model;
mod nazo_font_clone;
mod replica_consumer_safety;

use container_patch::{FontContainerTarget, replace_shared_font_clones};
use executable_patch::{
    FontFileRecordTarget, encode_memory_card_messages, patch_executable, verify_patched_executable,
};
use font::{
    FontConfiguration, choose_background_index, choose_foreground_index, collect_unique_hangul,
    count_changed_pixels_outside_slots, load_font, load_font_resources, render_hangul,
    select_reassignable_slots,
};
use model::{MemoryCardFontSurfaceReport, MemoryCardTranslationBuildVerification};
use nazo_font_clone::{
    FILE_LIST_SYMBOL as NAZO_FILE_LIST_SYMBOL,
    IMAGE_CONTAINER_ISO_PATH as NAZO_IMAGE_CONTAINER_ISO_PATH, load_nazo_menu_font_clone,
};
use replica_consumer_safety::validate_secondary_font_replica_consumer_scope;

pub use model::{
    FontReplicaConsumerSafetyReport, MemoryCardTranslationBuildInputs,
    MemoryCardTranslationBuildReport,
};

const EXECUTABLE_ISO_PATH: &str = "/SLPM_624.00;1";
const PRIMARY_IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_D.BIN;1";
const PRIMARY_FILE_LIST_SYMBOL: &str = "FileList_Image";

pub fn build_memory_card_translation_development(
    inputs: MemoryCardTranslationBuildInputs<'_>,
) -> Result<MemoryCardTranslationBuildReport> {
    match build_memory_card_translation(inputs, ComponentBuildMode::WriteArtifact)? {
        ComponentBuildOutput::Artifact(report) => Ok(report),
        ComponentBuildOutput::SectorPatches(_) => unreachable!("artifact mode returns a report"),
    }
}

pub(crate) fn plan_memory_card_translation(
    inputs: MemoryCardTranslationBuildInputs<'_>,
) -> Result<Vec<SectorPatch>> {
    match build_memory_card_translation(inputs, ComponentBuildMode::ReturnSectorPatches)? {
        ComponentBuildOutput::Artifact(_) => {
            unreachable!("sector-patch mode returns planned sectors")
        }
        ComponentBuildOutput::SectorPatches(patches) => Ok(patches),
    }
}

fn build_memory_card_translation(
    inputs: MemoryCardTranslationBuildInputs<'_>,
    mode: ComponentBuildMode,
) -> Result<ComponentBuildOutput<MemoryCardTranslationBuildReport>> {
    let executable_entry = find_inventory_file(inputs.inventory, EXECUTABLE_ISO_PATH)?;
    let image_d_entry = find_inventory_file(inputs.inventory, PRIMARY_IMAGE_CONTAINER_ISO_PATH)?;
    let image_g_entry = find_inventory_file(inputs.inventory, NAZO_IMAGE_CONTAINER_ISO_PATH)?;
    ensure!(
        executable_entry.byte_length as usize == inputs.executable.len()
            && image_d_entry.byte_length as usize == inputs.image_d_container.len()
            && image_g_entry.byte_length as usize == inputs.image_g_container.len(),
        "verified ISO file lengths changed before memory-card translation build"
    );
    let executable_sha256 = executable_entry
        .sha256
        .as_deref()
        .context("executable inventory entry has no SHA-256")?;
    let image_d_sha256 = image_d_entry
        .sha256
        .as_deref()
        .context("IMAGE_D inventory entry has no SHA-256")?;
    let image_g_sha256 = image_g_entry
        .sha256
        .as_deref()
        .context("IMAGE_G inventory entry has no SHA-256")?;
    let source_bin_sha256 = &inputs.inventory.source.bin.sha256;

    let bundle: MemoryCardTranslationBundle = read_json_file(inputs.translation_path)?;
    let corpus_inputs = || MessageCorpusInputs {
        source_bin_sha256: source_bin_sha256.clone(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_sha256.to_string(),
        executable: inputs.executable,
    };
    let load = survey_memory_card_messages(corpus_inputs())?;
    let save = survey_memory_card_save_messages(corpus_inputs())?;
    let my_nazo_error = survey_my_nazo_error_messages(corpus_inputs())?;
    let validation = validate_memory_card_translation_bundle(
        &bundle,
        MemoryCardTranslationInputs {
            source_bin_sha256,
            executable_iso_path: EXECUTABLE_ISO_PATH,
            executable_sha256,
            load: &load,
            save: &save,
            my_nazo_error: &my_nazo_error,
        },
    )?;
    ensure!(
        validation.untranslated == 0 && validation.unique_messages == bundle.entries.len(),
        "memory-card development build requires a Korean draft for every exact-source message"
    );

    let temporary = tempfile::tempdir().context("failed to create memory-card build directory")?;
    let source_font = render_font_atlas(
        FontAtlasInputs {
            source_bin_sha256: source_bin_sha256.clone(),
            executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
            executable_sha256: executable_sha256.to_string(),
            executable: inputs.executable,
            image_container_iso_path: PRIMARY_IMAGE_CONTAINER_ISO_PATH.to_string(),
            image_container_sha256: image_d_sha256.to_string(),
            image_container: inputs.image_d_container,
        },
        &temporary.path().join("source-font.png"),
    )?;
    let nazo_font = render_nazo_font_atlas(
        FontAtlasInputs {
            source_bin_sha256: source_bin_sha256.clone(),
            executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
            executable_sha256: executable_sha256.to_string(),
            executable: inputs.executable,
            image_container_iso_path: NAZO_IMAGE_CONTAINER_ISO_PATH.to_string(),
            image_container_sha256: image_g_sha256.to_string(),
            image_container: inputs.image_g_container,
        },
        &temporary.path().join("source-nazo-font.png"),
    )?;
    let replica_consumer_safety =
        validate_secondary_font_replica_consumer_scope(inputs.executable)?;
    let message_corpora = [load, save, my_nazo_error];
    let font_consumers = survey_executable_font_consumers(ExecutableFontConsumerInputs {
        source_bin_sha256: source_bin_sha256.clone(),
        executable_iso_path: EXECUTABLE_ISO_PATH.to_string(),
        executable_sha256: executable_sha256.to_string(),
        executable: inputs.executable,
    })?;
    let allocation = plan_glyph_allocation(GlyphAllocationInputs {
        executable: inputs.executable,
        font_atlas: &source_font,
        message_corpora: &message_corpora,
        font_consumers: &font_consumers,
    })?;
    let capacity = validate_memory_card_translation_capacity(&bundle, &allocation)?;
    ensure!(
        capacity.translated_messages == bundle.entries.len(),
        "memory-card capacity plan does not cover every translated message"
    );
    let hangul = collect_unique_hangul(
        bundle
            .entries
            .iter()
            .map(|entry| entry.translation.text.as_str()),
    )?;
    ensure!(
        hangul.len() == capacity.required_unique_hangul_glyphs,
        "memory-card first-appearance glyph demand differs from the capacity check"
    );
    let slots = select_reassignable_slots(&allocation, hangul.len())?;

    let font_config: FontConfiguration = read_json_file(inputs.font_config_path)?;
    let font = load_font(&font_config, inputs.font_path)?;
    let mut font_resources =
        load_font_resources(inputs.executable, inputs.image_d_container, &source_font)?;
    let nazo_font_resources =
        load_nazo_menu_font_clone(inputs.executable, inputs.image_g_container, &nazo_font)?;
    let source_primary_font_resources = font_resources.clone();
    let source_linear_indices = font_resources.linear_indices.clone();
    let source_linear_indices_sha256 = sha256(&source_linear_indices);
    let atlas_width = usize::try_from(font_resources.image_record.width)?;
    let atlas_height = usize::try_from(font_resources.image_record.height)?;
    let background = choose_background_index(&font_resources.linear_indices, atlas_width, &slots)?;
    let foreground = choose_foreground_index(
        &font_resources.linear_indices,
        &font_resources.palette,
        &source_font,
        atlas_width,
        background,
    )?;
    let assignments = render_hangul(
        &font,
        &hangul,
        &slots,
        &font_config.rasterization,
        background,
        foreground,
        atlas_width,
        &mut font_resources.linear_indices,
    )?;
    let (protected_font_pixels, changed_protected_font_pixels) =
        count_changed_pixels_outside_slots(
            &source_linear_indices,
            &font_resources.linear_indices,
            atlas_width,
            atlas_height,
            &slots,
        )?;
    ensure!(
        changed_protected_font_pixels == 0,
        "memory-card font replacement changed protected Japanese, English, or non-glyph pixels"
    );
    let replacement_linear_indices_sha256 = sha256(&font_resources.linear_indices);
    ensure!(
        replacement_linear_indices_sha256 != source_linear_indices_sha256,
        "memory-card font replacement changed no palette indices"
    );

    let encoded_messages = encode_memory_card_messages(inputs.executable, &bundle, &assignments)?;
    ensure!(
        encoded_messages.len() == bundle.entries.len(),
        "memory-card executable patch does not cover the complete translation bundle"
    );
    let replacement_swizzled = swizzle_psmt8_uploaded_as_psmct32(
        &font_resources.linear_indices,
        atlas_width,
        atlas_height,
    )?;
    let replacement_cpx = encode_cpx_block(&replacement_swizzled)?;
    let patched_fonts = replace_shared_font_clones(
        FontContainerTarget {
            container_iso_path: PRIMARY_IMAGE_CONTAINER_ISO_PATH,
            region_id: "memory-card-primary-font-cpx-slot",
            region_reason: "consumer-linked IMAGE_D record 21 main memory-card font CPX slot",
            write_id: "replace-memory-card-primary-font-cpx-slot",
            source: inputs.image_d_container,
            resources: &source_primary_font_resources,
        },
        FontContainerTarget {
            container_iso_path: NAZO_IMAGE_CONTAINER_ISO_PATH,
            region_id: "memory-card-nazo-font-clone-cpx-slot",
            region_reason: "consumer-linked IMAGE_G record 122 Nazo-menu main-font clone CPX slot",
            write_id: "replace-memory-card-nazo-font-clone-cpx-slot",
            source: inputs.image_g_container,
            resources: &nazo_font_resources,
        },
        &replacement_cpx,
    )?;
    let file_record_targets = [
        FontFileRecordTarget {
            role: "primary memory-card font",
            file_list_symbol: PRIMARY_FILE_LIST_SYMBOL,
            image_file_record: source_primary_font_resources.image_record_index,
            expected_stored_length: source_primary_font_resources
                .image_record
                .stored_byte_length,
        },
        FontFileRecordTarget {
            role: "Nazo-menu main-font clone",
            file_list_symbol: NAZO_FILE_LIST_SYMBOL,
            image_file_record: nazo_font_resources.image_record_index,
            expected_stored_length: nazo_font_resources.image_record.stored_byte_length,
        },
    ];
    let (patched_executable, executable_expected_writes) = patch_executable(
        inputs.executable,
        &assignments,
        &encoded_messages,
        &file_record_targets,
        replacement_cpx.len(),
    )?;
    verify_patched_executable(
        inputs.executable,
        &patched_executable,
        &assignments,
        &encoded_messages,
        &file_record_targets,
        replacement_cpx.len(),
    )?;

    let file_candidates = vec![
        IsoFileCandidate {
            iso_path: EXECUTABLE_ISO_PATH.to_string(),
            extent_lba: executable_entry.extent_lba,
            original: inputs.executable.to_vec(),
            replacement: patched_executable.clone(),
        },
        IsoFileCandidate {
            iso_path: PRIMARY_IMAGE_CONTAINER_ISO_PATH.to_string(),
            extent_lba: image_d_entry.extent_lba,
            original: inputs.image_d_container.to_vec(),
            replacement: patched_fonts.primary.bytes.clone(),
        },
        IsoFileCandidate {
            iso_path: NAZO_IMAGE_CONTAINER_ISO_PATH.to_string(),
            extent_lba: image_g_entry.extent_lba,
            original: inputs.image_g_container.to_vec(),
            replacement: patched_fonts.nazo_menu.bytes.clone(),
        },
    ];
    let sector_patches = plan_sector_patches(inputs.source_bin_path, &file_candidates)?;

    let readback_font = load_font_resources(
        &patched_executable,
        &patched_fonts.primary.bytes,
        &source_font,
    )?;
    ensure!(
        sha256(&readback_font.linear_indices) == replacement_linear_indices_sha256,
        "IMAGE_D memory-card font consumer readback differs from the replacement indices"
    );
    let readback_nazo_font = load_nazo_menu_font_clone(
        &patched_executable,
        &patched_fonts.nazo_menu.bytes,
        &nazo_font,
    )?;
    ensure!(
        sha256(&readback_nazo_font.linear_indices) == replacement_linear_indices_sha256,
        "IMAGE_G Nazo-menu font consumer readback differs from the replacement indices"
    );
    verify_patched_executable(
        inputs.executable,
        &patched_executable,
        &assignments,
        &encoded_messages,
        &file_record_targets,
        replacement_cpx.len(),
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
        MemoryCardTranslationBuildReport {
            schema: 1,
            status: "pre-human-review memory-card development image built; translation review and cold-boot runtime verification remain open",
            release_candidate_eligible: false,
            source_bin_sha256: source_bin_sha256.clone(),
            output_bin_path: path_string(inputs.output_bin_path)?,
            output_bin_byte_length: inputs.inventory.source.bin.byte_length,
            output_bin_sha256,
            output_cue_path: path_string(inputs.output_cue_path)?,
            preview_path: path_string(inputs.preview_path)?,
            preview_png_sha256: sha256(&preview_bytes),
            translated_messages: encoded_messages.len(),
            in_progress_messages: validation.in_progress,
            needs_review_messages: validation.needs_review,
            needs_human_review_messages: validation.needs_human_review,
            assigned_hangul_glyphs: assignments.len(),
            assignable_font_slots: capacity.proven_reassignable_slots,
            remaining_font_slots: capacity.remaining_reassignable_slots,
            font_id: font_config.id,
            font_sha256: font_config.sha256,
            font_license: font_config.license,
            font_upstream_repository: font_config.upstream_repository,
            font_pixel_size_milli: (font_config.rasterization.pixel_size * 1000.0).round() as u32,
            font_coverage_threshold: font_config.rasterization.coverage_threshold,
            source_linear_indices_sha256: source_linear_indices_sha256.clone(),
            replacement_linear_indices_sha256: replacement_linear_indices_sha256.clone(),
            protected_font_pixels,
            changed_protected_font_pixels,
            assignments,
            font_surfaces: vec![
                MemoryCardFontSurfaceReport {
                    role: "primary memory-card load/save font",
                    container_iso_path: PRIMARY_IMAGE_CONTAINER_ISO_PATH,
                    file_list_symbol: PRIMARY_FILE_LIST_SYMBOL,
                    image_file_record: source_primary_font_resources.image_record_index,
                    palette_file_record: source_primary_font_resources.palette_record_index,
                    registration_path: "MemLoadTexTbl image 1/palette 1 -> RegBuff slot 1 -> FontPrint_Act",
                    source_cpx_sha256: patched_fonts.primary.source_cpx_sha256.clone(),
                    replacement_cpx_sha256: patched_fonts.primary.replacement_cpx_sha256.clone(),
                    source_linear_indices_sha256: source_linear_indices_sha256.clone(),
                    replacement_linear_indices_sha256: replacement_linear_indices_sha256.clone(),
                    source_cpx_byte_length: patched_fonts.primary.source_cpx_byte_length,
                    replacement_cpx_byte_length: patched_fonts.primary.replacement_cpx_byte_length,
                    cpx_slot_byte_length: patched_fonts.primary.slot_byte_length,
                    cpx_padding_byte_length: patched_fonts.primary.padding_byte_length,
                    container_expected_writes: patched_fonts.primary.expected_writes,
                },
                MemoryCardFontSurfaceReport {
                    role: "My Nazo error-path main-font clone",
                    container_iso_path: NAZO_IMAGE_CONTAINER_ISO_PATH,
                    file_list_symbol: NAZO_FILE_LIST_SYMBOL,
                    image_file_record: nazo_font_resources.image_record_index,
                    palette_file_record: nazo_font_resources.palette_record_index,
                    registration_path: "NazoTexTbl image 9/palette 9 -> every Nazo registration set RegBuff slot 1 -> FontPrint_Act",
                    source_cpx_sha256: patched_fonts.nazo_menu.source_cpx_sha256.clone(),
                    replacement_cpx_sha256: patched_fonts.nazo_menu.replacement_cpx_sha256.clone(),
                    source_linear_indices_sha256,
                    replacement_linear_indices_sha256,
                    source_cpx_byte_length: patched_fonts.nazo_menu.source_cpx_byte_length,
                    replacement_cpx_byte_length: patched_fonts
                        .nazo_menu
                        .replacement_cpx_byte_length,
                    cpx_slot_byte_length: patched_fonts.nazo_menu.slot_byte_length,
                    cpx_padding_byte_length: patched_fonts.nazo_menu.padding_byte_length,
                    container_expected_writes: patched_fonts.nazo_menu.expected_writes,
                },
            ],
            font_replica_consumer_safety: replica_consumer_safety,
            executable_expected_writes,
            image_container_expected_writes: patched_fonts.primary.expected_writes
                + patched_fonts.nazo_menu.expected_writes,
            raw_sector_plans: sector_patches.len(),
            raw_sector_data_writes,
            modified_lbas,
            verification: MemoryCardTranslationBuildVerification {
                source_consumers: "MemLoad_Act and MemSave_Act -> MsgPrint_Act -> FontPrint_Act use MemLoadTexTbl slot 1 -> D21; Nazo_Menu_Run calls MyNazo_ErrMsg -> MsgPrint_Act and every Nazo registration set maps NazoTexTbl image 9/palette 9 -> RegBuff slot 1 -> G122/G242",
                translation_scope: "all 75 unique Japanese memory-card messages are replaced; protected ASCII runs and runtime controls remain in source order",
                lookup: "FontList_Zen custom codes use deterministic first-match slots selected from the closed main-message consumer census; identical assignments are mirrored into G122 after its Frame, Nazo, Nazo-ending, Rank, fixed-pattern, direct-pattern, and dead-record census proves no live fixed slot-1 conflict",
                font_rendering: "the fixed Galmuri14 input is centered in original 24x24 cells using source palette indices",
                protected_pixels: "D21 and its byte/palette/index-identical G122 source clone receive the same replacement; all pixels outside assigned fullwidth slots, including halfwidth English glyphs and still-required Japanese slots, remain unchanged",
                cpx_roundtrip: "one replacement swizzle -> CPX encode -> CPX decode is exact in both fixed D21 and G122 slots",
                output_iso_readback: "patched executable, IMAGE_D, and IMAGE_G extents match immutable-source Expected Writes; both FileList stored lengths and both consumer-linked decoded index surfaces match the replacement",
                human_review: "all selected translations remain pre-human-review; despite the closed exact-source G122 consumer census, this development artifact is not release-candidate eligible",
                runtime_status: "pending cold-boot memory-card load, save, overwrite, failure, and My Nazo error-path verification",
            },
        },
    ))
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .with_context(|| format!("path is not UTF-8: {}", path.display()))
}
