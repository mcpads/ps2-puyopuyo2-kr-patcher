use std::path::Path;

use serde::Serialize;

use super::font::MemoryCardGlyphAssignment;
use crate::source::DiscInventory;

pub struct MemoryCardTranslationBuildInputs<'a> {
    pub inventory: &'a DiscInventory,
    pub source_bin_path: &'a Path,
    pub executable: &'a [u8],
    pub image_d_container: &'a [u8],
    pub image_g_container: &'a [u8],
    pub translation_path: &'a Path,
    pub font_config_path: &'a Path,
    pub font_path: &'a Path,
    pub output_bin_path: &'a Path,
    pub output_cue_path: &'a Path,
    pub preview_path: &'a Path,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardTranslationBuildReport {
    pub schema: u32,
    pub status: &'static str,
    pub release_candidate_eligible: bool,
    pub source_bin_sha256: String,
    pub output_bin_path: String,
    pub output_bin_byte_length: u64,
    pub output_bin_sha256: String,
    pub output_cue_path: String,
    pub preview_path: String,
    pub preview_png_sha256: String,
    pub translated_messages: usize,
    pub in_progress_messages: usize,
    pub needs_review_messages: usize,
    pub needs_human_review_messages: usize,
    pub assigned_hangul_glyphs: usize,
    pub assignable_font_slots: usize,
    pub remaining_font_slots: usize,
    pub font_id: String,
    pub font_sha256: String,
    pub font_license: String,
    pub font_upstream_repository: String,
    pub font_pixel_size_milli: u32,
    pub font_coverage_threshold: u8,
    pub source_linear_indices_sha256: String,
    pub replacement_linear_indices_sha256: String,
    pub protected_font_pixels: usize,
    pub changed_protected_font_pixels: usize,
    pub assignments: Vec<MemoryCardGlyphAssignment>,
    pub font_surfaces: Vec<MemoryCardFontSurfaceReport>,
    pub font_replica_consumer_safety: FontReplicaConsumerSafetyReport,
    pub executable_expected_writes: usize,
    pub image_container_expected_writes: usize,
    pub raw_sector_plans: usize,
    pub raw_sector_data_writes: usize,
    pub modified_lbas: Vec<u32>,
    pub verification: MemoryCardTranslationBuildVerification,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardFontSurfaceReport {
    pub role: &'static str,
    pub container_iso_path: &'static str,
    pub file_list_symbol: &'static str,
    pub image_file_record: usize,
    pub palette_file_record: usize,
    pub registration_path: &'static str,
    pub source_cpx_sha256: String,
    pub replacement_cpx_sha256: String,
    pub source_linear_indices_sha256: String,
    pub replacement_linear_indices_sha256: String,
    pub source_cpx_byte_length: usize,
    pub replacement_cpx_byte_length: usize,
    pub cpx_slot_byte_length: usize,
    pub cpx_padding_byte_length: usize,
    pub container_expected_writes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct FontReplicaConsumerSafetyReport {
    pub owner_load_tables: Vec<&'static str>,
    pub owner_load_table_count: usize,
    pub validated_fixed_pattern_tables: Vec<&'static str>,
    pub validated_fixed_pattern_records: usize,
    pub slot_one_fixed_pattern_records: usize,
    pub direct_pattern_call_sites: usize,
    pub dynamic_slot_one_call_sites: usize,
    pub fixed_slot_one_call_sites: usize,
    pub unreachable_slot_one_pattern_records: usize,
    pub coverage_status: &'static str,
    pub unresolved_scope: &'static str,
    pub static_consumer_scope_closed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardTranslationBuildVerification {
    pub source_consumers: &'static str,
    pub translation_scope: &'static str,
    pub lookup: &'static str,
    pub font_rendering: &'static str,
    pub protected_pixels: &'static str,
    pub cpx_roundtrip: &'static str,
    pub output_iso_readback: &'static str,
    pub human_review: &'static str,
    pub runtime_status: &'static str,
}
