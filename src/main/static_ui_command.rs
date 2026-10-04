use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::static_regions::StaticRegionCatalog;
use puyo2_ps2_tool::graphics_translation::static_ui::{
    StaticTextDraftInputs, StaticTextDraftScope, render_static_text_drafts,
};
use puyo2_ps2_tool::graphics_translation::{
    load_graphics_translation_set, write_graphics_translation_set,
};
use puyo2_ps2_tool::source::{find_inventory_file, read_json_file, write_json_file};
use puyo2_ps2_tool::texture_language_survey::verify_texture_preview_files;
use puyo2_ps2_tool::texture_preview_corpus::TexturePreviewCorpus;

use super::{SourceArgs, inspect};

pub(super) struct StaticTextDraftCommand {
    pub surfaces: Vec<String>,
    pub source: SourceArgs,
    pub graphics_translation: PathBuf,
    pub catalog: PathBuf,
    pub texture_previews: PathBuf,
    pub font_config: PathBuf,
    pub font: PathBuf,
    pub overlay_directory: PathBuf,
    pub preview_directory: PathBuf,
    pub report: PathBuf,
    pub refresh_existing: bool,
    pub scope: StaticTextDraftScope,
}

pub(super) fn run_static_text_draft_render(command: StaticTextDraftCommand) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let mut graphics = load_graphics_translation_set(&command.graphics_translation)?;
    ensure!(
        graphics.source.source_bin_sha256 == inventory.source.bin.sha256,
        "static text graphics ledger differs from the verified source BIN"
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
        "static text graphics ledger differs from the verified executable"
    );
    let mut catalog: StaticRegionCatalog = read_json_file(&command.catalog)?;
    for selected in &command.surfaces {
        ensure!(
            catalog
                .surfaces
                .iter()
                .any(|surface| &surface.source_surface_id == selected),
            "static text surface is not in the catalog: {selected}"
        );
    }
    if !command.surfaces.is_empty() {
        catalog
            .surfaces
            .retain(|surface| command.surfaces.contains(&surface.source_surface_id));
    }
    let previews: TexturePreviewCorpus = read_json_file(&command.texture_previews)?;
    verify_texture_preview_files(&previews)?;
    let report = render_static_text_drafts(StaticTextDraftInputs {
        graphics: &mut graphics,
        catalog: &catalog,
        previews: &previews,
        font_config_path: &command.font_config,
        font_path: &command.font,
        overlay_directory: &command.overlay_directory,
        preview_directory: &command.preview_directory,
        refresh_existing: command.refresh_existing,
        scope: command.scope,
    })?;
    write_graphics_translation_set(&command.graphics_translation, &graphics)?;
    write_json_file(&command.report, &report)?;
    println!(
        "rendered {} {:?} regions across {} product surfaces; deferred {} and skipped {} surfaces; wrote role overlays to {} and report {}",
        report.rendered_regions,
        report.scope,
        report.rendered_surfaces,
        report.deferred_surfaces.len(),
        report.skipped_surfaces.len(),
        command.overlay_directory.display(),
        command.report.display()
    );
    Ok(())
}
