use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::disc_patch::{
    SectorPatch, SectorPatchAccumulator, SectorPatchMergeReport, hash_sector_patches,
    verify_sector_patch_readback, write_mode2_cue, write_patched_bin,
};
use crate::executable_text_translation::ExecutableTextTranslationBundle;
use crate::graphics_translation::static_regions::StaticRegionCatalog;
use crate::graphics_translation::{
    GraphicsArtworkPlacementStatus, GraphicsTranslationBundle, GraphicsTreatmentProfile,
    graphics_translation_set_files,
};
use crate::memory_card_translation::MemoryCardTranslationBundle;
use crate::memory_card_translation_build::{
    MemoryCardTranslationBuildInputs, plan_memory_card_translation,
};
use crate::puzzle_puyo_translation_build::{
    PuzzlePuyoTranslationBuildInputs, plan_puzzle_puyo_translation,
};
use crate::ranking_translation_build::{RankingTranslationBuildInputs, plan_ranking_translation};
use crate::source::{DiscInventory, read_json_file};
use crate::static_ui_translation_build::{
    StaticGraphicsOverlayScope, StaticUiTranslationBuildInputs, plan_static_ui_translation,
};
use crate::story_dialogue_translation::{
    StoryDialogueTranslationBuildInputs, StoryDialogueTranslationBundle,
    plan_story_dialogue_translation, story_dialogue_translation_set_files,
};
use crate::texture_preview_corpus::TexturePreviewCorpus;
use crate::translation_terminology::{
    ApprovedTerminology, ApprovedTerminologyValidation, validate_approved_terminology,
};
use crate::tutorial_translation_build::{
    TutorialTranslationBuildInputs, plan_tutorial_translation,
};

pub struct TechnicalTranslationBuildInputs<'a> {
    pub inventory: &'a DiscInventory,
    pub source_bin_path: &'a Path,
    pub executable: &'a [u8],
    pub image_d: &'a [u8],
    pub image_g: &'a [u8],
    pub image_md: &'a [u8],
    pub memory_card_translation_path: &'a Path,
    pub executable_text_translation_path: &'a Path,
    pub approved_terminology_path: &'a Path,
    pub graphics_translation_path: &'a Path,
    pub graphics: &'a GraphicsTranslationBundle,
    pub story_translation_path: &'a Path,
    pub story: &'a StoryDialogueTranslationBundle,
    pub shared_font_config_path: &'a Path,
    pub story_font_config_path: &'a Path,
    pub shared_font_path: &'a Path,
    pub story_font_path: &'a Path,
    pub ranking_asset_path: &'a Path,
    pub static_catalog_path: &'a Path,
    pub static_catalog: &'a StaticRegionCatalog,
    pub texture_previews_path: &'a Path,
    pub texture_previews: &'a TexturePreviewCorpus,
    pub output_bin_path: &'a Path,
    pub output_cue_path: &'a Path,
}

#[derive(Clone, Debug, Serialize)]
pub struct TechnicalTranslationBuildReport {
    pub schema: u32,
    pub status: &'static str,
    pub release_candidate_eligible: bool,
    pub source_bin_sha256: String,
    pub output_bin_path: String,
    pub output_bin_byte_length: u64,
    pub output_bin_sha256: String,
    pub output_cue_path: String,
    pub included_components: usize,
    pub component_reports: Vec<TechnicalTranslationComponentReport>,
    pub input_artifacts: Vec<TechnicalTranslationInputArtifact>,
    pub approved_terminology: ApprovedTerminologyValidation,
    pub merged_raw_sectors: usize,
    pub merged_user_data_bytes: usize,
    pub merged_expected_data_writes: usize,
    pub repeated_identical_user_data_bytes: usize,
    pub verification: TechnicalTranslationBuildVerification,
}

