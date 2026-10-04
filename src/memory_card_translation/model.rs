use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCardTranslationBundle {
    pub schema: u32,
    pub source: TranslationBundleSource,
    pub scope: MemoryCardTranslationScope,
    pub population: MemoryCardTranslationPopulation,
    pub entries: Vec<MemoryCardTranslationEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationBundleSource {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCardTranslationScope {
    pub consumer_corpora: Vec<String>,
    pub target: String,
    pub preserve: String,
    pub build_policy: String,
    pub release_candidate: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCardTranslationPopulation {
    pub corpus_entry_occurrences: usize,
    pub unique_messages: usize,
    pub shared_messages: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCardTranslationEntry {
    pub source: MemoryCardTranslationSource,
    pub translation: KoreanTranslation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCardTranslationSource {
    pub message_id: String,
    pub symbol: String,
    pub source_address: String,
    pub source_file_offset: String,
    pub storage_byte_length: usize,
    pub payload_byte_length: usize,
    pub source_sha256: String,
    pub source_hex: String,
    pub terminator: String,
    pub source_text: String,
    pub protected_ascii: Vec<String>,
    pub protected_runtime_controls: Vec<String>,
    pub consumer_corpora: Vec<String>,
    pub reference_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KoreanTranslation {
    pub status: TranslationStatus,
    pub text: String,
    pub translator_notes: Vec<String>,
    pub human_approval: Option<TranslationDecisionRecord>,
    pub independent_review: Option<TranslationDecisionRecord>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationStatus {
    Untranslated,
    InProgress,
    NeedsReview,
    NeedsHumanReview,
    Complete,
}

impl TranslationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Untranslated => "untranslated",
            Self::InProgress => "in_progress",
            Self::NeedsReview => "needs_review",
            Self::NeedsHumanReview => "needs_human_review",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationDecisionRecord {
    pub recorded_at: String,
    pub recorded_from: String,
    pub decision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryCardTranslationValidation {
    pub unique_messages: usize,
    pub untranslated: usize,
    pub in_progress: usize,
    pub needs_review: usize,
    pub needs_human_review: usize,
    pub complete: usize,
    pub release_candidate_eligible: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryCardTranslationCapacity {
    pub translated_messages: usize,
    pub required_unique_hangul_glyphs: usize,
    pub proven_reassignable_slots: usize,
    pub remaining_reassignable_slots: usize,
    pub fixed_storage_checks: usize,
    pub minimum_remaining_storage_bytes: Option<usize>,
    pub status: &'static str,
}
