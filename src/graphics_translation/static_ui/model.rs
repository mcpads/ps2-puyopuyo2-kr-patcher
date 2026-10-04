use std::path::Path;

use serde::{Deserialize, Serialize};

use super::super::static_regions::StaticRegionCatalog;
use super::super::{GraphicsTranslationBundle, GraphicsTreatmentProfile, HashedAsset, PixelBounds};
use crate::texture_preview_corpus::TexturePreviewCorpus;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticTextDraftScope {
    FunctionalUi,
    DeterministicText,
}

impl StaticTextDraftScope {
    pub(super) fn selects(self, profile: GraphicsTreatmentProfile) -> bool {
        match self {
            Self::FunctionalUi => profile == GraphicsTreatmentProfile::FunctionalUiText,
            Self::DeterministicText => profile == GraphicsTreatmentProfile::FunctionalUiText,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::FunctionalUi => "functional UI",
            Self::DeterministicText => "deterministic static text",
        }
    }

    pub(super) fn requires_complete_render(self) -> bool {
        self == Self::DeterministicText
    }
}

pub(crate) struct RegionRestorationInputs<'a> {
    pub(crate) surface_width: usize,
    pub(crate) surface_height: usize,
    pub(crate) bounds: PixelBounds,
    pub(crate) source_mask: &'a [bool],
    pub(crate) all_source_masks: &'a [bool],
    pub(crate) protected_english: &'a [bool],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticUiFontConfiguration {
    pub schema: u32,
    pub id: String,
    pub expected_filename: String,
    pub sha256: String,
    pub license: String,
    pub upstream_repository: String,
    pub rasterization: StaticUiRasterizationConfiguration,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticUiRasterizationConfiguration {
    pub minimum_pixel_size: u32,
    pub maximum_pixel_size: u32,
    pub coverage_threshold: u8,
    pub outline_radius: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct StaticTextDraftReport {
    pub schema: u32,
    pub scope: StaticTextDraftScope,
    pub source_bin_sha256: String,
    pub font_id: String,
    pub font_sha256: String,
    pub selected_product_surfaces: usize,
    pub deferred_surfaces: Vec<DeferredStaticTextSurface>,
    pub rendered_surfaces: usize,
    pub rendered_regions: usize,
    pub skipped_surfaces: Vec<SkippedStaticUiSurface>,
    pub surfaces: Vec<StaticUiSurfaceDraft>,
    pub release_candidate_eligible: bool,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeferredStaticTextSurface {
    pub source_surface_id: String,
    pub region_count: usize,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkippedStaticUiSurface {
    pub source_surface_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct StaticUiSurfaceDraft {
    pub source_surface_id: String,
    pub source_png_sha256: String,
    pub overlay: HashedAsset,
    pub preview_path: String,
    pub preview_png_sha256: String,
    pub source_japanese_pixels: usize,
    pub korean_overlay_pixels: usize,
    pub changed_preview_pixels: usize,
    pub changed_protected_pixels: usize,
    pub regions: Vec<StaticUiRegionDraft>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StaticUiRegionDraft {
    pub region_id: String,
    pub korean_text: String,
    pub pixel_size: u32,
    pub rendered_copies: usize,
    pub source_pixels: usize,
    pub overlay_pixels: usize,
    pub visible_background_pixels: usize,
    pub background_reconstruction: StaticUiBackgroundReconstruction,
    pub lines: Vec<StaticUiLinePlacement>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticUiBackgroundReconstruction {
    Transparent,
    NearestPixelInsideDeclaredBounds,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct StaticUiLinePlacement {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub alignment: StaticUiLineAlignment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticUiLineAlignment {
    Left,
    Center,
    Right,
}

pub struct StaticTextDraftInputs<'a> {
    pub graphics: &'a mut GraphicsTranslationBundle,
    pub catalog: &'a StaticRegionCatalog,
    pub previews: &'a TexturePreviewCorpus,
    pub font_config_path: &'a Path,
    pub font_path: &'a Path,
    pub overlay_directory: &'a Path,
    pub preview_directory: &'a Path,
    pub refresh_existing: bool,
    pub scope: StaticTextDraftScope,
}
