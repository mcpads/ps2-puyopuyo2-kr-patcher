use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::{
    load_graphics_translation_set, validate_graphics_translation_asset_files,
};
use puyo2_ps2_tool::source::{find_inventory_file, read_inventory_file, write_json_file};
use puyo2_ps2_tool::story_dialogue_translation::{
    StoryDialogueTranslationBuildInputs, build_story_dialogue_translation_development,
    load_story_dialogue_translation_set, validate_story_dialogue_translation_bundle,
};

use super::{SourceArgs, inspect};

pub(super) struct StoryDialogueTranslationBuildCommand {
    pub(super) source: SourceArgs,
    pub(super) graphics_translation: PathBuf,
    pub(super) story_translation: PathBuf,
    pub(super) font_config: PathBuf,
    pub(super) font: PathBuf,
    pub(super) output_bin: PathBuf,
    pub(super) output_cue: PathBuf,
    pub(super) preview_directory: PathBuf,
    pub(super) report: PathBuf,
}

pub(super) fn run_story_dialogue_translation_build(
    command: StoryDialogueTranslationBuildCommand,
) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let executable = read_inventory_file(&command.source.bin, &inventory, "/SLPM_624.00;1")?;
    let image_container = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_MD.BIN;1")?;
    let graphics = load_graphics_translation_set(&command.graphics_translation)?;
    ensure!(
        graphics.source.source_bin_sha256 == inventory.source.bin.sha256
            && graphics.source.executable_iso_path == "/SLPM_624.00;1"
            && graphics.source.executable_sha256
                == find_inventory_file(&inventory, "/SLPM_624.00;1")?
                    .sha256
                    .as_deref()
                    .context("executable inventory entry has no SHA-256")?,
        "story graphics ledger differs from the verified source"
    );
    validate_graphics_translation_asset_files(&graphics)?;
    let story = load_story_dialogue_translation_set(&command.story_translation)?;
    validate_story_dialogue_translation_bundle(&story, &graphics)?;
    let report =
        build_story_dialogue_translation_development(StoryDialogueTranslationBuildInputs {
            inventory: &inventory,
            source_bin_path: &command.source.bin,
            executable: &executable,
            image_container: &image_container,
            story: &story,
            graphics: &graphics,
            font_config_path: &command.font_config,
            font_path: &command.font,
            output_bin_path: &command.output_bin,
            output_cue_path: &command.output_cue,
            preview_directory: &command.preview_directory,
        })?;
    write_json_file(&command.report, &report)?;
    println!(
        "built {} pre-human-review story dialogue replacements across {} changed raw sectors to {} ({}); wrote {}",
        report.surfaces,
        report.raw_sector_plans,
        command.output_bin.display(),
        report.output_bin_sha256,
        command.report.display()
    );
    Ok(())
}
