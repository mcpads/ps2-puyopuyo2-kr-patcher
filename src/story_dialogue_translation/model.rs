use serde::{Deserialize, Serialize};

use crate::graphics_translation::GraphicsTranslationBundleSource;
use crate::texture_language_survey::TextureLanguageClassification;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueTranslationBundle {
    pub schema: u32,
    pub source: GraphicsTranslationBundleSource,
    pub scope: StoryDialogueTranslationScope,
    pub population: StoryDialogueTranslationPopulation,
    pub entries: Vec<StoryDialogueTranslationEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueTranslationManifest {
    pub schema: u32,
    pub source: GraphicsTranslationBundleSource,
    pub scope: StoryDialogueTranslationScope,
    pub population: StoryDialogueTranslationPopulation,
    pub parts: Vec<StoryDialoguePartReference>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialoguePartReference {
    pub part_id: String,
    pub relative_path: String,
    pub first_consumer_sequence: usize,
    pub last_consumer_sequence: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueTranslationPart {
    pub schema: u32,
    pub part_id: String,
    pub entries: Vec<StoryDialogueTranslationEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueTranslationScope {
    pub target: String,
    pub preserve: String,
    pub reference_policy: String,
    pub build_policy: String,
    pub release_candidate: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueTranslationPopulation {
    pub consumer_tables: usize,
    pub dialogue_surfaces: usize,
    pub unresolved_tables: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueTranslationEntry {
    pub source: StoryDialogueSurfaceSource,
    pub authoring: StoryDialogueAuthoring,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueSurfaceSource {
    pub story_surface_id: String,
    pub consumer_table: String,
    pub consumer_sequence: usize,
    pub source_surface_id: String,
    pub container_iso_path: String,
    pub image_file_record: usize,
    pub width: u32,
    pub height: u32,
    pub linear_indices_sha256: String,
    pub classification: TextureLanguageClassification,
    pub requires_english_pixel_preservation: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueAuthoring {
    pub status: StoryDialogueStatus,
    pub scene_id: Option<String>,
    pub scene_variant: Option<String>,
    pub segments: Vec<StoryDialogueSegment>,
    pub transcription_notes: Vec<String>,
    pub transcription_review: Option<StoryDialogueDecisionRecord>,
    pub human_approval: Option<StoryDialogueDecisionRecord>,
    pub independent_review: Option<StoryDialogueDecisionRecord>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryDialogueStatus {
    NeedsTranscription,
    NeedsReview,
    NeedsHumanReview,
    Complete,
}

impl StoryDialogueStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeedsTranscription => "needs_transcription",
            Self::NeedsReview => "needs_review",
            Self::NeedsHumanReview => "needs_human_review",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueSegment {
    pub segment_id: String,
    pub speaker_id: String,
    pub source_text: String,
    pub korean_text: String,
    pub sibling_references: Vec<SiblingStoryReference>,
    pub translator_notes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SiblingStoryReference {
    pub stable_id: String,
    pub match_kind: SiblingStoryMatchKind,
    pub note: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SiblingStoryMatchKind {
    ExactSourceMatch,
    NearExpandedMatch,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryDialogueDecisionRecord {
    pub recorded_at: String,
    pub recorded_from: String,
    pub decision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoryDialogueTranslationValidation {
    pub dialogue_surfaces: usize,
    pub needs_transcription: usize,
    pub needs_review: usize,
    pub needs_human_review: usize,
    pub complete: usize,
    pub release_candidate_eligible: usize,
    pub transcribed_segments: usize,
    pub source_bound_layout_lines: usize,
    pub source_characters: usize,
    pub korean_characters: usize,
    pub sibling_reference_links: usize,
}
