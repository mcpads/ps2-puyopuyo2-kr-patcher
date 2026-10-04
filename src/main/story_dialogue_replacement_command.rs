use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::{
    load_graphics_translation_set, validate_graphics_translation_asset_files,
};
use puyo2_ps2_tool::source::{find_inventory_file, read_inventory_file, write_json_file};
use puyo2_ps2_tool::story_dialogue_translation::{
    StoryDialogueReplacementInputs, load_story_dialogue_translation_set,
    render_story_dialogue_replacements, validate_story_dialogue_translation_bundle,
};

use super::{SourceArgs, inspect};

pub(super) struct StoryDialogueReplacementCommand {
    pub(super) source: SourceArgs,
    pub(super) graphics_translation: PathBuf,
    pub(super) story_translation: PathBuf,
    pub(super) font_config: PathBuf,
    pub(super) font: PathBuf,
    pub(super) output_directory: PathBuf,
    pub(super) report: PathBuf,
}

pub(super) fn run_story_dialogue_replacement_render(
    command: StoryDialogueReplacementCommand,
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
    let report = render_story_dialogue_replacements(StoryDialogueReplacementInputs {
        executable: &executable,
        image_container: &image_container,
        story: &story,
        graphics: &graphics,
        font_config_path: &command.font_config,
        font_path: &command.font,
        output_directory: &command.output_directory,
    })?;
    write_json_file(&command.report, &report)?;
    println!(
        "rendered {} pre-human-review story dialogue replacements with {} Korean rows; changed {} pixels inside {} editable-union pixels, 0 of {} protected pixels, and 0 of {} protected English pixels; wrote {}",
        report.surfaces,
        report.rows,
        report.changed_pixels,
        report.editable_union_pixels,
        report.protected_pixels,
        report.protected_english_pixels,
        command.report.display(),
    );
    Ok(())
}
