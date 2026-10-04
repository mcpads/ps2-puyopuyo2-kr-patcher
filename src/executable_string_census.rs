use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use serde::{Deserialize, Serialize};

mod classification;
mod references;
mod scanner;

use classification::classify_candidates;
use references::find_string_references;
use scanner::read_japanese_string_symbols;

pub use crate::executable_references::{
    StaticReference as ExecutableStringReference,
    StaticReferenceKind as ExecutableStringReferenceKind,
};
pub(crate) use scanner::{decode_exact_string, symbol_file_range};

#[derive(Clone, Debug)]
pub struct ExecutableStringCensusInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableStringCensus {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub summary: ExecutableStringCensusSummary,
    pub review_basis: String,
    pub candidates: Vec<JapaneseStringCandidate>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExecutableStringCensusSummary {
    pub file_backed_symbols: usize,
    pub exact_shift_jis_symbol_candidates: usize,
    pub japanese_script_candidates: usize,
    pub declared_message_targets: usize,
    pub additional_display_text_targets: usize,
    pub font_repertoires: usize,
    pub non_string_binary: usize,
    pub unresolved_candidates: usize,
    pub referenced_candidates: usize,
    pub unreferenced_candidates: usize,
    pub typed_address_references: usize,
    pub pointer_references: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct JapaneseStringCandidate {
    pub symbol: String,
    pub address: String,
    pub file_offset: String,
    pub storage_byte_length: usize,
    pub source_sha256: String,
    pub source_hex: String,
    pub display_text: String,
    pub japanese_characters: usize,
    pub classification: ExecutableStringClassification,
    pub translation_target: bool,
    pub declared_message_corpora: Vec<String>,
    pub translation_policy: String,
    pub review_note: String,
    pub references: Vec<ExecutableStringReference>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExecutableStringReviewCatalog {
    pub schema: u32,
    pub executable_sha256: String,
    pub review_basis: String,
    pub entries: Vec<ExecutableStringReviewEntry>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExecutableStringReviewEntry {
    pub symbol: String,
    pub address: String,
    pub source_sha256: String,
    pub classification: ReviewedCandidateClassification,
    pub note: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewedCandidateClassification {
    DisplayedJapaneseMetadata,
    FontRepertoire,
    NonStringBinary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutableStringClassification {
    DeclaredMessage,
    DisplayedJapaneseMetadata,
    FontRepertoire,
    NonStringBinary,
}

pub fn census_executable_strings(
    inputs: ExecutableStringCensusInputs<'_>,
    catalog: &ExecutableStringReviewCatalog,
) -> Result<ExecutableStringCensus> {
    let file = object::File::parse(inputs.executable)
        .context("failed to parse executable for string census")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips && file.endianness() == Endianness::Little,
        "string census requires a little-endian MIPS ELF"
    );
    let file_backed_symbols = file
        .symbols()
        .filter(|symbol| {
            scanner::symbol_file_range(&file, symbol)
                .ok()
                .flatten()
                .is_some()
        })
        .count();
    let scan = read_japanese_string_symbols(&file)?;
    let target_addresses = scan
        .japanese_strings
        .iter()
        .map(|string| (string.address, string.symbol.clone()))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        target_addresses.len() == scan.japanese_strings.len(),
        "Japanese string symbols have duplicate addresses"
    );
    let references = find_string_references(&file, &target_addresses)?;
    let decisions =
        classify_candidates(&scan.japanese_strings, catalog, &inputs.executable_sha256)?;
    let declared_message_symbols = crate::message_corpus::declared_message_corpus_memberships();
    let mut referenced_candidates = 0;
    let mut typed_address_references = 0;
    let mut pointer_references = 0;
    let mut declared_message_targets = 0;
    let mut additional_display_text_targets = 0;
    let mut font_repertoires = 0;
    let mut non_string_binary = 0;
    let candidates = scan
        .japanese_strings
        .into_iter()
        .map(|string| {
            let string_references = references.get(&string.symbol).cloned().unwrap_or_default();
            if !string_references.is_empty() {
                referenced_candidates += 1;
            }
            for reference in &string_references {
                match reference.kind {
                    ExecutableStringReferenceKind::TypedAddressMaterialization => {
                        typed_address_references += 1;
                    }
                    ExecutableStringReferenceKind::Pointer => pointer_references += 1,
                }
            }
            let decision = decisions
                .get(&string.symbol)
                .expect("every scanned candidate must have a classification");
            match decision.classification {
                ExecutableStringClassification::DeclaredMessage => declared_message_targets += 1,
                ExecutableStringClassification::DisplayedJapaneseMetadata => {
                    additional_display_text_targets += 1;
                }
                ExecutableStringClassification::FontRepertoire => font_repertoires += 1,
                ExecutableStringClassification::NonStringBinary => non_string_binary += 1,
            }
            JapaneseStringCandidate {
                symbol: string.symbol,
                address: format!("{:#010x}", string.address),
                file_offset: format!("{:#010x}", string.file_offset),
                storage_byte_length: string.bytes.len(),
                source_sha256: crate::indexed_texture::sha256(&string.bytes),
                source_hex: string
                    .bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                display_text: string.display_text,
                japanese_characters: string.japanese_characters,
                classification: decision.classification,
                translation_target: decision.translation_target,
                declared_message_corpora: decision
                    .declared_message_corpora
                    .iter()
                    .map(|corpus| (*corpus).to_string())
                    .collect(),
                translation_policy: decision.translation_policy.to_string(),
                review_note: decision.review_note.clone(),
                references: string_references,
            }
        })
        .collect::<Vec<_>>();
    ensure!(
        declared_message_targets == declared_message_symbols.len(),
        "global census does not cover every declared message corpus symbol"
    );
    let summary = ExecutableStringCensusSummary {
        file_backed_symbols,
        exact_shift_jis_symbol_candidates: scan.exact_string_symbols,
        japanese_script_candidates: candidates.len(),
        declared_message_targets,
        additional_display_text_targets,
        font_repertoires,
        non_string_binary,
        unresolved_candidates: 0,
        referenced_candidates,
        unreferenced_candidates: candidates.len() - referenced_candidates,
        typed_address_references,
        pointer_references,
    };
    Ok(ExecutableStringCensus {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        summary,
        review_basis: catalog.review_basis.clone(),
        candidates,
        evidence_boundary: "This census covers every named file-backed ELF symbol whose complete storage is one NUL-terminated, byte-reversible Shift-JIS candidate containing Japanese script. It reuses the five producer-specific message catalogs and requires an exact-source review decision for every remaining candidate. References cover aligned little-endian pointers and typed lui/addiu address materializations in named file-backed symbols. It does not cover GP-relative or computed references, unnamed storage, runtime-built text, baked graphics, or video, and static references do not prove runtime reachability.",
    })
}
