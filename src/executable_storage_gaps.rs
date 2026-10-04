use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object, ObjectSection};
use serde::{Deserialize, Serialize};

mod review;
mod scanner;

use review::{ReviewedGapCandidate, validate_review_catalog};
use scanner::scan_storage_gaps;

#[derive(Clone, Debug)]
pub struct ExecutableStorageGapInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExecutableStorageGapReviewCatalog {
    pub schema: u32,
    pub executable_sha256: String,
    pub review_basis: String,
    pub regions: Vec<ExecutableStorageGapReviewRegion>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExecutableStorageGapReviewRegion {
    pub region_id: String,
    pub start_address: String,
    pub end_address: String,
    pub source_sha256: String,
    pub classification: ExecutableStorageGapClassification,
    pub note: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutableStorageGapClassification {
    DisplayedJapaneseText,
    NonStringBinary,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableStorageGapReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub section: ExecutableStorageGapSection,
    pub summary: ExecutableStorageGapSummary,
    pub review_basis: String,
    pub review_regions: Vec<ExecutableStorageGapReviewedRegion>,
    pub candidates: Vec<ExecutableStorageGapCandidate>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableStorageGapSection {
    pub name: &'static str,
    pub address: String,
    pub file_offset: String,
    pub byte_length: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableStorageGapSummary {
    pub maximal_nonempty_nul_terminated_runs: usize,
    pub exact_shift_jis_runs: usize,
    pub japanese_script_runs: usize,
    pub text_symbol_owned_japanese_runs: usize,
    pub data_symbol_owned_japanese_runs: usize,
    pub storage_gap_candidates: usize,
    pub displayed_japanese_text: usize,
    pub non_string_binary: usize,
    pub unresolved_candidates: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableStorageGapReviewedRegion {
    pub region_id: String,
    pub start_address: String,
    pub end_address: String,
    pub byte_length: usize,
    pub source_sha256: String,
    pub classification: ExecutableStorageGapClassification,
    pub candidate_count: usize,
    pub note: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableStorageGapCandidate {
    pub storage_id: String,
    pub address: String,
    pub file_offset: String,
    pub storage_byte_length: usize,
    pub source_sha256: String,
    pub source_hex: String,
    pub display_text: String,
    pub japanese_characters: usize,
    pub review_region_id: String,
    pub classification: ExecutableStorageGapClassification,
    pub translation_target: bool,
    pub translation_policy: &'static str,
    pub review_note: String,
}

pub fn survey_executable_storage_gaps(
    inputs: ExecutableStorageGapInputs<'_>,
    catalog: &ExecutableStorageGapReviewCatalog,
) -> Result<ExecutableStorageGapReport> {
    ensure!(
        crate::indexed_texture::sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    let file = object::File::parse(inputs.executable)
        .context("failed to parse executable for storage-gap survey")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips && file.endianness() == Endianness::Little,
        "storage-gap survey requires a little-endian MIPS ELF"
    );
    let section = file
        .section_by_name("main")
        .context("executable has no main file-backed section")?;
    let section_data = section.data()?;
    let (section_file_offset, section_file_size) = section
        .file_range()
        .context("main section has no file range")?;
    ensure!(
        section_file_size == u64::try_from(section_data.len())?,
        "main section file range differs from its data length"
    );
    let scan = scan_storage_gaps(&file, &section)?;
    let reviewed = validate_review_catalog(
        catalog,
        &inputs.executable_sha256,
        section.address(),
        section_data,
        &scan.candidates,
    )?;
    let displayed_japanese_text = reviewed
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.classification == ExecutableStorageGapClassification::DisplayedJapaneseText
        })
        .count();
    let non_string_binary = reviewed.candidates.len() - displayed_japanese_text;
    let candidates = reviewed
        .candidates
        .into_iter()
        .map(candidate_report)
        .collect::<Vec<_>>();
    ensure!(
        displayed_japanese_text == 0,
        "new displayed Japanese storage requires a producer-specific corpus before adoption"
    );

    Ok(ExecutableStorageGapReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        section: ExecutableStorageGapSection {
            name: "main",
            address: format!("{:#010x}", section.address()),
            file_offset: format!("{section_file_offset:#010x}"),
            byte_length: section_data.len(),
            sha256: crate::indexed_texture::sha256(section_data),
        },
        summary: ExecutableStorageGapSummary {
            maximal_nonempty_nul_terminated_runs: scan.maximal_nonempty_nul_terminated_runs,
            exact_shift_jis_runs: scan.exact_shift_jis_runs,
            japanese_script_runs: scan.japanese_script_runs,
            text_symbol_owned_japanese_runs: scan.text_symbol_owned_japanese_runs,
            data_symbol_owned_japanese_runs: scan.data_symbol_owned_japanese_runs,
            storage_gap_candidates: candidates.len(),
            displayed_japanese_text,
            non_string_binary,
            unresolved_candidates: 0,
        },
        review_basis: catalog.review_basis.clone(),
        review_regions: reviewed.regions,
        candidates,
        evidence_boundary: "This survey scans every maximal nonempty NUL-terminated run in the file-backed main section and exact-decodes byte-reversible Shift-JIS. Runs intersecting a named text symbol or fully contained by a named data symbol are owned by the existing symbol census; every remaining Japanese-script candidate must fall in one exact-hash reviewed region. It does not cover GP-relative or computed references, runtime-built text, other disc files, baked graphics, video, or runtime display.",
    })
}

fn candidate_report(candidate: ReviewedGapCandidate) -> ExecutableStorageGapCandidate {
    let translation_target =
        candidate.classification == ExecutableStorageGapClassification::DisplayedJapaneseText;
    ExecutableStorageGapCandidate {
        storage_id: format!("executable/storage/{:08x}", candidate.address),
        address: format!("{:#010x}", candidate.address),
        file_offset: format!("{:#010x}", candidate.file_offset),
        storage_byte_length: candidate.bytes.len(),
        source_sha256: crate::indexed_texture::sha256(&candidate.bytes),
        source_hex: candidate
            .bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        display_text: candidate.display_text,
        japanese_characters: candidate.japanese_characters,
        review_region_id: candidate.review_region_id,
        classification: candidate.classification,
        translation_target,
        translation_policy: if translation_target {
            "translate Japanese bytes only; preserve ASCII bytes and controls"
        } else {
            "preserve all bytes; decoded Japanese is incidental binary data"
        },
        review_note: candidate.review_note,
    }
}
