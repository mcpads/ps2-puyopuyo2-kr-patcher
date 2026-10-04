use serde::{Deserialize, Serialize};

use crate::executable_computed_references::ExecutableComputedReferenceReport;
use crate::executable_font_consumers::ExecutableFontConsumerReport;
use crate::executable_gp_references::ExecutableGpReferenceReport;
use crate::executable_storage_gaps::ExecutableStorageGapReport;
use crate::executable_string_census::{ExecutableStringCensus, ExecutableStringClassification};
use crate::movie_corpus::{MovieCorpus, MovieLanguageClassification};
use crate::texture_language_survey::{TextureLanguageClassification, TextureLanguageSurvey};
use crate::texture_load_calls::TextureLoadCallSurvey;
use crate::unlinked_texture_corpus::language_survey::UnlinkedTextureLanguageSurvey;

pub struct TranslationTargetInputs<'a> {
    pub source_bin_sha256: &'a str,
    pub executable_iso_path: &'a str,
    pub executable_sha256: &'a str,
    pub executable_strings: &'a ExecutableStringCensus,
    pub storage_gaps: &'a ExecutableStorageGapReport,
    pub gp_references: &'a ExecutableGpReferenceReport,
    pub computed_references: &'a ExecutableComputedReferenceReport,
    pub font_consumers: &'a ExecutableFontConsumerReport,
    pub texture_load_calls: &'a TextureLoadCallSurvey,
    pub linked_textures: &'a TextureLanguageSurvey,
    pub unlinked_textures: &'a UnlinkedTextureLanguageSurvey,
    pub movies: &'a MovieCorpus,
}

#[derive(Clone, Debug, Serialize)]
pub struct TranslationTargetSurvey {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub scope: TranslationTargetScope,
    pub evidence_checks: TranslationTargetEvidenceChecks,
    pub summary: TranslationTargetSummary,
    pub executable_text_targets: Vec<ExecutableTextTarget>,
    pub linked_texture_targets: Vec<LinkedTextureTarget>,
    pub movie_targets: Vec<MovieTarget>,
    pub excluded_japanese_candidates: Vec<ExcludedJapaneseCandidate>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TranslationTargetScope {
    pub included: &'static str,
    pub target_unit: &'static str,
    pub excluded: &'static str,
    pub population_status: &'static str,
    pub implementation_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct TranslationTargetEvidenceChecks {
    pub executable_storage_gap_candidates: usize,
    pub new_display_text_from_storage_gaps: usize,
    pub new_candidates_from_gp_references: usize,
    pub new_candidates_from_computed_references: usize,
    pub conservative_control_flow_boundaries: usize,
    pub unresolved_font_consumer_paths: usize,
    pub texture_load_tables_without_static_call_path: usize,
    pub source_incompatible_texture_calls: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TranslationTargetSummary {
    pub executable_japanese_candidates: usize,
    pub executable_text_targets: usize,
    pub executable_non_display_exclusions: usize,
    pub linked_texture_japanese_candidates: usize,
    pub linked_texture_targets: usize,
    pub linked_texture_consumer_exclusions: usize,
    pub linked_texture_product_scope_exclusions: usize,
    pub unlinked_texture_japanese_candidates: usize,
    pub unlinked_texture_targets: usize,
    pub unlinked_texture_consumer_exclusions: usize,
    pub movie_japanese_candidates: usize,
    pub movie_targets: usize,
    pub movie_product_scope_exclusions: usize,
    pub admitted_target_assets: usize,
    pub excluded_japanese_candidates: usize,
    pub unresolved_target_decisions: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableTextTarget {
    pub symbol: String,
    pub address: String,
    pub storage_byte_length: usize,
    pub source_sha256: String,
    pub japanese_characters: usize,
    pub classification: ExecutableStringClassification,
    pub declared_message_corpora: Vec<String>,
    pub translation_policy: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LinkedTextureTarget {
    pub container_iso_path: String,
    pub image_file_record: usize,
    pub width: u32,
    pub height: u32,
    pub linear_indices_sha256: String,
    pub classification: TextureLanguageClassification,
    pub requires_english_pixel_preservation: bool,
    pub owner_tables: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MovieTarget {
    pub iso_path: String,
    pub sha256: String,
    pub frames: usize,
    pub classification: MovieLanguageClassification,
    pub requires_english_pixel_preservation: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExcludedJapaneseCandidate {
    pub surface: TranslationSurface,
    pub locator: String,
    pub reason: TranslationExclusionReason,
    pub review_note: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationSurface {
    ExecutableStorage,
    LinkedTexture,
    UnlinkedTexture,
    Movie,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationExclusionReason {
    FontRepertoireNotDisplayedText,
    BinaryFalsePositive,
    NoStaticLoadPath,
    NoRegisteredConsumer,
    NoExecutableConsumer,
    OtherTitlePromotion,
}
