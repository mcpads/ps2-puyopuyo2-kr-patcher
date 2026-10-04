use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::source::DiscInventory;

pub struct PuzzlePuyoTranslationBuildInputs<'a> {
    pub inventory: &'a DiscInventory,
    pub source_bin_path: &'a Path,
    pub executable: &'a [u8],
    pub image_container: &'a [u8],
    pub translation_path: &'a Path,
    pub font_config_path: &'a Path,
    pub font_path: &'a Path,
    pub output_bin_path: &'a Path,
    pub output_cue_path: &'a Path,
    pub preview_path: &'a Path,
}

#[derive(Clone, Debug, Serialize)]
pub struct PuzzlePuyoTranslationBuildReport {
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
    pub assigned_hangul_glyphs: usize,
    pub assigned_halfwidth_glyphs: usize,
    pub remaining_halfwidth_slots: usize,
    pub assignable_font_slots: usize,
    pub remaining_font_slots: usize,
    pub reserved_fallback_slot: usize,
    pub font_table_symbol: String,
    pub font_id: String,
    pub font_sha256: String,
    pub font_license: String,
    pub font_upstream_repository: String,
    pub font_pixel_size_milli: u32,
    pub font_coverage_threshold: u8,
    pub font_image_file_record: usize,
    pub font_palette_file_record: usize,
    pub source_linear_indices_sha256: String,
    pub replacement_linear_indices_sha256: String,
    pub protected_halfwidth_pixels: usize,
    pub changed_protected_halfwidth_pixels: usize,
    pub source_cpx_byte_length: usize,
    pub replacement_cpx_byte_length: usize,
    pub cpx_slot_byte_length: usize,
    pub cpx_padding_byte_length: usize,
    pub executable_expected_writes: usize,
    pub image_container_expected_writes: usize,
    pub raw_sector_plans: usize,
    pub raw_sector_data_writes: usize,
    pub modified_lbas: Vec<u32>,
    pub verification: PuzzlePuyoTranslationBuildVerification,
}

#[derive(Clone, Debug, Serialize)]
pub struct PuzzlePuyoTranslationBuildVerification {
    pub source_consumer: &'static str,
    pub translation_scope: &'static str,
    pub lookup: &'static str,
    pub font_rendering: &'static str,
    pub cpx_roundtrip: &'static str,
    pub output_iso_readback: &'static str,
    pub human_review: &'static str,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FontConfiguration {
    pub(super) schema: u32,
    pub(super) id: String,
    pub(super) expected_filename: String,
    pub(super) sha256: String,
    pub(super) license: String,
    pub(super) upstream_repository: String,
    pub(super) rasterization: RasterizationConfiguration,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RasterizationConfiguration {
    pub(super) pixel_size: f32,
    pub(super) coverage_threshold: u8,
}
