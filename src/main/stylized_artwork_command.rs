use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::static_regions::StaticRegionCatalog;
use puyo2_ps2_tool::graphics_translation::stylized_artwork::{
    StylizedArtworkAsset, StylizedArtworkDraftInputs, StylizedArtworkSetAsset,
    StylizedArtworkSetDraftInputs, render_stylized_artwork_draft,
    render_stylized_artwork_set_draft,
};
use puyo2_ps2_tool::graphics_translation::{
    load_graphics_translation_set, write_graphics_translation_set,
};
use puyo2_ps2_tool::source::{read_inventory_file, read_json_file, write_json_file};
use puyo2_ps2_tool::texture_language_survey::verify_texture_preview_files;
use puyo2_ps2_tool::texture_preview_corpus::TexturePreviewCorpus;

use super::{SourceArgs, inspect};

pub(super) struct StylizedArtworkCommand {
    pub source: SourceArgs,
    pub graphics_translation: PathBuf,
    pub catalog: PathBuf,
    pub texture_previews: PathBuf,
    pub asset: PathBuf,
    pub native_preview: PathBuf,
    pub surface_preview: PathBuf,
    pub report: PathBuf,
    pub refresh_existing: bool,
}

pub(super) struct StylizedArtworkSetCommand {
    pub source: SourceArgs,
    pub graphics_translation: PathBuf,
    pub catalog: PathBuf,
    pub texture_previews: PathBuf,
    pub asset: PathBuf,
    pub native_preview_directory: PathBuf,
    pub surface_preview: PathBuf,
    pub report: PathBuf,
    pub refresh_existing: bool,
}

pub(super) fn run_stylized_artwork_render(command: StylizedArtworkCommand) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let mut graphics = load_graphics_translation_set(&command.graphics_translation)?;
    let catalog: StaticRegionCatalog = read_json_file(&command.catalog)?;
    let previews: TexturePreviewCorpus = read_json_file(&command.texture_previews)?;
    verify_texture_preview_files(&previews)?;
    let asset: StylizedArtworkAsset = read_json_file(&command.asset)?;
    let target = graphics
        .entries
        .iter()
        .find(|entry| entry.source.source_surface_id == asset.source_surface_id)
        .with_context(|| {
            format!(
                "graphics ledger has no stylized artwork target: {}",
                asset.source_surface_id
            )
        })?;
    ensure!(
        graphics.source.source_bin_sha256 == inventory.source.bin.sha256,
        "stylized artwork ledger differs from the verified source BIN"
    );
    let executable = read_inventory_file(
        &command.source.bin,
        &inventory,
        &graphics.source.executable_iso_path,
    )?;
    let image_container = read_inventory_file(
        &command.source.bin,
        &inventory,
        &target.source.container_iso_path,
    )?;
    let report = render_stylized_artwork_draft(StylizedArtworkDraftInputs {
        graphics: &mut graphics,
        catalog: &catalog,
        previews: &previews,
        executable: &executable,
        image_container: &image_container,
        asset_path: &command.asset,
        asset: &asset,
        native_preview_path: &command.native_preview,
        surface_preview_path: &command.surface_preview,
        refresh_existing: command.refresh_existing,
    })?;
    write_graphics_translation_set(&command.graphics_translation, &graphics)?;
    write_json_file(&command.report, &report)?;
    println!(
        "rendered {} as consumer-aligned {} native pixels with {} protected-pixel changes; wrote overlay {}, preview {}, and report {}",
        report.korean_text,
        report.korean_overlay_pixels,
        report.changed_protected_indices,
        report.replacement_overlay.path,
        command.surface_preview.display(),
        command.report.display()
    );
    Ok(())
}

pub(super) fn run_stylized_artwork_set_render(command: StylizedArtworkSetCommand) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let mut graphics = load_graphics_translation_set(&command.graphics_translation)?;
    let catalog: StaticRegionCatalog = read_json_file(&command.catalog)?;
    let previews: TexturePreviewCorpus = read_json_file(&command.texture_previews)?;
    verify_texture_preview_files(&previews)?;
    let asset: StylizedArtworkSetAsset = read_json_file(&command.asset)?;
    let target = graphics
        .entries
        .iter()
        .find(|entry| entry.source.source_surface_id == asset.source_surface_id)
        .with_context(|| {
            format!(
                "graphics ledger has no stylized artwork set target: {}",
                asset.source_surface_id
            )
        })?;
    ensure!(
        graphics.source.source_bin_sha256 == inventory.source.bin.sha256,
        "stylized artwork set ledger differs from the verified source BIN"
    );
    let executable = read_inventory_file(
        &command.source.bin,
        &inventory,
        &graphics.source.executable_iso_path,
    )?;
    let image_container = read_inventory_file(
        &command.source.bin,
        &inventory,
        &target.source.container_iso_path,
    )?;
    let report = render_stylized_artwork_set_draft(StylizedArtworkSetDraftInputs {
        graphics: &mut graphics,
        catalog: &catalog,
        previews: &previews,
        executable: &executable,
        image_container: &image_container,
        asset_path: &command.asset,
        asset: &asset,
        native_preview_directory: &command.native_preview_directory,
        surface_preview_path: &command.surface_preview,
        refresh_existing: command.refresh_existing,
    })?;
    write_graphics_translation_set(&command.graphics_translation, &graphics)?;
    write_json_file(&command.report, &report)?;
    println!(
        "rendered {} artwork members with {} native pixels and {} protected-pixel changes; wrote overlay {}, preview {}, and report {}",
        report.members.len(),
        report.korean_overlay_pixels,
        report.changed_protected_indices,
        report.replacement_overlay.path,
        command.surface_preview.display(),
        command.report.display()
    );
    Ok(())
}
