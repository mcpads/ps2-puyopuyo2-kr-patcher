use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::cpx::survey_cpx_container;
use puyo2_ps2_tool::decode_linear_indices_at_cpx_offset;
use puyo2_ps2_tool::graphics_translation::PixelBounds;
use puyo2_ps2_tool::graphics_translation::palette_index_census::{
    PaletteIndexCensusInputs, survey_palette_indices,
};
use puyo2_ps2_tool::graphics_translation::static_regions::{
    StaticRegionCatalog, inventory_static_regions,
};
use puyo2_ps2_tool::graphics_translation::{
    GraphicsTranslationBundle, load_graphics_translation_set, write_graphics_translation_set,
};
use puyo2_ps2_tool::source::{
    DiscInventory, find_inventory_file, read_inventory_file, read_json_file, write_json_file,
};
use puyo2_ps2_tool::texture_language_survey::verify_texture_preview_files;
use puyo2_ps2_tool::texture_preview_corpus::TexturePreviewCorpus;
use serde::Serialize;

use super::{SourceArgs, inspect};

pub(super) struct StaticRegionCommand {
    pub source: SourceArgs,
    pub graphics_translation: PathBuf,
    pub catalog: PathBuf,
    pub texture_previews: PathBuf,
    pub output_directory: PathBuf,
    pub report: PathBuf,
    pub refresh_existing: bool,
}

pub(super) struct StaticRegionPaletteCensusCommand {
    pub source: SourceArgs,
    pub graphics_translation: PathBuf,
    pub texture_previews: PathBuf,
    pub source_surface_id: String,
    pub preview_png_sha256: String,
    pub rectangles: Vec<StaticRegionCensusRectangle>,
    pub output: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) struct StaticRegionCensusRectangle {
    pub name: String,
    pub bounds: PixelBounds,
}

impl FromStr for StaticRegionCensusRectangle {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let fields = value.split(':').collect::<Vec<_>>();
        if fields.len() != 5 || fields[0].trim().is_empty() {
            return Err("rectangle must be NAME:X:Y:WIDTH:HEIGHT".to_string());
        }
        let parse = |field: &str, role: &str| {
            field
                .parse::<u32>()
                .map_err(|error| format!("invalid rectangle {role}: {error}"))
        };
        Ok(Self {
            name: fields[0].to_string(),
            bounds: PixelBounds {
                x: parse(fields[1], "x")?,
                y: parse(fields[2], "y")?,
                width: parse(fields[3], "width")?,
                height: parse(fields[4], "height")?,
            },
        })
    }
}

#[derive(Serialize)]
struct NamedPaletteIndexCensus {
    name: String,
    census: puyo2_ps2_tool::graphics_translation::palette_index_census::PaletteIndexCensus,
}

#[derive(Serialize)]
struct StaticRegionPaletteCensusBatch {
    schema: u32,
    source_surface_id: String,
    preview_png_sha256: String,
    rectangles: Vec<NamedPaletteIndexCensus>,
    evidence_boundary: String,
}

pub(super) fn run_static_region_inventory(command: StaticRegionCommand) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let mut graphics = load_graphics_translation_set(&command.graphics_translation)?;
    validate_graphics_source(&inventory, &graphics)?;
    let catalog: StaticRegionCatalog = read_json_file(&command.catalog)?;
    let previews: TexturePreviewCorpus = read_json_file(&command.texture_previews)?;
    verify_texture_preview_files(&previews)?;
    let source_linear_indices =
        load_source_linear_indices(&command.source, &inventory, &graphics, &catalog)?;
    let report = inventory_static_regions(
        &mut graphics,
        &catalog,
        &previews,
        &source_linear_indices,
        &command.output_directory,
        command.refresh_existing,
    )?;
    write_graphics_translation_set(&command.graphics_translation, &graphics)?;
    write_json_file(&command.report, &report)?;
    println!(
        "inventoried {} Japanese regions across {} Japanese-only or mixed-language static graphics surfaces; wrote masks to {} and report {}",
        report.inventoried_regions,
        report.inventoried_surfaces,
        command.output_directory.display(),
        command.report.display()
    );
    Ok(())
}

