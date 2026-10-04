use serde::{Deserialize, Serialize};

use crate::memory_card_translation::{KoreanTranslation, TranslationBundleSource};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutableTextTranslationBundle {
    pub schema: u32,
    pub source: TranslationBundleSource,
    pub scope: ExecutableTextTranslationScope,
    pub population: ExecutableTextTranslationPopulation,
    pub entries: Vec<ExecutableTextTranslationEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutableTextTranslationScope {
    pub consumer_families: Vec<String>,
    pub target: String,
    pub preserve: String,
    pub build_policy: String,
    pub release_candidate: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutableTextTranslationPopulation {
    pub tutorial_messages: usize,
    pub puzzle_puyo_messages: usize,
    pub memory_card_metadata_titles: usize,
    pub total_entries: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutableTextTranslationEntry {
    pub source: ExecutableTextTranslationSource,
    pub translation: KoreanTranslation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutableTextTranslationSource {
    pub asset_id: String,
    pub consumer_family: String,
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
    pub reference_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutableTextTranslationValidation {
    pub total_entries: usize,
    pub untranslated: usize,
    pub in_progress: usize,
    pub needs_review: usize,
    pub needs_human_review: usize,
    pub complete: usize,
    pub release_candidate_eligible: usize,
    pub fixed_storage_checks: usize,
    pub minimum_remaining_storage_bytes: Option<usize>,
    pub tutorial_message_bank_byte_length: usize,
    pub tutorial_encoded_byte_length: usize,
    pub tutorial_remaining_message_bank_bytes: usize,
    pub consumer_demands: Vec<ExecutableTextConsumerDemand>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutableTextConsumerDemand {
    pub consumer_family: String,
    pub translated_entries: usize,
    pub required_unique_hangul_glyphs: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutableTextGlyphPlan {
    pub consumer_family: String,
    pub font_table_symbol: String,
    pub physical_slots: usize,
    pub reserved_fallback_slot: usize,
    pub assignable_slots: usize,
    pub required_unique_hangul_glyphs: usize,
    pub remaining_slots: usize,
    pub assignments: Vec<ExecutableTextGlyphAssignment>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutableTextGlyphAssignment {
    pub character: String,
    pub code_hex: String,
    pub slot_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutableTextHalfwidthGlyphPlan {
    pub consumer_family: String,
    pub font_table_symbol: String,
    pub physical_slots: usize,
    pub source_supported_glyphs: usize,
    pub assignable_shadowed_space_slots: usize,
    pub required_added_glyphs: usize,
    pub remaining_shadowed_space_slots: usize,
    pub assignments: Vec<ExecutableTextGlyphAssignment>,
}
