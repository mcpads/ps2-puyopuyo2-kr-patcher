use super::*;

#[derive(Clone, Debug, Serialize)]
pub struct MessageCorpusReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub scope: MessageCorpusScope,
    pub population: MessagePopulation,
    pub consumer: MessageConsumerEvidence,
    pub control_codes: Vec<ControlCodeUsage>,
    pub references: Vec<MessageReference>,
    pub messages: Vec<MessageEntry>,
    pub translation_scope: MessageTranslationScope,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialMessageCorpusReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub scope: MessageCorpusScope,
    pub population: MessagePopulation,
    pub producer: TutorialMessageProducerEvidence,
    pub renderer: TutorialMessageRendererEvidence,
    pub message_bank: TutorialMessageBankEvidence,
    pub message_table: TutorialMessageTableEvidence,
    pub layout: TutorialMessageLayoutEvidence,
    pub font_lookup: TutorialFontLookupEvidence,
    pub control_codes: Vec<ControlCodeUsage>,
    pub messages: Vec<MessageEntry>,
    pub translation_scope: MessageTranslationScope,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialMessageBankEvidence {
    pub start_symbol: &'static str,
    pub address: String,
    pub file_offset: String,
    pub byte_length: usize,
    pub source_sha256: String,
    pub declared_message_bytes: usize,
    pub zero_padding_bytes: usize,
    pub message_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialMessageProducerEvidence {
    pub isa_profile: &'static str,
    pub symbol: &'static str,
    pub address: String,
    pub byte_length: usize,
    pub sha256: String,
    pub instruction_count: usize,
    pub renderer_actor_creations: usize,
    pub message_reference: MessageReference,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialMessageRendererEvidence {
    pub symbol: &'static str,
    pub address: String,
    pub byte_length: usize,
    pub sha256: String,
    pub instruction_count: usize,
    pub invariants: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialMessageTableEvidence {
    pub symbol: &'static str,
    pub address: String,
    pub file_offset: String,
    pub byte_length: usize,
    pub sha256: String,
    pub pointer_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialMessageLayoutEvidence {
    pub command_opcode: String,
    pub sequence_symbols: Vec<&'static str>,
    pub physical_message_commands: usize,
    pub unique_message_indices: usize,
    pub repeated_message_commands: usize,
    pub actor: usize,
    pub origin_x_pixels: usize,
    pub origin_y_pixels: usize,
    pub depth: usize,
    pub reveal_rate: usize,
    pub viewport_width_pixels: usize,
    pub viewport_height_pixels: usize,
    pub glyph_width_pixels: usize,
    pub glyph_height_pixels: usize,
    pub line_advance_pixels: usize,
    pub line_cell_capacity: usize,
    pub line_capacity: usize,
    pub commands: Vec<TutorialMessageCommandEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TutorialMessageCommandEvidence {
    pub sequence_symbol: &'static str,
    pub byte_offset: String,
    pub message_index: usize,
    pub actor: usize,
    pub x_pixels: usize,
    pub y_pixels: usize,
    pub depth: usize,
    pub reveal_rate: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TutorialFontLookupEvidence {
    pub symbol: &'static str,
    pub address: String,
    pub byte_length: usize,
    pub sha256: String,
    pub instruction_count: usize,
    pub font_table_symbol: &'static str,
    pub font_table_address: String,
    pub font_table_byte_length: usize,
    pub font_table_sha256: String,
    pub stored_code_slots: usize,
    pub unique_code_units: usize,
    pub duplicate_code_slots: usize,
    pub repeated_code_units: Vec<RepeatedFontCodeUnit>,
    pub invariants: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RepeatedFontCodeUnit {
    pub raw_hex: String,
    pub occurrences: usize,
    pub first_slot: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MessageCorpusScope {
    pub consumer_symbol: &'static str,
    pub renderer_symbol: &'static str,
    pub included: &'static str,
    pub excluded: &'static str,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MessagePopulation {
    pub status: &'static str,
    pub consumer_message_sites: usize,
    pub direct_sites: usize,
    pub indexed_table_sites: usize,
    pub physical_reference_edges: usize,
    pub unique_messages: usize,
    pub duplicate_reference_edges: usize,
    pub unresolved_sites: usize,
    pub raw_payload_bytes: usize,
    pub japanese_characters: usize,
    pub protected_english_runs: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MessageConsumerEvidence {
    pub isa_profile: &'static str,
    pub consumer_symbol: &'static str,
    pub consumer_address: String,
    pub consumer_byte_length: usize,
    pub consumer_sha256: String,
    pub consumer_instruction_count: usize,
    pub renderer_symbol: &'static str,
    pub renderer_address: String,
    pub renderer_byte_length: usize,
    pub renderer_sha256: String,
    pub renderer_instruction_count: usize,
    pub nonnull_message_store_count: usize,
    pub ignored_null_message_stores: usize,
    pub control_dispatch: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TypedInstructionEvidence {
    pub address: String,
    pub bytes: String,
    pub canonical: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ControlCodeUsage {
    pub kind: String,
    pub raw_values: Vec<String>,
    pub occurrences: usize,
    pub byte_width: usize,
    pub policy: String,
    pub confirmed_effect: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MessageReference {
    pub reference_id: String,
    pub kind: &'static str,
    pub table_symbol: Option<String>,
    pub message_ids: Vec<String>,
    pub message_symbols: Vec<String>,
    pub instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MessageEntry {
    pub message_id: String,
    pub symbol: String,
    pub source_address: String,
    pub source_file_offset: String,
    pub storage_byte_length: usize,
    pub payload_byte_length: usize,
    pub source_sha256: String,
    pub source_hex: String,
    pub terminator: &'static str,
    pub display_text: String,
    pub tokens: Vec<MessageToken>,
    pub protected_english_runs: Vec<ProtectedEnglishRun>,
    pub reference_ids: Vec<String>,
    pub roundtrip: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MessageToken {
    pub byte_offset: usize,
    pub byte_length: usize,
    pub raw_hex: String,
    pub kind: &'static str,
    pub value: String,
    pub translation_policy: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProtectedEnglishRun {
    pub byte_offset: usize,
    pub raw_hex: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MessageTranslationScope {
    pub target: &'static str,
    pub preserve: &'static str,
    pub report_role: &'static str,
}

pub struct MessageCorpusInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
}