pub(super) fn run_static_region_palette_census(
    command: StaticRegionPaletteCensusCommand,
) -> Result<()> {
    let inventory = inspect(&command.source)?;
    let graphics = load_graphics_translation_set(&command.graphics_translation)?;
    validate_graphics_source(&inventory, &graphics)?;
    let target = graphics
        .entries
        .iter()
        .find(|entry| entry.source.source_surface_id == command.source_surface_id)
        .with_context(|| {
            format!(
                "graphics ledger has no source surface: {}",
                command.source_surface_id
            )
        })?;
    let previews: TexturePreviewCorpus = read_json_file(&command.texture_previews)?;
    verify_texture_preview_files(&previews)?;
    let matching_previews = previews
        .previews
        .iter()
        .filter(|preview| {
            preview.image_container_iso_path == target.source.container_iso_path
                && preview.image_file_record == target.source.image_file_record
                && preview.width == target.source.width
                && preview.height == target.source.height
                && preview.linear_indices_sha256 == target.source.linear_indices_sha256
                && preview.png_sha256 == command.preview_png_sha256
        })
        .collect::<Vec<_>>();
    ensure!(
        matching_previews.len() == 1,
        "palette-index census requires one exact preview for {} and hash {}",
        command.source_surface_id,
        command.preview_png_sha256
    );
    let preview = matching_previews[0];
    let container = read_inventory_file(
        &command.source.bin,
        &inventory,
        &target.source.container_iso_path,
    )?;
    let survey = survey_cpx_container(&target.source.container_iso_path, &container)?;
    let block = survey
        .blocks
        .get(target.source.image_file_record)
        .context("palette-index census image record is outside the CPX prefix")?;
    let linear_indices = decode_linear_indices_at_cpx_offset(
        &container,
        block.offset,
        target.source.width,
        target.source.height,
    )?;
    ensure!(
        !command.rectangles.is_empty(),
        "palette-index census has no rectangles"
    );
    let mut seen_names = BTreeSet::new();
    let rectangles = command
        .rectangles
        .into_iter()
        .map(|rectangle| {
            ensure!(
                seen_names.insert(rectangle.name.clone()),
                "duplicate palette-index census rectangle name: {}",
                rectangle.name
            );
            Ok(NamedPaletteIndexCensus {
                name: rectangle.name,
                census: survey_palette_indices(PaletteIndexCensusInputs {
                    source_surface_id: &target.source.source_surface_id,
                    preview_png_path: preview.png_path.as_ref(),
                    preview_png_sha256: &preview.png_sha256,
                    linear_indices_sha256: &target.source.linear_indices_sha256,
                    linear_indices: &linear_indices,
                    width: target.source.width,
                    height: target.source.height,
                    bounds: rectangle.bounds,
                })?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let report = StaticRegionPaletteCensusBatch {
        schema: 1,
        source_surface_id: target.source.source_surface_id.clone(),
        preview_png_sha256: preview.png_sha256.clone(),
        rectangles,
        evidence_boundary: "Each named rectangle is an exact-source palette-index census, not an editable-region classification, translation approval, runtime observation, or release-readiness claim.".to_string(),
    };
    write_json_file(&command.output, &report)?;
    println!(
        "surveyed {} palette-index rectangles for {} to {}",
        report.rectangles.len(),
        report.source_surface_id,
        command.output.display()
    );
    Ok(())
}

fn validate_graphics_source(
    inventory: &DiscInventory,
    graphics: &GraphicsTranslationBundle,
) -> Result<()> {
    ensure!(
        graphics.source.source_bin_sha256 == inventory.source.bin.sha256,
        "static graphics ledger differs from the verified source BIN"
    );
    let executable_sha256 = find_inventory_file(inventory, &graphics.source.executable_iso_path)?
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
        "static graphics ledger differs from the verified executable"
    );
    Ok(())
}

fn load_source_linear_indices(
    source: &SourceArgs,
    inventory: &DiscInventory,
    graphics: &GraphicsTranslationBundle,
    catalog: &StaticRegionCatalog,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let entries = graphics
        .entries
        .iter()
        .map(|entry| (entry.source.source_surface_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let container_paths = catalog
        .surfaces
        .iter()
        .map(|declaration| {
            entries
                .get(declaration.source_surface_id.as_str())
                .map(|entry| entry.source.container_iso_path.clone())
                .with_context(|| {
                    format!(
                        "graphics ledger has no declared static surface: {}",
                        declaration.source_surface_id
                    )
                })
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let mut containers = BTreeMap::new();
    for iso_path in container_paths {
        let bytes = read_inventory_file(&source.bin, inventory, &iso_path)?;
        let survey = survey_cpx_container(&iso_path, &bytes)?;
        containers.insert(iso_path, (bytes, survey));
    }

    let mut surfaces = BTreeMap::new();
    for declaration in &catalog.surfaces {
        let entry = entries
            .get(declaration.source_surface_id.as_str())
            .context("static graphics entry disappeared while loading source indices")?;
        let (container, survey) = containers
            .get(&entry.source.container_iso_path)
            .context("static graphics container was not loaded")?;
        let block = survey
            .blocks
            .get(entry.source.image_file_record)
            .with_context(|| {
                format!(
                    "static graphics image record is outside the CPX prefix: {}",
                    declaration.source_surface_id
                )
            })?;
        let indices = decode_linear_indices_at_cpx_offset(
            container,
            block.offset,
            entry.source.width,
            entry.source.height,
        )?;
        ensure!(
            surfaces
                .insert(declaration.source_surface_id.clone(), indices)
                .is_none(),
            "duplicate static graphics source indices: {}",
            declaration.source_surface_id
        );
    }
    Ok(surfaces)
}
