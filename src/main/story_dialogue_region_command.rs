use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::story_dialogue_regions::inventory_story_dialogue_regions;
use puyo2_ps2_tool::graphics_translation::{
    load_graphics_translation_set, write_graphics_translation_set,
};
use puyo2_ps2_tool::source::{find_inventory_file, read_json_file, write_json_file};
use puyo2_ps2_tool::story_dialogue_translation::{
    load_story_dialogue_translation_set, validate_story_dialogue_translation_bundle,
};
use puyo2_ps2_tool::texture_language_survey::verify_texture_preview_files;
use puyo2_ps2_tool::texture_preview_corpus::TexturePreviewCorpus;

use super::{SourceArgs, inspect};

pub(super) struct StoryDialogueRegionCommand {
    pub source: SourceArgs,
    pub graphics_translation: PathBuf,
    pub story_translation: PathBuf,
    pub texture_previews: PathBuf,
    pub output_directory: PathBuf,
    pub report: PathBuf,
}

pub(super) fn run_story_dialogue_region_inventory(
    command: StoryDialogueRegionCommand,
) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let mut graphics = load_graphics_translation_set(&command.graphics_translation)?;
    ensure!(
        graphics.source.source_bin_sha256 == inventory.source.bin.sha256,
        "story graphics ledger differs from the verified source BIN"
    );
    let executable_sha256 = find_inventory_file(&inventory, &graphics.source.executable_iso_path)?
        .sha256
        .as_deref()
        .with_context(|| {
            format!(
                "executable inventory entry has no SHA-256: {}",
                graphics.source.executable_iso_path
            )
        })?;
    ensure!(
        graphics.source.executable_sha256 == executable_sha256,
        "story graphics ledger differs from the verified executable"
    );

    let story = load_story_dialogue_translation_set(&command.story_translation)?;
    validate_story_dialogue_translation_bundle(&story, &graphics)?;
    let previews: TexturePreviewCorpus = read_json_file(&command.texture_previews)?;
    verify_texture_preview_files(&previews)?;
    let report = inventory_story_dialogue_regions(
        &mut graphics,
        &story,
        &previews,
        &command.output_directory,
    )?;
    write_graphics_translation_set(&command.graphics_translation, &graphics)?;
    write_json_file(&command.report, &report)?;
    let english_masks = report
        .surfaces
        .iter()
        .filter(|surface| surface.protected_english_mask.is_some())
        .count();
    println!(
        "inventoried {} Japanese-bearing story dialogue pixel regions with {} protected-English masks and left {} surfaces pending; wrote {} masks to {} and report {}",
        report.inventoried_surfaces,
        english_masks,
        report.skipped_surfaces.len(),
        report.inventoried_surfaces * 2 + english_masks,
        command.output_directory.display(),
        command.report.display()
    );
    Ok(())
}
