use std::collections::BTreeMap;

use anyhow::{Result, ensure};

use crate::translation_targets::LinkedTextureTarget;

pub(crate) mod asset_naming;
pub mod deterministic_label;
pub mod font_atlas_draft;
pub mod font_atlas_layout;
mod model;
pub mod palette_index_census;
pub mod ranking_labels;
pub(crate) mod raster;
pub mod static_regions;
pub mod static_ui;
pub mod story_dialogue_regions;
pub mod stylized_artwork;
pub mod title_draft;
mod translation_set;
mod validation;

pub use model::*;
pub use translation_set::{
    graphics_translation_set_files, load_graphics_translation_set, write_graphics_translation_set,
    write_new_graphics_translation_set,
};
pub use validation::{
    validate_graphics_translation_asset_files, validate_graphics_translation_bundle,
};

pub struct GraphicsTranslationInputs<'a> {
    pub source_bin_sha256: &'a str,
    pub executable_iso_path: &'a str,
    pub executable_sha256: &'a str,
    pub linked_texture_targets: &'a [LinkedTextureTarget],
}

pub fn compose_graphics_translation_bundle(
    inputs: GraphicsTranslationInputs<'_>,
) -> Result<GraphicsTranslationBundle> {
    ensure!(
        !inputs.linked_texture_targets.is_empty(),
        "graphics translation target population is empty"
    );

    let mut entries = BTreeMap::new();
    for target in inputs.linked_texture_targets {
        let source = source_from_target(target)?;
        ensure!(
            entries
                .insert(
                    source.source_surface_id.clone(),
                    GraphicsTranslationEntry {
                        source,
                        authoring: GraphicsAuthoring {
                            status: GraphicsTranslationStatus::NeedsSemanticInventory,
                            semantic_asset_id: None,
                            semantic_role: None,
                            continuity_group: None,
                            design_candidates: Vec::new(),
                            region_inventory_complete: false,
                            protected_nontext_masks: Vec::new(),
                            protected_english_mask: None,
                            replacement_image: None,
                            replacement_overlay: None,
                            regions: Vec::new(),
                            translator_notes: Vec::new(),
                            human_approval: None,
                            independent_review: None,
                        },
                    },
                )
                .is_none(),
            "duplicate graphics source surface"
        );
    }

    Ok(GraphicsTranslationBundle {
        schema: 1,
        source: GraphicsTranslationBundleSource {
            source_bin_sha256: inputs.source_bin_sha256.to_string(),
            executable_iso_path: inputs.executable_iso_path.to_string(),
            executable_sha256: inputs.executable_sha256.to_string(),
        },
        scope: GraphicsTranslationScope {
            target: "Japanese text baked into every executable-linked IMAGE_D, IMAGE_G, and IMAGE_MD texture target"
                .to_string(),
            preserve: "all English/ASCII artwork and every pixel outside an admitted Japanese region; dimensions, palette-index semantics, consumer ownership, and container identity"
                .to_string(),
            authoring_policy: "source surfaces are physical evidence locators, while semantic asset IDs, roles, continuity groups, and per-region treatment profiles are separately curated"
                .to_string(),
            build_policy: "review state controls release-candidate eligibility, not explicit non-distribution development builds; insertion must verify protected pixels and exact-source Expected Writes"
                .to_string(),
            release_candidate: false,
        },
        population: GraphicsTranslationPopulation {
            linked_texture_surfaces: entries.len(),
        },
        entries: entries.into_values().collect(),
    })
}

fn source_from_target(target: &LinkedTextureTarget) -> Result<GraphicsTranslationSource> {
    ensure!(
        target.width > 0 && target.height > 0,
        "graphics target has zero dimensions: {}#{}",
        target.container_iso_path,
        target.image_file_record
    );
    ensure!(
        !target.owner_tables.is_empty(),
        "graphics target has no admitted consumer: {}#{}",
        target.container_iso_path,
        target.image_file_record
    );
    let container_id = target
        .container_iso_path
        .trim_start_matches('/')
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace('_', "-");
    ensure!(
        !container_id.is_empty(),
        "graphics target has no stable container identity"
    );
    Ok(GraphicsTranslationSource {
        source_surface_id: format!(
            "linked-texture/{container_id}/{:06}",
            target.image_file_record
        ),
        container_iso_path: target.container_iso_path.clone(),
        image_file_record: target.image_file_record,
        width: target.width,
        height: target.height,
        linear_indices_sha256: target.linear_indices_sha256.clone(),
        classification: target.classification,
        requires_english_pixel_preservation: target.requires_english_pixel_preservation,
        owner_tables: target.owner_tables.clone(),
    })
}

#[cfg(test)]
#[path = "graphics_translation/graphics_translation_tests.rs"]
mod graphics_translation_tests;
