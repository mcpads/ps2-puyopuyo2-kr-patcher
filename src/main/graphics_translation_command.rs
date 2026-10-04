use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use puyo2_ps2_tool::graphics_translation::{
    GraphicsTranslationInputs, compose_graphics_translation_bundle, load_graphics_translation_set,
    validate_graphics_translation_asset_files, validate_graphics_translation_bundle,
    write_new_graphics_translation_set,
};
use puyo2_ps2_tool::source::{DiscInventory, find_inventory_file, read_json_file};
use puyo2_ps2_tool::translation_targets::LinkedTextureTarget;
use serde::Deserialize;

use super::{SourceArgs, inspect};

pub(super) struct GraphicsTranslationCommand {
    pub source: SourceArgs,
    pub target_map: PathBuf,
    pub translation: PathBuf,
}

#[derive(Debug, Deserialize)]
struct GraphicsTargetMap {
    schema: u32,
    source_bin_sha256: String,
    executable_iso_path: String,
    executable_sha256: String,
    linked_texture_targets: Vec<LinkedTextureTarget>,
}

impl GraphicsTargetMap {
    fn inputs(&self) -> GraphicsTranslationInputs<'_> {
        GraphicsTranslationInputs {
            source_bin_sha256: &self.source_bin_sha256,
            executable_iso_path: &self.executable_iso_path,
            executable_sha256: &self.executable_sha256,
            linked_texture_targets: &self.linked_texture_targets,
        }
    }
}

pub(super) fn run_graphics_translation_init(command: GraphicsTranslationCommand) -> Result<()> {
    let target_map = load_verified_target_map(&command)?;
    let bundle = compose_graphics_translation_bundle(target_map.inputs())?;
    write_new_graphics_translation_set(&command.translation, &bundle)?;
    println!(
        "initialized {} exact-source linked texture surfaces at {}; semantic and Japanese-region inventories remain explicit authoring work",
        bundle.population.linked_texture_surfaces,
        command.translation.display()
    );
    Ok(())
}

pub(super) fn run_graphics_translation_check(command: GraphicsTranslationCommand) -> Result<()> {
    let target_map = load_verified_target_map(&command)?;
    let bundle = load_graphics_translation_set(&command.translation)?;
    let validation = validate_graphics_translation_bundle(&bundle, target_map.inputs())?;
    let referenced_assets = validate_graphics_translation_asset_files(&bundle)?;
    println!(
        "validated {} product linked-texture surfaces and {} referenced authoring assets: {} need semantic inventory, {} need Japanese-region inventory, {} in progress, {} awaiting human review, {} complete; {} release-candidate eligible",
        validation.linked_texture_surfaces,
        referenced_assets,
        validation.needs_semantic_inventory,
        validation.needs_region_inventory,
        validation.in_progress,
        validation.needs_human_review,
        validation.complete,
        validation.release_candidate_eligible,
    );
    Ok(())
}

fn load_verified_target_map(command: &GraphicsTranslationCommand) -> Result<GraphicsTargetMap> {
    let inventory = inspect(&command.source)?;
    let target_map: GraphicsTargetMap = read_json_file(&command.target_map)?;
    verify_target_map_identity(&inventory, &target_map)?;
    Ok(target_map)
}

fn verify_target_map_identity(
    inventory: &DiscInventory,
    target_map: &GraphicsTargetMap,
) -> Result<()> {
    ensure!(
        target_map.schema == 1,
        "unsupported translation target schema"
    );
    ensure!(
        target_map.source_bin_sha256 == inventory.source.bin.sha256,
        "graphics target map differs from the verified source BIN"
    );
    let executable_sha256 = find_inventory_file(inventory, &target_map.executable_iso_path)?
        .sha256
        .as_deref()
        .with_context(|| {
            format!(
                "executable inventory entry has no SHA-256: {}",
                target_map.executable_iso_path
            )
        })?;
    ensure!(
        target_map.executable_sha256 == executable_sha256,
        "graphics target map differs from the verified executable"
    );
    ensure!(
        !target_map.linked_texture_targets.is_empty(),
        "graphics target map has no linked texture targets"
    );
    Ok(())
}
