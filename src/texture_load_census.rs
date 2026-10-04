use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object, ObjectSection, ObjectSymbol};
use serde::Serialize;

use crate::cpx::read_u32_at;
use crate::indexed_texture::{LoadRecord, parse_load_records, sha256};

mod elf;

use elf::{find_table_references, read_file_sources};

#[cfg(test)]
use elf::{find_address_materializations, find_pointer_references};

#[derive(Clone, Debug)]
pub struct TextureLoadCensusInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureLoadCensus {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub file_sources: Vec<TextureFileSource>,
    pub source_coverage: Vec<TextureSourceCoverage>,
    pub summary: TextureLoadCensusSummary,
    pub candidates: Vec<TextureTableCandidate>,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureFileSource {
    pub file_id: usize,
    pub iso_path: String,
    pub file_list_symbol: String,
    pub file_records: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureSourceCoverage {
    pub file_id: usize,
    pub iso_path: String,
    pub image_records: usize,
    pub loaded_image_records: usize,
    pub unloaded_image_records: Vec<usize>,
    pub palette_records: usize,
    pub loaded_palette_records: usize,
    pub unloaded_palette_records: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureLoadCensusSummary {
    pub table_candidates: usize,
    pub load_tables: usize,
    pub named_load_tables: usize,
    pub structurally_discovered_load_tables: usize,
    pub other_named_tables: usize,
    pub pointer_tables: usize,
    pub empty_load_tables: usize,
    pub unclassified_named_tables: usize,
    pub load_records: usize,
    pub image_loads: usize,
    pub unique_image_records: usize,
    pub palette_loads: usize,
    pub unique_palette_records: usize,
    pub auxiliary_loads: usize,
    pub uniquely_mapped_tables: usize,
    pub ambiguously_mapped_tables: usize,
    pub unmapped_tables: usize,
    pub referenced_tables: usize,
    pub unreferenced_tables: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureTableCandidate {
    pub symbol: String,
    pub address: String,
    pub byte_length: u64,
    pub classification: TextureTableClassification,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextureTableClassification {
    LoadTable {
        records: Vec<TextureLoadRecord>,
        compatible_sources: Vec<TextureCompatibleSource>,
        references: Vec<TextureTableReference>,
    },
    PointerTable {
        load_table_symbols: Vec<String>,
        references: Vec<TextureTableReference>,
    },
    EmptyLoadTable {
        references: Vec<TextureTableReference>,
    },
    OtherNamedTable {
        reason: String,
        references: Vec<TextureTableReference>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureLoadRecord {
    pub flags: u32,
    pub operation: u32,
    pub file_record: usize,
    pub destination: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureCompatibleSource {
    pub file_id: usize,
    pub iso_path: String,
    pub file_list_symbol: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TextureTableReference {
    pub owner_symbol: String,
    pub address: String,
    pub kind: TextureTableReferenceKind,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureTableReferenceKind {
    TypedAddressMaterialization,
    Pointer,
}

pub(super) struct FileSourceRecords {
    pub(super) evidence: TextureFileSource,
    pub(super) records: Vec<crate::indexed_texture::FileRecord>,
}

pub fn survey_texture_load_tables(
    inputs: TextureLoadCensusInputs<'_>,
) -> Result<TextureLoadCensus> {
    ensure!(
        sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    let file = object::File::parse(inputs.executable).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips,
        "ELF architecture is not 32-bit MIPS"
    );
    ensure!(
        file.endianness() == Endianness::Little,
        "ELF is not little-endian"
    );

    let file_sources = read_file_sources(&file)?;
    let symbols = read_texture_table_candidates(&file, &file_sources)?;
    let candidate_addresses = symbols
        .iter()
        .map(|symbol| Ok((u32::try_from(symbol.address)?, symbol.name.clone())))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let load_table_addresses = symbols
        .iter()
        .filter(|symbol| symbol.load_records.is_some())
        .map(|symbol| Ok((u32::try_from(symbol.address)?, symbol.name.clone())))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let references = find_table_references(&file, &candidate_addresses)?;

    let mut candidates = Vec::with_capacity(symbols.len());
    let mut loaded_image_records = BTreeMap::<usize, BTreeSet<usize>>::new();
    let mut loaded_palette_records = BTreeMap::<usize, BTreeSet<usize>>::new();
    let mut summary = TextureLoadCensusSummary {
        table_candidates: symbols.len(),
        load_tables: 0,
        named_load_tables: 0,
        structurally_discovered_load_tables: 0,
        other_named_tables: 0,
        pointer_tables: 0,
        empty_load_tables: 0,
        unclassified_named_tables: 0,
        load_records: 0,
        image_loads: 0,
        unique_image_records: 0,
        palette_loads: 0,
        unique_palette_records: 0,
        auxiliary_loads: 0,
        uniquely_mapped_tables: 0,
        ambiguously_mapped_tables: 0,
        unmapped_tables: 0,
        referenced_tables: 0,
        unreferenced_tables: 0,
    };
    for symbol in symbols {
        let classification = match symbol.load_records {
            Some(records) => {
                summary.load_tables += 1;
                if symbol.name.contains("TexTbl") {
                    summary.named_load_tables += 1;
                } else {
                    summary.structurally_discovered_load_tables += 1;
                }
                summary.load_records += records.len();
                for record in &records {
                    match record.operation {
                        0 => summary.auxiliary_loads += 1,
                        1 => summary.image_loads += 1,
                        2 => summary.palette_loads += 1,
                        _ => unreachable!("load record operations were validated"),
                    }
                }
                let compatible_sources = file_sources
                    .iter()
                    .filter(|source| load_table_matches_source(&records, &source.records))
                    .map(|source| TextureCompatibleSource {
                        file_id: source.evidence.file_id,
                        iso_path: source.evidence.iso_path.clone(),
                        file_list_symbol: source.evidence.file_list_symbol.clone(),
                    })
                    .collect::<Vec<_>>();
                match compatible_sources.len() {
                    0 => summary.unmapped_tables += 1,
                    1 => summary.uniquely_mapped_tables += 1,
                    _ => summary.ambiguously_mapped_tables += 1,
                }
                if let [source] = compatible_sources.as_slice() {
                    for record in &records {
                        match record.operation {
                            1 => {
                                loaded_image_records
                                    .entry(source.file_id)
                                    .or_default()
                                    .insert(record.file_record);
                            }
                            2 => {
                                loaded_palette_records
                                    .entry(source.file_id)
                                    .or_default()
                                    .insert(record.file_record);
                            }
                            _ => {}
                        }
                    }
                }
                let table_references = references
                    .get(symbol.name.as_str())
                    .cloned()
                    .unwrap_or_default();
                if table_references.is_empty() {
                    summary.unreferenced_tables += 1;
                } else {
                    summary.referenced_tables += 1;
                }
                TextureTableClassification::LoadTable {
                    records: records.into_iter().map(TextureLoadRecord::from).collect(),
                    compatible_sources,
                    references: table_references,
                }
            }
            None => {
                summary.other_named_tables += 1;
                let table_references = references
                    .get(symbol.name.as_str())
                    .cloned()
                    .unwrap_or_default();
                if is_empty_load_table(&symbol.bytes) {
                    summary.empty_load_tables += 1;
                    TextureTableClassification::EmptyLoadTable {
                        references: table_references,
                    }
                } else if let Some(load_table_symbols) =
                    read_pointer_table(&symbol.bytes, &load_table_addresses)?
                {
                    summary.pointer_tables += 1;
                    TextureTableClassification::PointerTable {
                        load_table_symbols,
                        references: table_references,
                    }
                } else {
                    summary.unclassified_named_tables += 1;
                    let error = parse_load_records(&symbol.bytes)
                        .and_then(validate_load_record_values)
                        .expect_err("non-load candidate unexpectedly passed load validation");
                    TextureTableClassification::OtherNamedTable {
                        reason: format!("{error:#}"),
                        references: table_references,
                    }
                }
            }
        };
        candidates.push(TextureTableCandidate {
            symbol: symbol.name,
            address: format!("{:#010x}", symbol.address),
            byte_length: u64::try_from(symbol.bytes.len())?,
            classification,
        });
    }

    summary.unique_image_records = loaded_image_records.values().map(BTreeSet::len).sum();
    summary.unique_palette_records = loaded_palette_records.values().map(BTreeSet::len).sum();
    let source_coverage = build_source_coverage(
        &file_sources,
        &loaded_image_records,
        &loaded_palette_records,
    );

    Ok(TextureLoadCensus {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        source_coverage,
        file_sources: file_sources
            .into_iter()
            .map(|source| source.evidence)
            .collect(),
        summary,
        candidates,
        evidence_boundary: "Static ELF census of structurally valid texture load tables, named non-load texture tables, compatible file lists, and direct address references; it does not by itself prove registration, display, baked-language content, or runtime reachability.",
    })
}

fn build_source_coverage(
    file_sources: &[FileSourceRecords],
    loaded_image_records: &BTreeMap<usize, BTreeSet<usize>>,
    loaded_palette_records: &BTreeMap<usize, BTreeSet<usize>>,
) -> Vec<TextureSourceCoverage> {
    file_sources
        .iter()
        .map(|source| {
            let image_records = source
                .records
                .iter()
                .enumerate()
                .filter(|(_, record)| record.width > 0 && record.height > 0)
                .map(|(index, _)| index)
                .collect::<BTreeSet<_>>();
            let palette_records = source
                .records
                .iter()
                .enumerate()
                .filter(|(_, record)| {
                    record.width == 0 && record.height == 0 && record.stored_byte_length == 256 * 4
                })
                .map(|(index, _)| index)
                .collect::<BTreeSet<_>>();
            let loaded_images = loaded_image_records
                .get(&source.evidence.file_id)
                .cloned()
                .unwrap_or_default();
            let loaded_palettes = loaded_palette_records
                .get(&source.evidence.file_id)
                .cloned()
                .unwrap_or_default();
            TextureSourceCoverage {
                file_id: source.evidence.file_id,
                iso_path: source.evidence.iso_path.clone(),
                image_records: image_records.len(),
                loaded_image_records: loaded_images.len(),
                unloaded_image_records: image_records.difference(&loaded_images).copied().collect(),
                palette_records: palette_records.len(),
                loaded_palette_records: loaded_palettes.len(),
                unloaded_palette_records: palette_records
                    .difference(&loaded_palettes)
                    .copied()
                    .collect(),
            }
        })
        .collect()
}

impl From<LoadRecord> for TextureLoadRecord {
    fn from(record: LoadRecord) -> Self {
        Self {
            flags: record.flags,
            operation: record.operation,
            file_record: record.file_record,
            destination: format!("{:#010x}", record.destination),
        }
    }
}

struct NamedSymbolBytes {
    name: String,
    address: u64,
    bytes: Vec<u8>,
    load_records: Option<Vec<LoadRecord>>,
}

fn read_texture_table_candidates(
    file: &object::File<'_>,
    file_sources: &[FileSourceRecords],
) -> Result<Vec<NamedSymbolBytes>> {
    let mut symbols = Vec::new();
    for symbol in file.symbols() {
        let Ok(name) = symbol.name() else {
            continue;
        };
        let Some(bytes) = read_file_backed_symbol_bytes(file, &symbol)? else {
            continue;
        };
        let load_records = parse_load_records(&bytes)
            .and_then(validate_load_record_values)
            .ok()
            .filter(|records| {
                file_sources
                    .iter()
                    .any(|source| load_table_matches_source(records, &source.records))
            });
        if load_records.is_none() && !name.contains("TexTbl") {
            continue;
        }
        symbols.push(NamedSymbolBytes {
            name: name.to_string(),
            address: symbol.address(),
            bytes,
            load_records,
        });
    }
    symbols.sort_by_key(|symbol| symbol.address);
    Ok(symbols)
}

fn read_file_backed_symbol_bytes(
    file: &object::File<'_>,
    symbol: &object::Symbol<'_, '_>,
) -> Result<Option<Vec<u8>>> {
    if !symbol.is_definition() || symbol.size() == 0 {
        return Ok(None);
    }
    let Some(section_index) = symbol.section_index() else {
        return Ok(None);
    };
    let section = file.section_by_index(section_index)?;
    let Some(relative) = symbol.address().checked_sub(section.address()) else {
        return Ok(None);
    };
    let Some((_, section_file_size)) = section.file_range() else {
        return Ok(None);
    };
    if relative + symbol.size() > section_file_size {
        return Ok(None);
    }
    let start = usize::try_from(relative)?;
    let Some(end) = start.checked_add(usize::try_from(symbol.size())?) else {
        return Ok(None);
    };
    Ok(section.data()?.get(start..end).map(<[u8]>::to_vec))
}

fn validate_load_record_values(records: Vec<LoadRecord>) -> Result<Vec<LoadRecord>> {
    ensure!(!records.is_empty(), "texture load table has no records");
    for record in &records {
        ensure!(
            record.flags <= 3,
            "texture load flags exceed the known bit field"
        );
        ensure!(
            record.operation <= 2,
            "texture load operation is outside the known 0..=2 range"
        );
    }
    ensure!(
        records.iter().any(|record| record.operation == 1)
            && records.iter().any(|record| record.operation == 2),
        "texture load table does not contain both image and palette records"
    );
    Ok(records)
}

fn is_empty_load_table(source: &[u8]) -> bool {
    source.len() == 16
        && source[..4] == u32::MAX.to_le_bytes()
        && source[4..].iter().all(|byte| *byte == 0)
}

fn read_pointer_table(
    source: &[u8],
    load_table_addresses: &BTreeMap<u32, String>,
) -> Result<Option<Vec<String>>> {
    if source.is_empty() || !source.len().is_multiple_of(4) {
        return Ok(None);
    }
    let mut symbols = Vec::with_capacity(source.len() / 4);
    for offset in (0..source.len()).step_by(4) {
        let address = read_u32_at(source, offset)?;
        let Some(symbol) = load_table_addresses.get(&address) else {
            return Ok(None);
        };
        symbols.push(symbol.clone());
    }
    Ok(Some(symbols))
}

fn load_table_matches_source(
    loads: &[LoadRecord],
    records: &[crate::indexed_texture::FileRecord],
) -> bool {
    loads.iter().all(|load| {
        let Some(record) = records.get(load.file_record) else {
            return false;
        };
        match load.operation {
            0 => true,
            1 => record.width > 0 && record.height > 0,
            2 => record.width == 0 && record.height == 0 && record.stored_byte_length == 256 * 4,
            _ => false,
        }
    })
}

#[cfg(test)]
#[path = "texture_load_census/texture_load_census_tests.rs"]
mod texture_load_census_tests;
