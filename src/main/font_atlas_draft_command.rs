use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::font_atlas_draft::{
    FontAtlasDraftInputs, render_font_atlas_drafts,
};
use puyo2_ps2_tool::graphics_translation::font_atlas_layout::FontAtlasLayout;
use puyo2_ps2_tool::graphics_translation::static_regions::StaticRegionCatalog;
use puyo2_ps2_tool::graphics_translation::{
    load_graphics_translation_set, write_graphics_translation_set,
};
use puyo2_ps2_tool::source::{
    find_inventory_file, read_inventory_file, read_json_file, write_json_file,
};
use puyo2_ps2_tool::texture_language_survey::verify_texture_preview_files;
use puyo2_ps2_tool::texture_preview_corpus::TexturePreviewCorpus;

use super::{SourceArgs, inspect};

pub(super) struct FontAtlasDraftCommand {
    pub source: SourceArgs,
    pub graphics_translation: PathBuf,
    pub catalog: PathBuf,
    pub layout: PathBuf,
    pub texture_previews: PathBuf,
    pub font_config: PathBuf,
    pub font: PathBuf,
    pub overlay_directory: PathBuf,
    pub preview_directory: PathBuf,
    pub report: PathBuf,
    pub refresh_existing: bool,
}

pub(super) fn run_font_atlas_draft_render(command: FontAtlasDraftCommand) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let executable = read_inventory_file(&command.source.bin, &inventory, "/SLPM_624.00;1")?;
    let mut graphics = load_graphics_translation_set(&command.graphics_translation)?;
    ensure!(
        graphics.source.source_bin_sha256 == inventory.source.bin.sha256,
        "font-atlas graphics ledger differs from the verified source BIN"
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
        "font-atlas graphics ledger differs from the verified executable"
    );
    let catalog: StaticRegionCatalog = read_json_file(&command.catalog)?;
    let layout: FontAtlasLayout = read_json_file(&command.layout)?;
    let previews: TexturePreviewCorpus = read_json_file(&command.texture_previews)?;
    verify_texture_preview_files(&previews)?;
    let report = render_font_atlas_drafts(FontAtlasDraftInputs {
        graphics: &mut graphics,
        catalog: &catalog,
        layout: &layout,
        previews: &previews,
        executable: &executable,
        font_config_path: &command.font_config,
        font_path: &command.font,
        overlay_directory: &command.overlay_directory,
        preview_directory: &command.preview_directory,
        refresh_existing: command.refresh_existing,
    })?;
    write_graphics_translation_set(&command.graphics_translation, &graphics)?;
    write_json_file(&command.report, &report)?;
    println!(
        "rendered {} font-atlas placements across {} regions and {} surfaces; wrote role overlays to {} and report {}",
        report.rendered_placements,
        report.rendered_regions,
        report.rendered_surfaces,
        command.overlay_directory.display(),
        command.report.display()
    );
    Ok(())
}
