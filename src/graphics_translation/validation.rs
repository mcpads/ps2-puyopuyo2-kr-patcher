use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{Result, ensure};

use super::{
    GlyphDemandSource, GraphicsArtworkMethod, GraphicsArtworkPlacementStatus, GraphicsArtworkPlan,
    GraphicsAuthoring, GraphicsTranslationBundle, GraphicsTranslationInputs,
    GraphicsTranslationStatus, GraphicsTranslationValidation, GraphicsTreatmentProfile,
    compose_graphics_translation_bundle,
};
use crate::indexed_texture::sha256;

pub fn validate_graphics_translation_asset_files(
    bundle: &GraphicsTranslationBundle,
) -> Result<usize> {
    let mut assets = BTreeMap::<&str, &str>::new();
    for entry in &bundle.entries {
        let authoring = &entry.authoring;
        let referenced = authoring
            .design_candidates
            .iter()
            .chain(authoring.protected_nontext_masks.iter())
            .chain(authoring.protected_english_mask.iter())
            .chain(authoring.replacement_image.iter())
            .chain(authoring.replacement_overlay.iter())
            .chain(
                authoring
                    .regions
                    .iter()
                    .map(|region| &region.source_pixel_mask),
            );
        for asset in referenced {
            if let Some(previous) = assets.insert(&asset.path, &asset.sha256) {
                ensure!(
                    previous == asset.sha256,
                    "graphics asset path has conflicting hashes: {}",
                    asset.path
                );
            }
        }
    }
    for (path, expected_sha256) in &assets {
        let bytes = fs::read(path)
            .map_err(|error| anyhow::anyhow!("failed to read graphics asset {path}: {error}"))?;
        ensure!(
            sha256(&bytes) == *expected_sha256,
            "graphics asset SHA-256 differs: {path}"
        );
    }
    Ok(assets.len())
}

pub fn validate_graphics_translation_bundle(
    bundle: &GraphicsTranslationBundle,
    inputs: GraphicsTranslationInputs<'_>,
) -> Result<GraphicsTranslationValidation> {
    let expected = compose_graphics_translation_bundle(inputs)?;
    ensure!(
        bundle.schema == 1,
        "unsupported graphics translation schema"
    );
    ensure!(
        bundle.source == expected.source
            && bundle.scope == expected.scope
            && bundle.population == expected.population,
        "graphics translation bundle header differs from the exact-source target baseline"
    );
    ensure!(
        bundle.entries.len() == expected.entries.len(),
        "graphics translation bundle does not cover every linked texture target"
    );

    let expected_sources = expected
        .entries
        .into_iter()
        .map(|entry| (entry.source.source_surface_id.clone(), entry.source))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut previous_id: Option<&str> = None;
    let mut validation = GraphicsTranslationValidation {
        linked_texture_surfaces: bundle.entries.len(),
        needs_semantic_inventory: 0,
        needs_region_inventory: 0,
        in_progress: 0,
        needs_human_review: 0,
        complete: 0,
        release_candidate_eligible: 0,
    };

    for entry in &bundle.entries {
        let source_id = entry.source.source_surface_id.as_str();
        ensure!(
            previous_id.is_none_or(|previous| previous < source_id),
            "graphics translation entries are not in stable source-surface order"
        );
        previous_id = Some(source_id);
        ensure!(
            seen.insert(source_id),
            "duplicate graphics translation entry: {source_id}"
        );
        let expected_source = expected_sources
            .get(source_id)
            .ok_or_else(|| anyhow::anyhow!("unexpected graphics translation entry: {source_id}"))?;
        ensure!(
            &entry.source == expected_source,
            "protected source fields differ for graphics translation entry: {source_id}"
        );
        validate_authoring(
            entry.source.width,
            entry.source.height,
            entry.source.requires_english_pixel_preservation,
            &entry.authoring,
            source_id,
        )?;
        match entry.authoring.status {
            GraphicsTranslationStatus::NeedsSemanticInventory => {
                validation.needs_semantic_inventory += 1;
            }
            GraphicsTranslationStatus::NeedsRegionInventory => {
                validation.needs_region_inventory += 1;
            }
            GraphicsTranslationStatus::InProgress => validation.in_progress += 1,
            GraphicsTranslationStatus::NeedsHumanReview => {
                validation.needs_human_review += 1;
            }
            GraphicsTranslationStatus::Complete => validation.complete += 1,
        }
    }
    ensure!(
        seen.len() == expected_sources.len(),
        "graphics translation bundle is missing an exact-source target"
    );
    Ok(validation)
}

