use std::path::PathBuf;

use anyhow::Result;
use puyo2_ps2_tool::graphics_translation::load_graphics_translation_set;
use puyo2_ps2_tool::graphics_translation::static_regions::StaticRegionCatalog;
use puyo2_ps2_tool::source::{read_inventory_file, read_json_file, write_json_file};
use puyo2_ps2_tool::static_ui_translation_build::{
    StaticGraphicsOverlayScope, StaticUiTranslationBuildInputs,
    build_static_ui_translation_development,
};
use puyo2_ps2_tool::texture_language_survey::verify_texture_preview_files;
use puyo2_ps2_tool::texture_preview_corpus::TexturePreviewCorpus;

use super::{SourceArgs, inspect};

pub(super) struct StaticUiTranslationBuildCommand {
    pub source: SourceArgs,
    pub graphics_translation: PathBuf,
    pub catalog: PathBuf,
    pub texture_previews: PathBuf,
    pub output_bin: PathBuf,
    pub output_cue: PathBuf,
    pub preview_directory: PathBuf,
    pub report: PathBuf,
    pub scope: StaticGraphicsOverlayScope,
}

pub(super) fn run_static_ui_translation_build(
    command: StaticUiTranslationBuildCommand,
) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let executable = read_inventory_file(&command.source.bin, &inventory, "/SLPM_624.00;1")?;
    let image_d = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_D.BIN;1")?;
    let image_g = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_G.BIN;1")?;
    let image_md = read_inventory_file(&command.source.bin, &inventory, "/IMAGE_MD.BIN;1")?;
    let graphics = load_graphics_translation_set(&command.graphics_translation)?;
    let catalog: StaticRegionCatalog = read_json_file(&command.catalog)?;
    let previews: TexturePreviewCorpus = read_json_file(&command.texture_previews)?;
    verify_texture_preview_files(&previews)?;
    let report = build_static_ui_translation_development(StaticUiTranslationBuildInputs {
        inventory: &inventory,
        source_bin_path: &command.source.bin,
        executable: &executable,
        image_d: &image_d,
        image_g: &image_g,
        image_md: &image_md,
        graphics: &graphics,
        catalog: &catalog,
        previews: &previews,
        output_bin_path: &command.output_bin,
        output_cue_path: &command.output_cue,
        preview_directory: &command.preview_directory,
        scope: command.scope,
    })?;
    write_json_file(&command.report, &report)?;
    println!(
        "built {} {:?} surfaces and {} regions across {} changed raw sectors to {} ({}); wrote {}",
        report.built_surfaces,
        report.scope,
        report.built_regions,
        report.raw_sector_plans,
        command.output_bin.display(),
        report.output_bin_sha256,
        command.report.display()
    );
    Ok(())
}