#[derive(Clone, Debug, Serialize)]
pub struct TechnicalTranslationComponentReport {
    pub id: &'static str,
    pub status: &'static str,
    pub sector_patch_sha256: String,
    pub planned_raw_sectors: usize,
    pub planned_raw_sector_data_writes: usize,
    pub modified_lbas: Vec<u32>,
    pub changed_user_data_bytes: usize,
    pub added_user_data_bytes: usize,
    pub repeated_identical_user_data_bytes: usize,
    pub sectors_already_touched: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TechnicalTranslationInputArtifact {
    pub role: &'static str,
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TechnicalTranslationBuildVerification {
    pub source_identity: &'static str,
    pub component_builds: &'static str,
    pub composition: &'static str,
    pub expected_writes: &'static str,
    pub form1_integrity: &'static str,
    pub output_readback: &'static str,
    pub excluded_scope: String,
    pub human_review: &'static str,
    pub runtime_status: &'static str,
}

pub fn build_technical_translation_development(
    inputs: TechnicalTranslationBuildInputs<'_>,
) -> Result<TechnicalTranslationBuildReport> {
    ensure!(
        inputs.inventory.source.bin.sha256 == hash_file(inputs.source_bin_path)?,
        "technical build source BIN differs from the verified inventory"
    );
    let input_artifacts = technical_translation_input_artifacts(&inputs)?;
    let approved_terminology: ApprovedTerminology =
        read_json_file(inputs.approved_terminology_path)?;
    let memory_card_translation: MemoryCardTranslationBundle =
        read_json_file(inputs.memory_card_translation_path)?;
    let executable_text_translation: ExecutableTextTranslationBundle =
        read_json_file(inputs.executable_text_translation_path)?;
    let approved_terminology = validate_approved_terminology(
        &approved_terminology,
        &memory_card_translation,
        &executable_text_translation,
        inputs.graphics,
        inputs.story,
    )?;
    let output_directory = inputs.output_bin_path.parent().unwrap_or(Path::new("."));
    let mut accumulator = SectorPatchAccumulator::default();
    let mut component_reports = Vec::new();

    let memory_card = plan_memory_card_translation(MemoryCardTranslationBuildInputs {
        inventory: inputs.inventory,
        source_bin_path: inputs.source_bin_path,
        executable: inputs.executable,
        image_d_container: inputs.image_d,
        image_g_container: inputs.image_g,
        translation_path: inputs.memory_card_translation_path,
        font_config_path: inputs.shared_font_config_path,
        font_path: inputs.shared_font_path,
        output_bin_path: inputs.output_bin_path,
        output_cue_path: inputs.output_cue_path,
        preview_path: &output_directory.join("memory-card-font.png"),
    })?;
    component_reports.push(merge_component(
        &mut accumulator,
        "memory_card",
        "memory-card component patch plan verified against the immutable source",
        memory_card,
    )?);

    let tutorial = plan_tutorial_translation(TutorialTranslationBuildInputs {
        inventory: inputs.inventory,
        source_bin_path: inputs.source_bin_path,
        executable: inputs.executable,
        image_container: inputs.image_d,
        translation_path: inputs.executable_text_translation_path,
        font_config_path: inputs.shared_font_config_path,
        font_path: inputs.shared_font_path,
        output_bin_path: inputs.output_bin_path,
        output_cue_path: inputs.output_cue_path,
        preview_path: &output_directory.join("tutorial-font.png"),
    })?;
    component_reports.push(merge_component(
        &mut accumulator,
        "tutorial",
        "tutorial component patch plan verified against the immutable source",
        tutorial,
    )?);

    let puzzle_puyo = plan_puzzle_puyo_translation(PuzzlePuyoTranslationBuildInputs {
        inventory: inputs.inventory,
        source_bin_path: inputs.source_bin_path,
        executable: inputs.executable,
        image_container: inputs.image_g,
        translation_path: inputs.executable_text_translation_path,
        font_config_path: inputs.shared_font_config_path,
        font_path: inputs.shared_font_path,
        output_bin_path: inputs.output_bin_path,
        output_cue_path: inputs.output_cue_path,
        preview_path: &output_directory.join("puzzle-puyo-font.png"),
    })?;
    component_reports.push(merge_component(
        &mut accumulator,
        "puzzle_puyo",
        "Puzzle Puyo component patch plan verified against the immutable source",
        puzzle_puyo,
    )?);

    let story = plan_story_dialogue_translation(StoryDialogueTranslationBuildInputs {
        inventory: inputs.inventory,
        source_bin_path: inputs.source_bin_path,
        executable: inputs.executable,
        image_container: inputs.image_md,
        story: inputs.story,
        graphics: inputs.graphics,
        font_config_path: inputs.story_font_config_path,
        font_path: inputs.story_font_path,
        output_bin_path: inputs.output_bin_path,
        output_cue_path: inputs.output_cue_path,
        preview_directory: output_directory,
    })?;
    component_reports.push(merge_component(
        &mut accumulator,
        "story_dialogue",
        "story-dialogue component patch plan verified against the immutable source",
        story,
    )?);

    let ranking = plan_ranking_translation(RankingTranslationBuildInputs {
        inventory: inputs.inventory,
        source_bin_path: inputs.source_bin_path,
        executable: inputs.executable,
        image_container: inputs.image_g,
        translation_path: inputs.graphics_translation_path,
        asset_path: inputs.ranking_asset_path,
        output_bin_path: inputs.output_bin_path,
        output_cue_path: inputs.output_cue_path,
        preview_path: &output_directory.join("ranking.png"),
    })?;
    component_reports.push(merge_component(
        &mut accumulator,
        "ranking",
        "ranking component patch plan verified against the immutable source",
        ranking,
    )?);

    let static_graphics = plan_static_ui_translation(StaticUiTranslationBuildInputs {
        inventory: inputs.inventory,
        source_bin_path: inputs.source_bin_path,
        executable: inputs.executable,
        image_d: inputs.image_d,
        image_g: inputs.image_g,
        image_md: inputs.image_md,
        graphics: inputs.graphics,
        catalog: inputs.static_catalog,
        previews: inputs.texture_previews,
        output_bin_path: inputs.output_bin_path,
        output_cue_path: inputs.output_cue_path,
        preview_directory: output_directory,
        scope: StaticGraphicsOverlayScope::BuildableStaticGraphics,
    })?;
    component_reports.push(merge_component(
        &mut accumulator,
        "static_graphics",
        "static-graphics component patch plan verified against the immutable source",
        static_graphics,
    )?);

    let merged_raw_sectors = accumulator.merged_sector_count();
    let merged_user_data_bytes = accumulator.merged_user_data_bytes();
    let repeated_identical_user_data_bytes = component_reports
        .iter()
        .map(|component| component.repeated_identical_user_data_bytes)
        .sum();
    let patches = accumulator.into_sector_patches()?;
    let merged_expected_data_writes = patches.iter().map(|patch| patch.data_writes).sum();
    ensure!(
        input_artifacts == technical_translation_input_artifacts(&inputs)?,
        "technical translation inputs changed while component plans were being built"
    );
    let output_bin_sha256 =
        write_patched_bin(inputs.source_bin_path, inputs.output_bin_path, &patches)?;
    write_mode2_cue(inputs.output_cue_path, inputs.output_bin_path)?;
    verify_sector_patch_readback(inputs.output_bin_path, &patches)?;

    let stylized_surfaces = inputs
        .graphics
        .entries
        .iter()
        .filter(|entry| {
            entry.authoring.regions.iter().any(|region| {
                region.treatment_profile == GraphicsTreatmentProfile::StylizedDisplayLettering
            })
        })
        .collect::<Vec<_>>();
    let buildable_stylized_surfaces = stylized_surfaces
        .iter()
        .filter(|entry| {
            entry.authoring.regions.iter().any(|region| {
                region.treatment_profile == GraphicsTreatmentProfile::StylizedDisplayLettering
                    && region
                        .artwork_plan
                        .is_some_and(|plan| plan.placement_status.is_buildable())
            })
        })
        .count();
    let deferred_stylized_surfaces = stylized_surfaces
        .iter()
        .filter(|entry| {
            entry.authoring.regions.iter().any(|region| {
                region.treatment_profile == GraphicsTreatmentProfile::StylizedDisplayLettering
                    && region.artwork_plan.is_none_or(|plan| {
                        plan.placement_status == GraphicsArtworkPlacementStatus::RoughRegionBounds
                    })
            })
        })
        .count();

    Ok(TechnicalTranslationBuildReport {
        schema: 1,
        status: "six technical translation components composed from the immutable source; runtime verification and human review remain open",
        release_candidate_eligible: false,
        source_bin_sha256: inputs.inventory.source.bin.sha256.clone(),
        output_bin_path: inputs.output_bin_path.display().to_string(),
        output_bin_byte_length: fs::metadata(inputs.output_bin_path)?.len(),
        output_bin_sha256,
        output_cue_path: inputs.output_cue_path.display().to_string(),
        included_components: component_reports.len(),
        component_reports,
        input_artifacts,
        approved_terminology,
        merged_raw_sectors,
        merged_user_data_bytes,
        merged_expected_data_writes,
        repeated_identical_user_data_bytes,
        verification: TechnicalTranslationBuildVerification {
            source_identity: "all six component builders and the composer used the same manifest-verified immutable source BIN",
            component_builds: "each component completed its source, asset, Expected Write, CPX or text, and in-memory consumer checks before returning sector patches; no temporary full-disc candidate was emitted",
            composition: "only Form1 user-data bytes that differ from the immutable source were merged; disjoint and identical replacements are accepted and conflicting replacements fail closed",
            expected_writes: "the final merged user-data ranges and regenerated integrity bytes were reapplied as Expected Writes against each immutable source sector",
            form1_integrity: "every component sector was validated before merging and every final changed sector passed Mode2/Form1 EDC and P/Q ECC validation",
            output_readback: "the one cumulative BIN was written after composition; every final changed raw sector was read back byte-for-byte and its Form1 integrity was revalidated",
            excluded_scope: format!(
                "the other-title promotion catalog surfaces D56 through D58, all MOV_01 through MOV_11 PSS bytes, and {deferred_stylized_surfaces} included stylized-display surfaces pending proven placement evidence are excluded from this artifact; this artifact includes memory-card, tutorial, Puzzle Puyo, story-dialogue, ranking, functional static text, phrase-atlas work, and {buildable_stylized_surfaces} stylized-artwork surfaces with consumer-aligned or explicitly source-aligned placement evidence, including the deterministic G39, G77, MD300, and MD302 tea-label rasters",
            ),
            human_review: "free Korean wording, rendered graphics, and final release approval remain human gates",
            runtime_status: "pending exact-artifact cold-boot coverage of all included component routes",
        },
    })
}

fn technical_translation_input_artifacts(
    inputs: &TechnicalTranslationBuildInputs<'_>,
) -> Result<Vec<TechnicalTranslationInputArtifact>> {
    let mut input_artifacts = vec![
        input_artifact(
            "memory_card_translation",
            inputs.memory_card_translation_path,
        )?,
        input_artifact(
            "executable_text_translation",
            inputs.executable_text_translation_path,
        )?,
        input_artifact("approved_terminology", inputs.approved_terminology_path)?,
    ];
    input_artifacts.extend(translation_set_input_artifacts(
        "graphics_translation",
        "graphics_translation_part",
        graphics_translation_set_files(inputs.graphics_translation_path)?,
    )?);
    input_artifacts.extend(translation_set_input_artifacts(
        "story_translation",
        "story_translation_part",
        story_dialogue_translation_set_files(inputs.story_translation_path)?,
    )?);
    input_artifacts.extend([
        input_artifact("story_font_config", inputs.story_font_config_path)?,
        input_artifact("shared_font", inputs.shared_font_path)?,
        input_artifact("story_font", inputs.story_font_path)?,
        input_artifact("ranking_asset", inputs.ranking_asset_path)?,
        input_artifact("static_catalog", inputs.static_catalog_path)?,
        input_artifact("texture_previews", inputs.texture_previews_path)?,
    ]);
    Ok(input_artifacts)
}

fn merge_component(
    accumulator: &mut SectorPatchAccumulator,
    id: &'static str,
    status: &'static str,
    patches: Vec<SectorPatch>,
) -> Result<TechnicalTranslationComponentReport> {
    let planned_raw_sectors = patches.len();
    let planned_raw_sector_data_writes = patches.iter().map(|patch| patch.data_writes).sum();
    let modified_lbas = patches.iter().map(|patch| patch.lba).collect::<Vec<_>>();
    let sector_patch_sha256 = hash_sector_patches(&patches);
    let merge = accumulator.merge_sector_patches(&patches, id)?;
    ensure!(
        merge.declared_sectors == planned_raw_sectors,
        "component {id} merged-sector count differs from its patch plan"
    );
    Ok(component_report(
        id,
        status,
        sector_patch_sha256,
        planned_raw_sectors,
        planned_raw_sector_data_writes,
        modified_lbas,
        merge,
    ))
}

#[allow(clippy::too_many_arguments)]
fn component_report(
    id: &'static str,
    status: &'static str,
    sector_patch_sha256: String,
    planned_raw_sectors: usize,
    planned_raw_sector_data_writes: usize,
    modified_lbas: Vec<u32>,
    merge: SectorPatchMergeReport,
) -> TechnicalTranslationComponentReport {
    TechnicalTranslationComponentReport {
        id,
        status,
        sector_patch_sha256,
        planned_raw_sectors,
        planned_raw_sector_data_writes,
        modified_lbas,
        changed_user_data_bytes: merge.changed_user_data_bytes,
        added_user_data_bytes: merge.added_user_data_bytes,
        repeated_identical_user_data_bytes: merge.repeated_identical_user_data_bytes,
        sectors_already_touched: merge.sectors_already_touched,
    }
}

fn input_artifact(role: &'static str, path: &Path) -> Result<TechnicalTranslationInputArtifact> {
    Ok(TechnicalTranslationInputArtifact {
        role,
        path: path.display().to_string(),
        sha256: hash_file(path)?,
    })
}

fn translation_set_input_artifacts(
    manifest_role: &'static str,
    part_role: &'static str,
    files: Vec<std::path::PathBuf>,
) -> Result<Vec<TechnicalTranslationInputArtifact>> {
    ensure!(
        !files.is_empty(),
        "translation set input does not contain a manifest"
    );
    files
        .into_iter()
        .enumerate()
        .map(|(index, path)| {
            input_artifact(if index == 0 { manifest_role } else { part_role }, &path)
        })
        .collect()
}

fn hash_file(path: &Path) -> Result<String> {
    let mut reader = BufReader::new(
        File::open(path).with_context(|| format!("failed to open {}", path.display()))?,
    );
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
