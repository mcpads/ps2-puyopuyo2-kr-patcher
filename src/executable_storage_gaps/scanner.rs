use anyhow::{Context, Result};
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};

#[derive(Clone, Debug, Eq, PartialEq)]
struct NamedRange {
    start: u64,
    end: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScannedStorageGap {
    pub address: u64,
    pub file_offset: u64,
    pub bytes: Vec<u8>,
    pub display_text: String,
    pub japanese_characters: usize,
}

pub(super) struct StorageGapScan {
    pub maximal_nonempty_nul_terminated_runs: usize,
    pub exact_shift_jis_runs: usize,
    pub japanese_script_runs: usize,
    pub text_symbol_owned_japanese_runs: usize,
    pub data_symbol_owned_japanese_runs: usize,
    pub candidates: Vec<ScannedStorageGap>,
}

pub(super) fn scan_storage_gaps(
    file: &object::File<'_>,
    section: &object::Section<'_, '_>,
) -> Result<StorageGapScan> {
    let section_data = section.data()?;
    let (section_file_offset, _) = section
        .file_range()
        .context("survey section has no file range")?;
    let section_start = section.address();
    let section_end = section_start + u64::try_from(section_data.len())?;
    let mut text_ranges = Vec::new();
    let mut data_ranges = Vec::new();
    for symbol in file.symbols() {
        let Some((_, byte_length)) =
            crate::executable_string_census::symbol_file_range(file, &symbol)?
        else {
            continue;
        };
        let start = symbol.address();
        let end = start + u64::try_from(byte_length)?;
        if start < section_start || end > section_end {
            continue;
        }
        let range = NamedRange { start, end };
        match symbol.kind() {
            SymbolKind::Text => text_ranges.push(range),
            SymbolKind::Data => data_ranges.push(range),
            _ => {}
        }
    }
    normalize_ranges(&mut text_ranges);
    normalize_ranges(&mut data_ranges);
    Ok(scan_section_runs(
        section_start,
        section_file_offset,
        section_data,
        &text_ranges,
        &data_ranges,
    ))
}

fn scan_section_runs(
    section_address: u64,
    section_file_offset: u64,
    source: &[u8],
    text_ranges: &[NamedRange],
    data_ranges: &[NamedRange],
) -> StorageGapScan {
    let mut scan = StorageGapScan {
        maximal_nonempty_nul_terminated_runs: 0,
        exact_shift_jis_runs: 0,
        japanese_script_runs: 0,
        text_symbol_owned_japanese_runs: 0,
        data_symbol_owned_japanese_runs: 0,
        candidates: Vec::new(),
    };
    let mut run_start = 0;
    for (terminator, byte) in source.iter().enumerate() {
        if *byte != 0 {
            continue;
        }
        if terminator == run_start {
            run_start += 1;
            continue;
        }
        let storage = &source[run_start..=terminator];
        scan.maximal_nonempty_nul_terminated_runs += 1;
        let Some((display_text, japanese_characters)) =
            crate::executable_string_census::decode_exact_string(storage)
        else {
            run_start = terminator + 1;
            continue;
        };
        scan.exact_shift_jis_runs += 1;
        if japanese_characters == 0 {
            run_start = terminator + 1;
            continue;
        }
        scan.japanese_script_runs += 1;
        let address = section_address + run_start as u64;
        let end = address + storage.len() as u64;
        if text_ranges
            .iter()
            .any(|range| intersects(range, address, end))
        {
            scan.text_symbol_owned_japanese_runs += 1;
        } else if data_ranges
            .iter()
            .any(|range| range.start <= address && end <= range.end)
        {
            scan.data_symbol_owned_japanese_runs += 1;
        } else {
            scan.candidates.push(ScannedStorageGap {
                address,
                file_offset: section_file_offset + run_start as u64,
                bytes: storage.to_vec(),
                display_text,
                japanese_characters,
            });
        }
        run_start = terminator + 1;
    }
    scan
}

fn normalize_ranges(ranges: &mut Vec<NamedRange>) {
    ranges.sort_by_key(|range| (range.start, range.end));
    ranges.dedup();
}

fn intersects(range: &NamedRange, start: u64, end: u64) -> bool {
    range.start < end && start < range.end
}

#[cfg(test)]
#[path = "scanner_tests.rs"]
mod scanner_tests;