fn validate_authoring(
    surface_width: u32,
    surface_height: u32,
    requires_english_pixel_preservation: bool,
    authoring: &GraphicsAuthoring,
    source_id: &str,
) -> Result<()> {
    let semantic_fields = [
        authoring.semantic_asset_id.as_deref(),
        authoring.semantic_role.as_deref(),
        authoring.continuity_group.as_deref(),
    ];
    let has_all_semantic_fields = semantic_fields
        .iter()
        .all(|field| field.is_some_and(|value| !value.trim().is_empty()));
    let has_functional_regions = authoring
        .regions
        .iter()
        .any(|region| region.treatment_profile == GraphicsTreatmentProfile::FunctionalUiText);
    let has_nonfunctional_regions = authoring
        .regions
        .iter()
        .any(|region| region.treatment_profile != GraphicsTreatmentProfile::FunctionalUiText);
    let has_bounded_partial_overlay = authoring.replacement_overlay.is_some()
        && has_functional_regions
        && has_nonfunctional_regions;
    let has_rough_artwork = authoring.regions.iter().any(|region| {
        region.artwork_plan.is_some_and(|plan| {
            plan.placement_status == GraphicsArtworkPlacementStatus::RoughRegionBounds
        })
    });

    match authoring.status {
        GraphicsTranslationStatus::NeedsSemanticInventory => ensure!(
            !authoring.region_inventory_complete
                && authoring.regions.is_empty()
                && authoring.design_candidates.is_empty()
                && authoring.replacement_image.is_none()
                && authoring.replacement_overlay.is_none()
                && authoring.human_approval.is_none()
                && authoring.independent_review.is_none(),
            "semantic-inventory entry contains later-stage evidence: {source_id}"
        ),
        GraphicsTranslationStatus::NeedsRegionInventory => ensure!(
            has_all_semantic_fields
                && !authoring.region_inventory_complete
                && authoring.replacement_image.is_none()
                && authoring.replacement_overlay.is_none()
                && authoring.human_approval.is_none()
                && authoring.independent_review.is_none(),
            "region-inventory entry is missing semantic identity or contains later-stage evidence: {source_id}"
        ),
        GraphicsTranslationStatus::InProgress => ensure!(
            has_all_semantic_fields
                && authoring.region_inventory_complete
                && !authoring.regions.is_empty()
                && (!requires_english_pixel_preservation
                    || authoring.protected_english_mask.is_some())
                && authoring.replacement_image.is_none()
                && (authoring.replacement_overlay.is_none() || has_bounded_partial_overlay)
                && authoring.human_approval.is_none()
                && authoring.independent_review.is_none(),
            "in-progress graphics entry lacks a closed region inventory or contains unbounded replacement or approval evidence: {source_id}"
        ),
        GraphicsTranslationStatus::NeedsHumanReview => ensure!(
            has_all_semantic_fields
                && authoring.region_inventory_complete
                && !authoring.regions.is_empty()
                && (!requires_english_pixel_preservation
                    || authoring.protected_english_mask.is_some())
                && (authoring.replacement_image.is_some()
                    ^ authoring.replacement_overlay.is_some())
                && !has_rough_artwork
                && authoring.human_approval.is_none()
                && authoring.independent_review.is_none(),
            "human-review graphics entry lacks consumer-aligned authored evidence or already contains a decision: {source_id}"
        ),
        GraphicsTranslationStatus::Complete => anyhow::bail!(
            "complete graphics status is unavailable until insertion, protected-pixel diffing, runtime observation, and release-candidate eligibility are represented: {source_id}"
        ),
    }

    for asset in [
        authoring.protected_english_mask.as_ref(),
        authoring.replacement_image.as_ref(),
        authoring.replacement_overlay.as_ref(),
    ]
    .into_iter()
    .flatten()
    .chain(authoring.design_candidates.iter())
    .chain(authoring.protected_nontext_masks.iter())
    {
        ensure!(
            !asset.path.trim().is_empty() && !asset.sha256.trim().is_empty(),
            "graphics authoring asset lacks a path or hash: {source_id}"
        );
    }

    let mut region_ids = BTreeSet::new();
    for region in &authoring.regions {
        ensure!(
            region_ids.insert(region.region_id.as_str()),
            "duplicate graphics region id in {source_id}: {}",
            region.region_id
        );
        ensure!(
            !region.region_id.trim().is_empty()
                && !region.semantic_role.trim().is_empty()
                && region.source_text.chars().any(is_japanese),
            "graphics region lacks semantic identity or Japanese transcription: {source_id}"
        );
        ensure!(
            region.bounds.width > 0
                && region.bounds.height > 0
                && region.bounds.x + region.bounds.width <= surface_width
                && region.bounds.y + region.bounds.height <= surface_height,
            "graphics region falls outside source dimensions: {source_id}/{}",
            region.region_id
        );
        if let Some(composition_bounds) = region.artwork_composition_bounds {
            ensure!(
                region.treatment_profile == GraphicsTreatmentProfile::StylizedDisplayLettering
                    && region.artwork_plan.is_some_and(|plan| {
                        plan.method == GraphicsArtworkMethod::GeneratedOrHandAuthored
                    })
                    && composition_bounds.width > 0
                    && composition_bounds.height > 0
                    && composition_bounds.x + composition_bounds.width <= surface_width
                    && composition_bounds.y + composition_bounds.height <= surface_height
                    && region.bounds.x >= composition_bounds.x
                    && region.bounds.y >= composition_bounds.y
                    && region.bounds.x + region.bounds.width
                        <= composition_bounds.x + composition_bounds.width
                    && region.bounds.y + region.bounds.height
                        <= composition_bounds.y + composition_bounds.height,
                "graphics artwork composition is invalid: {source_id}/{}",
                region.region_id
            );
        }
        ensure!(
            !region.source_pixel_mask.path.trim().is_empty()
                && !region.source_pixel_mask.sha256.trim().is_empty(),
            "graphics region has no hashed source-pixel mask: {source_id}/{}",
            region.region_id
        );
        ensure!(
            region.source_fill_color.is_none()
                || region.treatment_profile == GraphicsTreatmentProfile::FunctionalUiText,
            "source fill color is only supported for functional UI text"
        );
        validate_region_artwork_plan(
            source_id,
            &region.region_id,
            region.treatment_profile,
            region.artwork_plan,
        )?;
        validate_region_translation_source(
            source_id,
            region.treatment_profile,
            region.korean_text.as_deref(),
            region.glyph_demand_source,
        )?;
    }
    Ok(())
}

