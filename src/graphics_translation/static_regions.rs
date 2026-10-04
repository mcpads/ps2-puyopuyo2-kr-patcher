use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::asset_naming::linked_texture_surface_file_stem;
use super::raster::{hashed_asset, read_rgba_surface, write_binary_mask};
use super::validation::{validate_region_artwork_plan, validate_region_translation_source};
use super::{
    GlyphDemandSource, GraphicsArtworkMethod, GraphicsArtworkPlan,
    GraphicsSourceBackgroundRestoration, GraphicsTextRegion, GraphicsTranslationBundle,
    GraphicsTranslationStatus, GraphicsTreatmentProfile, HashedAsset, PixelBounds,
};
use crate::indexed_texture::sha256;
use crate::texture_language_survey::TextureLanguageClassification;
use crate::texture_preview_corpus::{TexturePreview, TexturePreviewCorpus};

mod pixel_selection;

pub use pixel_selection::{ChannelRange, RegionPixelSelection, RgbaColor};
use pixel_selection::{contains, rectangles_overlap, select_region_pixels};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StaticRegionCatalog {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub surfaces: Vec<StaticSurfaceDeclaration>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StaticSurfaceDeclaration {
    pub source_surface_id: String,
    pub source_png_sha256: String,
    #[serde(default)]
    pub additional_source_png_sha256s: Vec<String>,
    pub regions: Vec<StaticRegionDeclaration>,
    #[serde(default)]
    pub protected_english_regions: Vec<ProtectedEnglishRegionDeclaration>,
    #[serde(default)]
    pub functional_source_mask_dilation_radius: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StaticRegionDeclaration {
    pub region_id: String,
    pub semantic_role: String,
    pub bounds: PixelBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_composition_bounds: Option<PixelBounds>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub artwork_uses_overlay_alpha: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_restoration_bounds: Vec<PixelBounds>,
    #[serde(default)]
    pub source_background_restoration: GraphicsSourceBackgroundRestoration,
    #[serde(default)]
    pub protected_subregions: Vec<PixelBounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal_copy_count: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_pixel_size: Option<u32>,
    /// Explicit layout choice; source-pixel inference is only a legacy fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_alignment: Option<super::static_ui::StaticUiLineAlignment>,
    /// Use the existing one-pixel outline footprint to thicken the text fill.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default)]
    pub source_mask_dilation_radius: u8,
    #[serde(default)]
    pub pixel_selection: RegionPixelSelection,
    /// Optional fill color that must occur inside this region's source text mask.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_fill_color: Option<[u8; 4]>,
    pub source_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub korean_text: Option<String>,
    pub treatment_profile: GraphicsTreatmentProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_plan: Option<GraphicsArtworkPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph_demand_source: Option<GlyphDemandSource>,
    #[serde(default)]
    pub translator_notes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedEnglishRegionDeclaration {
    pub region_id: String,
    pub semantic_role: String,
    pub bounds: PixelBounds,
    #[serde(default)]
    pub protected_subregions: Vec<PixelBounds>,
    #[serde(default)]
    pub pixel_selection: RegionPixelSelection,
    pub source_text: String,
    #[serde(default)]
    pub translator_notes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StaticRegionInventoryReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub inventoried_surfaces: usize,
    pub inventoried_regions: usize,
    pub surfaces: Vec<StaticSurfaceRegionReport>,
    pub release_candidate_eligible: bool,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StaticSurfaceRegionReport {
    pub source_surface_id: String,
    pub source_png_sha256s: Vec<String>,
    pub editable_pixels: usize,
    pub protected_pixels: usize,
    pub regions: Vec<StaticRegionReport>,
    pub protected_english_regions: Vec<StaticRegionReport>,
    pub protected_english_mask: Option<HashedAsset>,
    pub protected_nontext_mask: HashedAsset,
    pub functional_source_mask_dilation_radius: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StaticRegionReport {
    pub region_id: String,
    pub bounds: PixelBounds,
    pub artwork_composition_bounds: Option<PixelBounds>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub artwork_uses_overlay_alpha: bool,
    pub source_restoration_bounds: Vec<PixelBounds>,
    pub source_background_restoration: GraphicsSourceBackgroundRestoration,
    pub protected_subregions: Vec<PixelBounds>,
    pub pixel_selection: RegionPixelSelection,
    pub source_mask_dilation_radius: u8,
    pub selected_pixels: usize,
    pub selected_palette_indices: Vec<u8>,
    pub source_pixel_mask: HashedAsset,
}

pub fn inventory_static_regions(
    graphics: &mut GraphicsTranslationBundle,
    catalog: &StaticRegionCatalog,
    previews: &TexturePreviewCorpus,
    source_linear_indices: &BTreeMap<String, Vec<u8>>,
    output_directory: &Path,
    refresh_existing: bool,
) -> Result<StaticRegionInventoryReport> {
    ensure!(
        catalog.schema == 1,
        "unsupported static region catalog schema"
    );
    ensure!(
        !catalog.surfaces.is_empty(),
        "static region catalog has no surfaces"
    );
    ensure!(
        graphics.source.source_bin_sha256 == catalog.source_bin_sha256,
        "static region catalog differs from the graphics source"
    );
    ensure!(
        graphics.source.source_bin_sha256 == previews.source_bin_sha256
            && graphics.source.executable_iso_path == previews.executable_iso_path
            && graphics.source.executable_sha256 == previews.executable_sha256,
        "texture previews differ from the static graphics source"
    );

    let graphics_by_source = graphics
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.source.source_surface_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut seen_surfaces = BTreeSet::new();
    let mut surface_reports = Vec::new();

    for declaration in &catalog.surfaces {
        ensure!(
            declaration.functional_source_mask_dilation_radius <= 2,
            "static functional source-mask dilation exceeds two pixels: {}",
            declaration.source_surface_id
        );
        ensure!(
            seen_surfaces.insert(declaration.source_surface_id.as_str()),
            "duplicate static region surface: {}",
            declaration.source_surface_id
        );
        ensure!(
            !declaration.regions.is_empty(),
            "static region surface has no declared regions: {}",
            declaration.source_surface_id
        );
        let graphics_index = *graphics_by_source
            .get(declaration.source_surface_id.as_str())
            .with_context(|| {
                format!(
                    "graphics ledger has no declared static surface: {}",
                    declaration.source_surface_id
                )
            })?;
        let target = &mut graphics.entries[graphics_index];
        let requires_english_mask = validate_declared_language_regions(
            target.source.classification,
            target.source.requires_english_pixel_preservation,
            declaration.protected_english_regions.len(),
            &declaration.source_surface_id,
        )?;
        ensure!(
            matches!(
                target.authoring.status,
                GraphicsTranslationStatus::NeedsRegionInventory
                    | GraphicsTranslationStatus::InProgress
                    | GraphicsTranslationStatus::NeedsHumanReview
            ) && !(target.authoring.replacement_image.is_some()
                && target.authoring.replacement_overlay.is_some())
                && target.authoring.human_approval.is_none()
                && target.authoring.independent_review.is_none(),
            "static graphics target is not in a region-authoring state: {}",
            declaration.source_surface_id
        );
        let declared_previews = find_exact_previews(previews, target, declaration)?;
        ensure!(
            declaration.additional_source_png_sha256s.is_empty()
                || declaration
                    .regions
                    .iter()
                    .all(|region| region.pixel_selection.is_source_pixel_stable()),
            "multi-palette static surface must select regions by stable source pixels: {}",
            declaration.source_surface_id
        );
        let preview = declared_previews[0];
        let surface = read_rgba_surface(Path::new(&preview.png_path))?;
        let linear_indices = source_linear_indices
            .get(&declaration.source_surface_id)
            .with_context(|| {
                format!(
                    "static surface has no exact-source linear indices: {}",
                    declaration.source_surface_id
                )
            })?;
        ensure!(
            surface.width == usize::try_from(target.source.width)?
                && surface.height == usize::try_from(target.source.height)?
                && linear_indices.len() == surface.width * surface.height
                && sha256(linear_indices) == target.source.linear_indices_sha256,
            "static preview dimensions or source linear indices differ: {}",
            declaration.source_surface_id
        );

        let mut claimed_pixels = vec![false; surface.width * surface.height];
        let mut authored_regions = Vec::new();
        let mut region_reports = Vec::new();
        let mut seen_regions = BTreeSet::new();
        let stem = linked_texture_surface_file_stem(&declaration.source_surface_id)?;
        for region in &declaration.regions {
            validate_asset_file_component(&region.region_id)?;
            ensure!(
                seen_regions.insert(region.region_id.as_str()),
                "duplicate static graphics region: {}/{}",
                declaration.source_surface_id,
                region.region_id
            );
            let context = format!("{}/{}", declaration.source_surface_id, region.region_id);
            ensure!(
                !region.region_id.trim().is_empty()
                    && !region.semantic_role.trim().is_empty()
                    && region.source_text.chars().any(is_japanese),
                "static graphics region lacks a direct Japanese transcription: {context}"
            );
            validate_region_translation_source(
                &declaration.source_surface_id,
                region.treatment_profile,
                region.korean_text.as_deref(),
                region.glyph_demand_source,
            )?;
            ensure!(
                region.source_fill_color.is_none()
                    || region.treatment_profile == GraphicsTreatmentProfile::FunctionalUiText,
                "source fill color is only supported for functional UI text"
            );
            validate_region_artwork_plan(
                &declaration.source_surface_id,
                &region.region_id,
                region.treatment_profile,
                region.artwork_plan,
            )?;
            ensure!(
                region.source_mask_dilation_radius <= 2,
                "static region source-mask dilation exceeds two pixels: {context}"
            );
            ensure!(
                region.source_background_restoration
                    == GraphicsSourceBackgroundRestoration::NearestSourceIndex
                    || region.treatment_profile
                        == GraphicsTreatmentProfile::StylizedDisplayLettering,
                "row-dominant source restoration is only valid for stylized artwork: {context}"
            );
            if let Some(composition_bounds) = region.artwork_composition_bounds {
                ensure!(
                    (region.treatment_profile == GraphicsTreatmentProfile::FunctionalUiText
                        && !region.artwork_uses_overlay_alpha)
                        || region.treatment_profile
                            == GraphicsTreatmentProfile::StylizedDisplayLettering
                            && region.artwork_plan.is_some_and(|plan| {
                                plan.method == GraphicsArtworkMethod::GeneratedOrHandAuthored
                            }),
                    "composition requires functional text or generated stylized artwork: {context}"
                );
                ensure!(
                    composition_bounds.width > 0
                        && composition_bounds.height > 0
                        && composition_bounds.x + composition_bounds.width
                            <= u32::try_from(surface.width)?
                        && composition_bounds.y + composition_bounds.height
                            <= u32::try_from(surface.height)?,
                    "artwork composition exceeds its source surface: {context}"
                );
                ensure!(
                    region.bounds.x >= composition_bounds.x
                        && region.bounds.y >= composition_bounds.y
                        && region.bounds.x + region.bounds.width
                            <= composition_bounds.x + composition_bounds.width
                        && region.bounds.y + region.bounds.height
                            <= composition_bounds.y + composition_bounds.height,
                    "artwork composition does not contain its Japanese text region: {context}"
                );
            }
            let claimed_before = claimed_pixels.clone();
            let mut mask = select_region_pixels(
                &surface,
                linear_indices,
                region.bounds,
                &region.protected_subregions,
                &region.pixel_selection,
                &mut claimed_pixels,
                &context,
            )?;
            let source_mask_dilation_radius = if region.source_mask_dilation_radius > 0 {
                region.source_mask_dilation_radius
            } else if region.treatment_profile == GraphicsTreatmentProfile::FunctionalUiText {
                declaration.functional_source_mask_dilation_radius
            } else {
                0
            };
            if source_mask_dilation_radius > 0 {
                mask = dilate_mask_inside_bounds(
                    &mask,
                    surface.width,
                    surface.height,
                    region.bounds,
                    source_mask_dilation_radius,
                )?;
                for protected in &region.protected_subregions {
                    clear_mask_inside_bounds(&mut mask, surface.width, surface.height, *protected)?;
                }
                for (pixel, selected) in mask.iter().enumerate() {
                    if !selected {
                        continue;
                    }
                    ensure!(
                        !claimed_before[pixel],
                        "dilated static source masks overlap: {context}"
                    );
                    claimed_pixels[pixel] = true;
                }
            }
            if let Some(composition_bounds) = region.artwork_composition_bounds {
                let left = usize::try_from(composition_bounds.x)?;
                let top = usize::try_from(composition_bounds.y)?;
                let right = usize::try_from(composition_bounds.x + composition_bounds.width)?;
                let bottom = usize::try_from(composition_bounds.y + composition_bounds.height)?;
                for y in top..bottom {
                    for x in left..right {
                        let pixel = y * surface.width + x;
                        ensure!(
                            !claimed_before[pixel],
                            "artwork composition overlaps another static region: {context}"
                        );
                        claimed_pixels[pixel] = true;
                    }
                }
            }
            validate_source_restoration_bounds(
                &mask,
                surface.width,
                surface.height,
                &region.source_restoration_bounds,
                &context,
            )?;
            let mask_path =
                output_directory.join(format!("{stem}-{}.source.mask.png", region.region_id));
            write_binary_mask(&mask_path, surface.width, surface.height, &mask)?;
            let source_pixel_mask = hashed_asset(&mask_path)?;
            let selected_pixels = mask.iter().filter(|selected| **selected).count();
            let selected_palette_indices = mask
                .iter()
                .enumerate()
                .filter_map(|(index, selected)| selected.then_some(linear_indices[index]))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            authored_regions.push(GraphicsTextRegion {
                region_id: region.region_id.clone(),
                semantic_role: region.semantic_role.clone(),
                bounds: region.bounds,
                artwork_composition_bounds: region.artwork_composition_bounds,
                artwork_uses_overlay_alpha: region.artwork_uses_overlay_alpha,
                source_restoration_bounds: region.source_restoration_bounds.clone(),
                source_background_restoration: region.source_background_restoration,
                source_fill_color: region.source_fill_color,
                source_text: region.source_text.clone(),
                korean_text: region.korean_text.clone(),
                treatment_profile: region.treatment_profile,
                artwork_plan: region.artwork_plan,
                glyph_demand_source: region.glyph_demand_source,
                source_pixel_mask: source_pixel_mask.clone(),
                translator_notes: region.translator_notes.clone(),
            });
            region_reports.push(StaticRegionReport {
                region_id: region.region_id.clone(),
                bounds: region.bounds,
                artwork_composition_bounds: region.artwork_composition_bounds,
                artwork_uses_overlay_alpha: region.artwork_uses_overlay_alpha,
                source_restoration_bounds: region.source_restoration_bounds.clone(),
                source_background_restoration: region.source_background_restoration,
                protected_subregions: region.protected_subregions.clone(),
                pixel_selection: region.pixel_selection.clone(),
                source_mask_dilation_radius,
                selected_pixels,
                selected_palette_indices,
                source_pixel_mask,
            });
        }
        let editable_pixels = claimed_pixels.iter().filter(|claimed| **claimed).count();

        let mut protected_english_pixels = vec![false; surface.width * surface.height];
        let mut protected_english_region_reports = Vec::new();
        for region in &declaration.protected_english_regions {
            validate_asset_file_component(&region.region_id)?;
            ensure!(
                seen_regions.insert(region.region_id.as_str()),
                "duplicate static graphics region: {}/{}",
                declaration.source_surface_id,
                region.region_id
            );
            ensure!(
                !region.region_id.trim().is_empty()
                    && !region.semantic_role.trim().is_empty()
                    && !region.source_text.trim().is_empty()
                    && !region.source_text.chars().any(is_japanese),
                "protected English region lacks a non-Japanese transcription: {}/{}",
                declaration.source_surface_id,
                region.region_id
            );
            let context = format!(
                "{}/protected-English/{}",
                declaration.source_surface_id, region.region_id
            );
            let mask = select_region_pixels(
                &surface,
                linear_indices,
                region.bounds,
                &region.protected_subregions,
                &region.pixel_selection,
                &mut claimed_pixels,
                &context,
            )?;
            for (combined, selected) in protected_english_pixels.iter_mut().zip(&mask) {
                *combined |= selected;
            }
            let mask_path = output_directory.join(format!(
                "{stem}-{}.protected-english.mask.png",
                region.region_id
            ));
            write_binary_mask(&mask_path, surface.width, surface.height, &mask)?;
            let source_pixel_mask = hashed_asset(&mask_path)?;
            let selected_pixels = mask.iter().filter(|selected| **selected).count();
            let selected_palette_indices = mask
                .iter()
                .enumerate()
                .filter_map(|(index, selected)| selected.then_some(linear_indices[index]))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            protected_english_region_reports.push(StaticRegionReport {
                region_id: region.region_id.clone(),
                bounds: region.bounds,
                artwork_composition_bounds: None,
                artwork_uses_overlay_alpha: false,
                source_restoration_bounds: Vec::new(),
                source_background_restoration:
                    GraphicsSourceBackgroundRestoration::NearestSourceIndex,
                protected_subregions: region.protected_subregions.clone(),
                pixel_selection: region.pixel_selection.clone(),
                source_mask_dilation_radius: 0,
                selected_pixels,
                selected_palette_indices,
                source_pixel_mask,
            });
        }

        let protected_english_mask = if requires_english_mask {
            let path = output_directory.join(format!("{stem}-protected-english.mask.png"));
            write_binary_mask(
                &path,
                surface.width,
                surface.height,
                &protected_english_pixels,
            )?;
            Some(hashed_asset(&path)?)
        } else {
            None
        };

        let protected_mask = claimed_pixels
            .iter()
            .map(|claimed| !claimed)
            .collect::<Vec<_>>();
        let protected_nontext_pixels = protected_mask
            .iter()
            .filter(|protected| **protected)
            .count();
        let protected_english_pixel_count = protected_english_pixels
            .iter()
            .filter(|protected| **protected)
            .count();
        let protected_pixels = protected_english_pixel_count + protected_nontext_pixels;
        ensure!(
            editable_pixels + protected_pixels == surface.width * surface.height,
            "static graphics masks do not partition the source surface: {}",
            declaration.source_surface_id
        );
        let protected_path = output_directory.join(format!("{stem}-protected-nontext.mask.png"));
        write_binary_mask(
            &protected_path,
            surface.width,
            surface.height,
            &protected_mask,
        )?;
        let protected_nontext_mask = hashed_asset(&protected_path)?;

        if matches!(
            target.authoring.status,
            GraphicsTranslationStatus::InProgress | GraphicsTranslationStatus::NeedsHumanReview
        ) {
            let matches_existing = target.authoring.regions == authored_regions
                && target.authoring.protected_nontext_masks == [protected_nontext_mask.clone()]
                && target.authoring.protected_english_mask == protected_english_mask;
            if !matches_existing {
                ensure!(
                    refresh_existing
                        && target.authoring.replacement_image.is_none()
                        && target.authoring.human_approval.is_none()
                        && target.authoring.independent_review.is_none(),
                    "existing static region authoring differs from the declared catalog; pass --refresh-existing only for unapproved deterministic assets: {}",
                    declaration.source_surface_id
                );
                target.authoring.status = GraphicsTranslationStatus::InProgress;
                target.authoring.regions = authored_regions;
                target.authoring.protected_nontext_masks = vec![protected_nontext_mask.clone()];
                target.authoring.protected_english_mask = protected_english_mask.clone();
                target.authoring.replacement_overlay = None;
            }
        } else {
            target.authoring.status = GraphicsTranslationStatus::InProgress;
            target.authoring.region_inventory_complete = true;
            target.authoring.regions = authored_regions;
            target.authoring.protected_nontext_masks = vec![protected_nontext_mask.clone()];
            target.authoring.protected_english_mask = protected_english_mask.clone();
        }
        surface_reports.push(StaticSurfaceRegionReport {
            source_surface_id: declaration.source_surface_id.clone(),
            source_png_sha256s: declared_previews
                .iter()
                .map(|preview| preview.png_sha256.clone())
                .collect(),
            editable_pixels,
            protected_pixels,
            regions: region_reports,
            protected_english_regions: protected_english_region_reports,
            protected_english_mask,
            protected_nontext_mask,
            functional_source_mask_dilation_radius: declaration
                .functional_source_mask_dilation_radius,
        });
    }

    Ok(StaticRegionInventoryReport {
        schema: 1,
        source_bin_sha256: graphics.source.source_bin_sha256.clone(),
        inventoried_surfaces: surface_reports.len(),
        inventoried_regions: surface_reports
            .iter()
            .map(|surface| surface.regions.len())
            .sum(),
        surfaces: surface_reports,
        release_candidate_eligible: false,
        evidence_boundary: "Each listed Japanese-only or Japanese-English mixed surface is bound to exact-source preview variant hashes and source linear palette-index identity. Every editable Japanese mask and protected English mask contains pixels admitted by a declared visible-pixel, color, palette-index, or hash-bound reviewed pixel-mask selection inside a manually declared tight rectangle. Mixed surfaces require at least one explicit protected English region. Multi-palette surfaces require source-pixel-stable selection; duplicate or absent declared colors and indices fail. Reviewed masks must be full-surface binary PNGs, select nothing outside the declared tight bounds, and match their declared SHA-256. Optional source-mask dilation is limited to two pixels inside the declared bounds, and explicitly protected subregions are removed again after dilation. Optional source-restoration partitions must be nonempty, non-overlapping, inside the surface, cover every selected source pixel, and each contain a selected pixel. Japanese, English, and non-text pixels form a non-overlapping full-surface partition. This inventory provides direct Korean drafts but not replacement artwork, palette or CPX insertion, runtime display, independent review, human approval, or release-candidate eligibility.",
    })
}

fn validate_declared_language_regions(
    classification: TextureLanguageClassification,
    requires_english_pixel_preservation: bool,
    protected_english_region_count: usize,
    source_surface_id: &str,
) -> Result<bool> {
    match classification {
        TextureLanguageClassification::JapaneseOnly => {
            ensure!(
                !requires_english_pixel_preservation && protected_english_region_count == 0,
                "Japanese-only static surface declares protected English pixels: {source_surface_id}"
            );
            Ok(false)
        }
        TextureLanguageClassification::JapaneseEnglishMixed => {
            ensure!(
                requires_english_pixel_preservation && protected_english_region_count > 0,
                "mixed-language static surface lacks protected English regions: {source_surface_id}"
            );
            Ok(true)
        }
        _ => anyhow::bail!(
            "declared static surface is not a Japanese translation target: {source_surface_id}"
        ),
    }
}

fn find_exact_previews<'a>(
    previews: &'a TexturePreviewCorpus,
    target: &super::GraphicsTranslationEntry,
    declaration: &StaticSurfaceDeclaration,
) -> Result<Vec<&'a TexturePreview>> {
    let hashes = std::iter::once(&declaration.source_png_sha256)
        .chain(&declaration.additional_source_png_sha256s)
        .collect::<Vec<_>>();
    ensure!(
        hashes.iter().collect::<BTreeSet<_>>().len() == hashes.len(),
        "static surface declares a duplicate preview variant: {}",
        declaration.source_surface_id
    );
    let selected = hashes
        .into_iter()
        .map(|hash| {
            let matches = previews
                .previews
                .iter()
                .filter(|preview| {
                    preview.image_container_iso_path == target.source.container_iso_path
                        && preview.image_file_record == target.source.image_file_record
                        && preview.png_sha256 == *hash
                })
                .collect::<Vec<_>>();
            ensure!(
                matches.len() == 1,
                "static surface does not have exactly one declared preview variant: {}",
                declaration.source_surface_id
            );
            let preview = matches[0];
            ensure!(
                preview.width == target.source.width
                    && preview.height == target.source.height
                    && preview.linear_indices_sha256 == target.source.linear_indices_sha256,
                "static preview differs from its exact graphics source: {}",
                declaration.source_surface_id
            );
            Ok(preview)
        })
        .collect::<Result<Vec<_>>>()?;
    validate_preview_owner_coverage(
        selected
            .iter()
            .map(|preview| preview.owner_tables.as_slice()),
        &target.source.owner_tables,
    )?;
    Ok(selected)
}

fn validate_preview_owner_coverage<'a>(
    preview_owners: impl IntoIterator<Item = &'a [String]>,
    source_owners: &[String],
) -> Result<()> {
    let expected = source_owners.iter().collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();
    for owners in preview_owners {
        ensure!(!owners.is_empty(), "static preview has no source consumer");
        for owner in owners {
            ensure!(
                expected.contains(owner),
                "static preview has an unrelated source consumer: {owner}"
            );
            observed.insert(owner);
        }
    }
    ensure!(
        observed == expected,
        "declared static previews do not cover every source consumer"
    );
    Ok(())
}

fn validate_asset_file_component(component: &str) -> Result<()> {
    ensure!(
        !component.is_empty()
            && component.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            }),
        "static region ID cannot form a safe asset filename: {component}"
    );
    Ok(())
}

