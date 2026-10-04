use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};
use serde::Serialize;

use super::{
    PreviewFileOutput, StoryDialogueReplacementInputs, StoryDialogueReplacementReport,
    generate_story_dialogue_replacements, paired_palette_file_record, path_string,
};
use crate::cpx::{CPX_BLOCK_ALIGNMENT, decode_cpx_block, encode_cpx_block};
use crate::cpx_patch::{FileRecordStoredLengthWrite, patch_file_record_stored_lengths};
use crate::disc_patch::{
    ComponentBuildMode, ComponentBuildOutput, IsoFileCandidate, SectorPatch, plan_sector_patches,
    verify_iso_readback, write_mode2_cue, write_patched_bin,
};
use crate::executable::read_executable_symbol;
use crate::graphics_translation::GraphicsTranslationBundle;
use crate::indexed_texture::{
    parse_file_records, read_decoded_image, sha256, swizzle_psmt8_uploaded_as_psmct32,
    unswizzle_psmt8_uploaded_as_psmct32,
};
use crate::source::{DiscInventory, find_inventory_file};
use crate::story_dialogue_translation::StoryDialogueTranslationBundle;

const EXECUTABLE_ISO_PATH: &str = "/SLPM_624.00;1";
const IMAGE_CONTAINER_ISO_PATH: &str = "/IMAGE_MD.BIN;1";
const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image3";

