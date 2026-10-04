use serde::{Deserialize, Serialize};

use crate::texture_language_survey::TextureLanguageClassification;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTranslationBundle {
    pub schema: u32,
    pub source: GraphicsTranslationBundleSource,
    pub scope: GraphicsTranslationScope,
    pub population: GraphicsTranslationPopulation,
    pub entries: Vec<GraphicsTranslationEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTranslationManifest {
    pub schema: u32,
    pub source: GraphicsTranslationBundleSource,
    pub scope: GraphicsTranslationScope,
    pub population: GraphicsTranslationPopulation,
    pub parts: Vec<GraphicsTranslationPartReference>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTranslationPartReference {
    pub part_id: String,
    pub relative_path: String,
    pub responsibility: String,
    pub entry_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTranslationPart {
    pub schema: u32,
    pub part_id: String,
    pub entries: Vec<GraphicsTranslationEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTranslationBundleSource {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTranslationScope {
    pub target: String,
    pub preserve: String,
    pub authoring_policy: String,
    pub build_policy: String,
    pub release_candidate: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTranslationPopulation {
    pub linked_texture_surfaces: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTranslationEntry {
    pub source: GraphicsTranslationSource,
    pub authoring: GraphicsAuthoring,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTranslationSource {
    pub source_surface_id: String,
    pub container_iso_path: String,
    pub image_file_record: usize,
    pub width: u32,
    pub height: u32,
    pub linear_indices_sha256: String,
    pub classification: TextureLanguageClassification,
    pub requires_english_pixel_preservation: bool,
    pub owner_tables: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsAuthoring {
    pub status: GraphicsTranslationStatus,
    pub semantic_asset_id: Option<String>,
    pub semantic_role: Option<String>,
    pub continuity_group: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub design_candidates: Vec<HashedAsset>,
    pub region_inventory_complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_nontext_masks: Vec<HashedAsset>,
    pub protected_english_mask: Option<HashedAsset>,
    pub replacement_image: Option<HashedAsset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_overlay: Option<HashedAsset>,
    pub regions: Vec<GraphicsTextRegion>,
    pub translator_notes: Vec<String>,
    pub human_approval: Option<GraphicsDecisionRecord>,
    pub independent_review: Option<GraphicsDecisionRecord>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsTranslationStatus {
    NeedsSemanticInventory,
    NeedsRegionInventory,
    InProgress,
    NeedsHumanReview,
    Complete,
}

impl GraphicsTranslationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeedsSemanticInventory => "needs_semantic_inventory",
            Self::NeedsRegionInventory => "needs_region_inventory",
            Self::InProgress => "in_progress",
            Self::NeedsHumanReview => "needs_human_review",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HashedAsset {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsTextRegion {
    pub region_id: String,
    pub semantic_role: String,
    pub bounds: PixelBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_composition_bounds: Option<PixelBounds>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub artwork_uses_overlay_alpha: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_restoration_bounds: Vec<PixelBounds>,
    #[serde(
        default,
        skip_serializing_if = "GraphicsSourceBackgroundRestoration::is_nearest_source_index"
    )]
    pub source_background_restoration: GraphicsSourceBackgroundRestoration,
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
    pub source_pixel_mask: HashedAsset,
    pub translator_notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsSourceBackgroundRestoration {
    #[default]
    NearestSourceIndex,
    DominantSourceIndexPerRow,
    TransparentSourceIndex,
}

impl GraphicsSourceBackgroundRestoration {
    fn is_nearest_source_index(&self) -> bool {
        *self == Self::NearestSourceIndex
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsTreatmentProfile {
    FontGlyphSupply,
    FontAtlas,
    FunctionalUiText,
    StylizedDisplayLettering,
    CutsceneBakedText,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsArtworkPlan {
    pub method: GraphicsArtworkMethod,
    pub placement_status: GraphicsArtworkPlacementStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsArtworkMethod {
    GeneratedOrHandAuthored,
    DeterministicRaster,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsArtworkPlacementStatus {
    RoughRegionBounds,
    SourceAligned,
    ConsumerAligned,
}

impl GraphicsArtworkPlacementStatus {
    pub(crate) fn is_buildable(self) -> bool {
        self != Self::RoughRegionBounds
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlyphDemandSource {
    MemoryCardMessages,
    TutorialMessages,
    PuzzlePuyoMessages,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PixelBounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsDecisionRecord {
    pub recorded_at: String,
    pub recorded_from: String,
    pub decision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphicsTranslationValidation {
    pub linked_texture_surfaces: usize,
    pub needs_semantic_inventory: usize,
    pub needs_region_inventory: usize,
    pub in_progress: usize,
    pub needs_human_review: usize,
    pub complete: usize,
    pub release_candidate_eligible: usize,
}