fn is_japanese(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' | '\u{3400}'..='\u{9fff}'
    )
}

fn validate_source_restoration_bounds(
    mask: &[bool],
    surface_width: usize,
    surface_height: usize,
    partitions: &[PixelBounds],
    context: &str,
) -> Result<()> {
    ensure!(
        mask.len() == surface_width * surface_height,
        "static source-restoration dimensions changed: {context}"
    );
    if partitions.is_empty() {
        return Ok(());
    }
    for (index, bounds) in partitions.iter().enumerate() {
        let left = usize::try_from(bounds.x)?;
        let top = usize::try_from(bounds.y)?;
        let right = usize::try_from(bounds.x + bounds.width)?;
        let bottom = usize::try_from(bounds.y + bounds.height)?;
        ensure!(
            left < right
                && top < bottom
                && right <= surface_width
                && bottom <= surface_height
                && partitions[..index]
                    .iter()
                    .all(|other| !rectangles_overlap(*other, *bounds)),
            "static source-restoration bounds are empty, overlapping, or outside the surface: {context}"
        );
        ensure!(
            (top..bottom).any(|y| (left..right).any(|x| mask[y * surface_width + x])),
            "static source-restoration partition has no selected source pixel: {context}"
        );
    }
    ensure!(
        mask.iter().enumerate().all(|(pixel, selected)| {
            !selected
                || partitions
                    .iter()
                    .any(|bounds| contains(*bounds, pixel % surface_width, pixel / surface_width))
        }),
        "static source mask escapes its restoration partitions: {context}"
    );
    Ok(())
}