pub struct StoryDialogueTranslationBuildInputs<'a> {
    pub inventory: &'a DiscInventory,
    pub source_bin_path: &'a Path,
    pub executable: &'a [u8],
    pub image_container: &'a [u8],
    pub story: &'a StoryDialogueTranslationBundle,
    pub graphics: &'a GraphicsTranslationBundle,
    pub font_config_path: &'a Path,
    pub font_path: &'a Path,
    pub output_bin_path: &'a Path,
    pub output_cue_path: &'a Path,
    pub preview_directory: &'a Path,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoryDialogueTranslationBuildReport {
    pub schema: u32,
    pub status: &'static str,
    pub release_candidate_eligible: bool,
    pub source_bin_sha256: String,
    pub output_bin_path: String,
    pub output_bin_byte_length: u64,
    pub output_bin_sha256: String,
    pub output_cue_path: String,
    pub surfaces: usize,
    pub rows: usize,
    pub source_cpx_byte_length: usize,
    pub replacement_cpx_byte_length: usize,
    pub cpx_slot_byte_length: usize,
    pub cpx_padding_byte_length: usize,
    pub image_container_expected_writes: usize,
    pub executable_expected_writes: usize,
    pub raw_sector_plans: usize,
    pub raw_sector_data_writes: usize,
    pub modified_lbas: Vec<u32>,
    pub replacements: Vec<StoryDialogueCpxReplacement>,
    pub preview_report: StoryDialogueReplacementReport,
    pub verification: StoryDialogueTranslationBuildVerification,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoryDialogueCpxReplacement {
    pub story_surface_id: String,
    pub consumer_table: String,
    pub image_file_record: usize,
    pub palette_file_record: usize,
    pub width: usize,
    pub height: usize,
    pub replacement_linear_indices_sha256: String,
    pub source_cpx_byte_length: usize,
    pub replacement_cpx_byte_length: usize,
    pub cpx_slot_byte_length: usize,
    pub cpx_padding_byte_length: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoryDialogueTranslationBuildVerification {
    pub replacement_source: &'static str,
    pub consumer_routes: &'static str,
    pub cpx_roundtrip: &'static str,
    pub expected_writes: &'static str,
    pub output_iso_readback: &'static str,
    pub protected_pixels: &'static str,
    pub human_review: &'static str,
    pub runtime_status: &'static str,
}

pub fn build_story_dialogue_translation_development(
    inputs: StoryDialogueTranslationBuildInputs<'_>,
) -> Result<StoryDialogueTranslationBuildReport> {
    match build_story_dialogue_translation(inputs, ComponentBuildMode::WriteArtifact)? {
        ComponentBuildOutput::Artifact(report) => Ok(report),
        ComponentBuildOutput::SectorPatches(_) => unreachable!("artifact mode returns a report"),
    }
}

pub(crate) fn plan_story_dialogue_translation(
    inputs: StoryDialogueTranslationBuildInputs<'_>,
) -> Result<Vec<SectorPatch>> {
    match build_story_dialogue_translation(inputs, ComponentBuildMode::ReturnSectorPatches)? {
        ComponentBuildOutput::Artifact(_) => {
            unreachable!("sector-patch mode returns planned sectors")
        }
        ComponentBuildOutput::SectorPatches(patches) => Ok(patches),
    }
}

fn build_story_dialogue_translation(
    inputs: StoryDialogueTranslationBuildInputs<'_>,
    mode: ComponentBuildMode,
) -> Result<ComponentBuildOutput<StoryDialogueTranslationBuildReport>> {
    let executable_entry = find_inventory_file(inputs.inventory, EXECUTABLE_ISO_PATH)?;
    let image_entry = find_inventory_file(inputs.inventory, IMAGE_CONTAINER_ISO_PATH)?;
    let executable_sha256 = sha256(inputs.executable);
    let image_container_sha256 = sha256(inputs.image_container);
    ensure!(
        executable_entry.byte_length as usize == inputs.executable.len()
            && image_entry.byte_length as usize == inputs.image_container.len()
            && executable_entry.sha256.as_deref() == Some(executable_sha256.as_str())
            && image_entry.sha256.as_deref() == Some(image_container_sha256.as_str()),
        "verified ISO files changed before story dialogue build"
    );
    ensure!(
        inputs.story.source.source_bin_sha256 == inputs.inventory.source.bin.sha256
            && inputs.graphics.source.source_bin_sha256 == inputs.inventory.source.bin.sha256,
        "story dialogue ledgers differ from the verified source"
    );

    let preview_file_output = match mode {
        ComponentBuildMode::WriteArtifact => PreviewFileOutput::Write,
        ComponentBuildMode::ReturnSectorPatches => PreviewFileOutput::Omit,
    };
    let generated = generate_story_dialogue_replacements(
        StoryDialogueReplacementInputs {
            executable: inputs.executable,
            image_container: inputs.image_container,
            story: inputs.story,
            graphics: inputs.graphics,
            font_config_path: inputs.font_config_path,
            font_path: inputs.font_path,
            output_directory: inputs.preview_directory,
        },
        preview_file_output,
    )?;
    ensure!(
        generated.surfaces.len() == inputs.story.entries.len()
            && generated.report.surfaces == inputs.story.entries.len()
            && generated.report.changed_protected_pixels == 0
            && generated.report.changed_protected_english_pixels == 0
            && generated.report.uncleared_source_only_pixels == 0,
        "generated story replacements are not complete and protected"
    );

    let file_list = read_executable_symbol(inputs.executable, IMAGE_FILE_LIST_SYMBOL)?;
    let file_records = parse_file_records(&file_list.bytes)?;
    let mut image_plan = WritePlan::new();
    let mut length_writes = Vec::with_capacity(generated.surfaces.len());
    let mut replacements = Vec::with_capacity(generated.surfaces.len());
    let mut seen_records = BTreeSet::new();

    for ((entry, surface), preview) in inputs
        .story
        .entries
        .iter()
        .zip(&generated.surfaces)
        .zip(&generated.report.replacements)
    {
        ensure!(
            surface.image_file_record == entry.source.image_file_record
                && preview.image_file_record == entry.source.image_file_record
                && preview.replacement_linear_indices_sha256
                    == sha256(&surface.replacement_linear_indices)
                && seen_records.insert(surface.image_file_record),
            "story replacement order, identity, or uniqueness changed"
        );
        let record = *file_records
            .get(surface.image_file_record)
            .context("story replacement file record is out of range")?;
        ensure!(
            usize::try_from(record.width)? == surface.width
                && usize::try_from(record.height)? == surface.height,
            "story replacement dimensions differ from FileList_Image3"
        );
        let replacement_swizzled = swizzle_psmt8_uploaded_as_psmct32(
            &surface.replacement_linear_indices,
            surface.width,
            surface.height,
        )?;
        let replacement_cpx = encode_cpx_block(&replacement_swizzled)?;
        let decoded = decode_cpx_block(&replacement_cpx)?;
        ensure!(
            decoded.bytes == replacement_swizzled
                && decoded.encoded_byte_length == replacement_cpx.len()
                && usize::try_from(decoded.header_value)? == surface.width * surface.height,
            "story replacement CPX roundtrip differs from its generated indices"
        );

        let slot_offset = usize::try_from(record.sector_offset)?
            .checked_mul(CPX_BLOCK_ALIGNMENT)
            .context("story CPX slot offset overflow")?;
        let slot_length = usize::try_from(record.sector_count)?
            .checked_mul(CPX_BLOCK_ALIGNMENT)
            .context("story CPX slot length overflow")?;
        ensure!(
            replacement_cpx.len() <= slot_length,
            "story replacement {} CPX is {} bytes but its fixed slot is only {slot_length} bytes",
            entry.source.story_surface_id,
            replacement_cpx.len()
        );
        let source_cpx_length = usize::try_from(record.stored_byte_length)? + 1;
        let mut replacement_slot = replacement_cpx.clone();
        replacement_slot.resize(slot_length, 0);
        let source_slot = inputs
            .image_container
            .get(slot_offset..slot_offset + slot_length)
            .context("story CPX slot exceeds IMAGE_MD")?;
        let record_id = format!("story-cpx-record-{:03}", surface.image_file_record);
        image_plan = image_plan.region(ImageRegion {
            id: format!("{record_id}-slot"),
            range: slot_offset..slot_offset + slot_length,
            kind: RegionKind::Data,
            reason: format!(
                "{} consumer-linked story dialogue CPX slot",
                entry.source.consumer_table
            ),
        });
        image_plan = image_plan.write(ExpectedWrite {
            id: format!("replace-{record_id}-slot"),
            owner: "story-dialogue-translation-development".to_string(),
            purpose: format!(
                "insert the bounded pre-human-review Korean dialogue for {}",
                entry.source.story_surface_id
            ),
            offset: slot_offset,
            expected_original: source_slot.to_vec(),
            replacement: replacement_slot,
            intent: WriteIntent::Data,
        });
        length_writes.push(FileRecordStoredLengthWrite {
            file_list_offset: file_list.file_offset,
            record_index: surface.image_file_record,
            expected_stored_length: record.stored_byte_length,
            replacement_cpx_length: replacement_cpx.len(),
            region_id: format!("{record_id}-stored-length"),
            region_reason: format!(
                "FileList_Image3 record {} CPX stored-length field",
                surface.image_file_record
            ),
            write_id: format!("update-{record_id}-stored-length"),
            owner: "story-dialogue-translation-development".to_string(),
            purpose: format!(
                "make {} consume the complete Korean replacement CPX stream",
                entry.source.consumer_table
            ),
        });
        replacements.push(StoryDialogueCpxReplacement {
            story_surface_id: entry.source.story_surface_id.clone(),
            consumer_table: entry.source.consumer_table.clone(),
            image_file_record: surface.image_file_record,
            palette_file_record: preview.palette_file_record,
            width: surface.width,
            height: surface.height,
            replacement_linear_indices_sha256: preview.replacement_linear_indices_sha256.clone(),
            source_cpx_byte_length: source_cpx_length,
            replacement_cpx_byte_length: replacement_cpx.len(),
            cpx_slot_byte_length: slot_length,
            cpx_padding_byte_length: slot_length - replacement_cpx.len(),
        });
    }
    ensure!(
        image_plan.writes.len() == inputs.story.entries.len()
            && length_writes.len() == inputs.story.entries.len(),
        "story build did not plan every replacement before applying writes"
    );

    let patched_image = image_plan.apply(inputs.image_container, None)?;
    let (patched_executable, executable_expected_writes) =
        patch_file_record_stored_lengths(inputs.executable, &length_writes)?;
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
    verify_story_readback(
        &patched_executable,
        &patched_image,
        inputs.story,
        &generated.surfaces,
        &generated.report,
    )?;

    if mode == ComponentBuildMode::ReturnSectorPatches {
        return Ok(ComponentBuildOutput::SectorPatches(sector_patches));
    }
    let output_bin_sha256 = write_patched_bin(
        inputs.source_bin_path,
        inputs.output_bin_path,
        &sector_patches,
    )?;
    write_mode2_cue(inputs.output_cue_path, inputs.output_bin_path)?;
    verify_iso_readback(inputs.output_bin_path, &file_candidates)?;

    let modified_lbas = sector_patches.iter().map(|patch| patch.lba).collect();
    let raw_sector_data_writes = sector_patches.iter().map(|patch| patch.data_writes).sum();
    let source_cpx_byte_length = replacements
        .iter()
        .map(|replacement| replacement.source_cpx_byte_length)
        .sum();
    let replacement_cpx_byte_length = replacements
        .iter()
        .map(|replacement| replacement.replacement_cpx_byte_length)
        .sum();
    let cpx_slot_byte_length = replacements
        .iter()
        .map(|replacement| replacement.cpx_slot_byte_length)
        .sum();
    let cpx_padding_byte_length = replacements
        .iter()
        .map(|replacement| replacement.cpx_padding_byte_length)
        .sum();
    Ok(ComponentBuildOutput::Artifact(
        StoryDialogueTranslationBuildReport {
            schema: 1,
            status: "pre-human-review story dialogue development image built; runtime verification pending",
            release_candidate_eligible: false,
            source_bin_sha256: inputs.inventory.source.bin.sha256.clone(),
            output_bin_path: path_string(inputs.output_bin_path)?,
            output_bin_byte_length: fs::metadata(inputs.output_bin_path)?.len(),
            output_bin_sha256,
            output_cue_path: path_string(inputs.output_cue_path)?,
            surfaces: replacements.len(),
            rows: generated.report.rows,
            source_cpx_byte_length,
            replacement_cpx_byte_length,
            cpx_slot_byte_length,
            cpx_padding_byte_length,
            image_container_expected_writes: image_plan.writes.len(),
            executable_expected_writes,
            raw_sector_plans: sector_patches.len(),
            raw_sector_data_writes,
            modified_lbas,
            replacements,
            preview_report: generated.report,
            verification: StoryDialogueTranslationBuildVerification {
                replacement_source: "the current split story ledger, exact source masks, fixed Galmuri11 configuration, and source palette indices are regenerated in memory; preview PNG files are outputs, not build inputs",
                consumer_routes: "all DEMOTexTbl00..49 image and palette pairs are re-resolved after the FileList_Image3 metadata writes",
                cpx_roundtrip: "all 74 linear-index replacements roundtrip through PSMT8 swizzle, CPX encode, and CPX decode inside their original fixed slots",
                expected_writes: "all 74 IMAGE_MD CPX slots and all 74 FileList_Image3 stored-length fields are planned against immutable original bytes before either file is changed",
                output_iso_readback: "patched executable and IMAGE_MD extents match the two planned file candidates, and all 74 records decode to the exact generated replacement indices",
                protected_pixels: "the preview report verifies zero changes outside each source-Japanese and generated-Korean mask union, exact preservation of reviewed fixed-cell English pixels, and zero uncleared source-only pixels",
                human_review: "translation, transcription, and visual approval are pending; this artifact is not eligible as a release candidate",
                runtime_status: "pending fresh-process emulator story-scene verification",
            },
        },
    ))
}

fn verify_story_readback(
    executable: &[u8],
    image_container: &[u8],
    story: &StoryDialogueTranslationBundle,
    generated: &[super::GeneratedStoryDialogueSurface],
    preview_report: &StoryDialogueReplacementReport,
) -> Result<()> {
    let file_list = read_executable_symbol(executable, IMAGE_FILE_LIST_SYMBOL)?;
    let records = parse_file_records(&file_list.bytes)?;
    let executable = object::File::parse(executable)
        .context("failed to parse patched story dialogue executable")?;
    for ((entry, surface), preview) in story
        .entries
        .iter()
        .zip(generated)
        .zip(&preview_report.replacements)
    {
        let record = *records
            .get(surface.image_file_record)
            .context("patched story file record is out of range")?;
        let decoded = read_decoded_image(image_container, record)?;
        let image_byte_length = surface
            .width
            .checked_mul(surface.height)
            .context("patched story dimensions overflow")?;
        let linear = unswizzle_psmt8_uploaded_as_psmct32(
            decoded
                .get(..image_byte_length)
                .context("patched story image is shorter than its dimensions")?,
            surface.width,
            surface.height,
        )?;
        ensure!(
            linear == surface.replacement_linear_indices
                && sha256(&linear) == preview.replacement_linear_indices_sha256
                && paired_palette_file_record(&executable, entry)? == preview.palette_file_record,
            "patched story readback or consumer route differs for {}",
            entry.source.story_surface_id
        );
    }
    Ok(())
}