pub(super) fn validate_region_artwork_plan(
    source_id: &str,
    region_id: &str,
    treatment_profile: GraphicsTreatmentProfile,
    artwork_plan: Option<GraphicsArtworkPlan>,
) -> Result<()> {
    if treatment_profile == GraphicsTreatmentProfile::StylizedDisplayLettering {
        let expected_method = expected_stylized_artwork_method(source_id, region_id);
        ensure!(
            artwork_plan.is_some_and(|plan| plan.method == expected_method),
            "stylized display lettering has the wrong artwork method: {source_id}/{region_id}"
        );
    } else {
        ensure!(
            artwork_plan.is_none(),
            "non-stylized graphics region cannot declare an artwork plan: {source_id}/{region_id}"
        );
    }
    Ok(())
}

fn expected_stylized_artwork_method(source_id: &str, region_id: &str) -> GraphicsArtworkMethod {
    if (source_id == "linked-texture/image-g/000039" && region_id == "tea-cup-label")
        || (source_id == "linked-texture/image-g/000077" && region_id.starts_with("tea-cup-label-"))
        || ([
            "linked-texture/image-md/000300",
            "linked-texture/image-md/000302",
        ]
        .contains(&source_id)
            && region_id.starts_with("tea-cup-label-pattern-"))
    {
        GraphicsArtworkMethod::DeterministicRaster
    } else {
        GraphicsArtworkMethod::GeneratedOrHandAuthored
    }
}

pub(super) fn validate_region_translation_source(
    source_id: &str,
    treatment_profile: GraphicsTreatmentProfile,
    korean_text: Option<&str>,
    glyph_demand_source: Option<GlyphDemandSource>,
) -> Result<()> {
    let expected_glyph_demand_source = match source_id {
        "linked-texture/image-d/000021" | "linked-texture/image-g/000122" => {
            Some(GlyphDemandSource::MemoryCardMessages)
        }
        "linked-texture/image-d/000022" => Some(GlyphDemandSource::TutorialMessages),
        "linked-texture/image-g/000113" => Some(GlyphDemandSource::PuzzlePuyoMessages),
        _ => None,
    };

    if treatment_profile == GraphicsTreatmentProfile::FontGlyphSupply {
        ensure!(
            korean_text.is_none(),
            "font glyph supply must derive demand from its text consumer instead of copying Korean text: {source_id}"
        );
        let glyph_demand_source = glyph_demand_source.ok_or_else(|| {
            anyhow::anyhow!("font glyph supply has no declared demand source: {source_id}")
        })?;
        ensure!(
            Some(glyph_demand_source) == expected_glyph_demand_source,
            "font glyph supply is not bound to the exact source consumer: {source_id}"
        );
        return Ok(());
    }

    if treatment_profile == GraphicsTreatmentProfile::CutsceneBakedText {
        ensure!(
            expected_glyph_demand_source.is_none() && glyph_demand_source.is_none(),
            "cutscene baked text declares an unrelated glyph demand source: {source_id}"
        );
        ensure!(
            korean_text.is_none(),
            "cutscene baked text must derive wording from the story ledger instead of copying Korean text: {source_id}"
        );
        return Ok(());
    }

    ensure!(
        expected_glyph_demand_source.is_none(),
        "exact source font glyph supply uses a direct graphics treatment: {source_id}"
    );
    ensure!(
        glyph_demand_source.is_none(),
        "direct graphics region declares an unrelated glyph demand source: {source_id}"
    );
    let korean_text = korean_text
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("graphics region has no direct Korean draft: {source_id}")
        })?;
    ensure!(
        !korean_text.chars().any(is_japanese),
        "graphics Korean draft contains Japanese text: {source_id}"
    );
    Ok(())
}

fn is_japanese(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' | '\u{3400}'..='\u{9fff}'
    )
}