fn dilate_mask_inside_bounds(
    mask: &[bool],
    surface_width: usize,
    surface_height: usize,
    bounds: PixelBounds,
    radius: u8,
) -> Result<Vec<bool>> {
    ensure!(
        mask.len() == surface_width * surface_height && radius <= 2,
        "static source-mask dilation inputs changed"
    );
    if radius == 0 {
        return Ok(mask.to_vec());
    }
    let left = usize::try_from(bounds.x)?;
    let top = usize::try_from(bounds.y)?;
    let right = usize::try_from(bounds.x + bounds.width)?;
    let bottom = usize::try_from(bounds.y + bounds.height)?;
    ensure!(
        left < right && top < bottom && right <= surface_width && bottom <= surface_height,
        "static source-mask dilation bounds exceed the surface"
    );
    let radius = usize::from(radius);
    let mut dilated = mask.to_vec();
    for y in top..bottom {
        for x in left..right {
            if !mask[y * surface_width + x] {
                continue;
            }
            for adjacent_y in y.saturating_sub(radius).max(top)..=(y + radius).min(bottom - 1) {
                for adjacent_x in x.saturating_sub(radius).max(left)..=(x + radius).min(right - 1) {
                    dilated[adjacent_y * surface_width + adjacent_x] = true;
                }
            }
        }
    }
    Ok(dilated)
}

fn clear_mask_inside_bounds(
    mask: &mut [bool],
    surface_width: usize,
    surface_height: usize,
    bounds: PixelBounds,
) -> Result<()> {
    ensure!(
        mask.len() == surface_width * surface_height,
        "static source-mask protection dimensions changed"
    );
    let left = usize::try_from(bounds.x)?;
    let top = usize::try_from(bounds.y)?;
    let right = usize::try_from(bounds.x + bounds.width)?;
    let bottom = usize::try_from(bounds.y + bounds.height)?;
    ensure!(
        left < right && top < bottom && right <= surface_width && bottom <= surface_height,
        "static source-mask protection exceeds the surface"
    );
    for y in top..bottom {
        for x in left..right {
            mask[y * surface_width + x] = false;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "static_regions_tests.rs"]
mod static_regions_tests;
