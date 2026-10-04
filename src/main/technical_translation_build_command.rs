use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::static_regions::StaticRegionCatalog;
use puyo2_ps2_tool::graphics_translation::{
    load_graphics_translation_set, validate_graphics_translation_asset_files,
};
use puyo2_ps2_tool::source::{
    find_inventory_file, read_inventory_file, read_json_file, write_json_file,
};
use puyo2_ps2_tool::story_dialogue_translation::{
    load_story_dialogue_translation_set, validate_story_dialogue_translation_bundle,
};
use puyo2_ps2_tool::technical_translation_build::{
    TechnicalTranslationBuildInputs, build_technical_translation_development,
};
use puyo2_ps2_tool::texture_language_survey::verify_texture_preview_files;
use puyo2_ps2_tool::texture_preview_corpus::TexturePreviewCorpus;

use super::{SourceArgs, inspect};

pub(super) struct TechnicalTranslationBuildCommand {
    pub source: SourceArgs,
    pub memory_card_translation: PathBuf,
    pub executable_text_translation: PathBuf,
    pub approved_terminology: PathBuf,
    pub graphics_translation: PathBuf,
    pub story_translation: PathBuf,
    pub shared_font_config: PathBuf,
    pub story_font_config: PathBuf,
    pub shared_font: PathBuf,
    pub story_font: PathBuf,
    pub ranking_asset: PathBuf,
    pub static_catalog: PathBuf,
    pub texture_previews: PathBuf,
    pub output_bin: PathBuf,
    pub output_cue: PathBuf,
    pub report: PathBuf,
}

pub(super) fn run_technical_translation_build(
    command: TechnicalTranslationBuildCommand,
) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let executable = read_inventory_file(&command.source.bin, &inventory, "/SLPM_624.00;1")?;
    let image_d = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_D.BIN;1")?;
    let image_g = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_G.BIN;1")?;
    let image_md = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_MD.BIN;1")?;

    let graphics = load_graphics_translation_set(&command.graphics_translation)?;
    ensure!(
        graphics.source.source_bin_sha256 == inventory.source.bin.sha256
            && graphics.source.executable_iso_path == "/SLPM_624.00;1"
            && graphics.source.executable_sha256
                == find_inventory_file(&inventory, "/SLPM_624.00;1")?
                    .sha256
                    .as_deref()
                    .context("executable inventory entry has no SHA-256")?,
        "graphics ledger differs from the verified technical-build source"
    );
    validate_graphics_translation_asset_files(&graphics)?;
    let story = load_story_dialogue_translation_set(&command.story_translation)?;
    validate_story_dialogue_translation_bundle(&story, &graphics)?;
    let static_catalog: StaticRegionCatalog = read_json_file(&command.static_catalog)?;
    let texture_previews: TexturePreviewCorpus = read_json_file(&command.texture_previews)?;
    verify_texture_preview_files(&texture_previews)?;

    let report = build_technical_translation_development(TechnicalTranslationBuildInputs {
        inventory: &inventory,
        source_bin_path: &command.source.bin,
        executable: &executable,
        image_d: &image_d,
        image_g: &image_g,
        image_md: &image_md,
        memory_card_translation_path: &command.memory_card_translation,
        executable_text_translation_path: &command.executable_text_translation,
        approved_terminology_path: &command.approved_terminology,
        graphics_translation_path: &command.graphics_translation,
        graphics: &graphics,
        story_translation_path: &command.story_translation,
        story: &story,
        shared_font_config_path: &command.shared_font_config,
        story_font_config_path: &command.story_font_config,
        shared_font_path: &command.shared_font,
        story_font_path: &command.story_font,
        ranking_asset_path: &command.ranking_asset,
        static_catalog_path: &command.static_catalog,
        static_catalog: &static_catalog,
        texture_previews_path: &command.texture_previews,
        texture_previews: &texture_previews,
        output_bin_path: &command.output_bin,
        output_cue_path: &command.output_cue,
    })?;
    write_json_file(&command.report, &report)?;
    println!(
        "built {} technical translation components across {} merged raw sectors to {} ({}); wrote {}",
        report.included_components,
        report.merged_raw_sectors,
        command.output_bin.display(),
        report.output_bin_sha256,
        command.report.display()
    );
    Ok(())
}
