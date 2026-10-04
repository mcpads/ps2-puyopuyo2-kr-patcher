use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::{
    InvalidFileRecordReference, TextureCallSourceCoverage, TextureLoadArgumentSource,
    TextureLoadCall,
};
use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::{parse_file_records, parse_load_records};
use crate::texture_load_census::{TextureLoadCensus, TextureTableClassification};

pub(super) struct FileRecordAudit {
    pub(super) source_images: usize,
    pub(super) referenced_images: usize,
    pub(super) unreferenced_images: usize,
    pub(super) source_coverage: Vec<TextureCallSourceCoverage>,
    pub(super) invalid_record_references: Vec<InvalidFileRecordReference>,
}

pub(super) fn audit_file_record_coverage(
    file: &object::File<'_>,
    census: &TextureLoadCensus,
    calls: &[TextureLoadCall],
) -> Result<FileRecordAudit> {
    let mut source_records = BTreeMap::new();
    for source in &census.file_sources {
        let (_, bytes) = read_symbol_bytes(file, &source.file_list_symbol)?;
        let records = parse_file_records(&bytes)?;
        ensure!(
            records.len() == source.file_records,
            "file-list length changed during call audit"
        );
        source_records.insert(source.file_id, records);
    }
    let mut referenced_images = BTreeMap::<usize, BTreeSet<usize>>::new();
    let mut invalid_record_references = Vec::new();
    for call in calls.iter().filter(|call| call.file_id <= 2) {
        let records = source_records
            .get(&call.file_id)
            .with_context(|| format!("image file id has no source: {}", call.file_id))?;
        let table_symbols = match &call.source {
            TextureLoadArgumentSource::DirectLoadTable { symbol } => vec![symbol.clone()],
            TextureLoadArgumentSource::PointerTable { .. } => call.possible_load_tables.clone(),
            TextureLoadArgumentSource::OtherStaticTable {
                symbol: Some(symbol),
                ..
            } => vec![symbol.clone()],
            TextureLoadArgumentSource::OtherStaticTable { symbol: None, .. }
            | TextureLoadArgumentSource::OtherDynamic => anyhow::bail!(
                "image-file call has no named static table: {}",
                call.call_address
            ),
        };
        let invalid_before = invalid_record_references.len();
        for table_symbol in table_symbols {
            for (operation, file_record) in table_records(file, census, &table_symbol)? {
                let validity = records.get(file_record).map(|record| {
                    file_record_matches_operation(
                        operation,
                        record.width,
                        record.height,
                        record.stored_byte_length,
                    )
                });
                if validity == Some(true) {
                    if operation == 1 {
                        referenced_images
                            .entry(call.file_id)
                            .or_default()
                            .insert(file_record);
                    }
                    continue;
                }
                let reason = match validity {
                    None => "file record is out of range",
                    Some(false) => "file record role differs from the load operation",
                    Some(true) => unreachable!(),
                };
                invalid_record_references.push(InvalidFileRecordReference {
                    owner_symbol: call.owner_symbol.clone(),
                    call_address: call.call_address.clone(),
                    file_id: call.file_id,
                    table_symbol: table_symbol.clone(),
                    operation,
                    file_record,
                    reason: reason.to_string(),
                });
            }
        }
        ensure!(
            call.source_compatible != Some(true)
                || invalid_record_references.len() == invalid_before,
            "source-compatible call produced an invalid file-record reference"
        );
    }
    invalid_record_references.sort_by(|left, right| {
        (
            &left.call_address,
            &left.table_symbol,
            left.operation,
            left.file_record,
        )
            .cmp(&(
                &right.call_address,
                &right.table_symbol,
                right.operation,
                right.file_record,
            ))
    });
    invalid_record_references.dedup_by(|left, right| {
        left.call_address == right.call_address
            && left.table_symbol == right.table_symbol
            && left.operation == right.operation
            && left.file_record == right.file_record
    });

    let mut source_coverage = Vec::new();
    for source in &census.file_sources {
        let records = &source_records[&source.file_id];
        let images = records
            .iter()
            .enumerate()
            .filter(|(_, record)| record.width > 0 && record.height > 0)
            .map(|(index, _)| index)
            .collect::<BTreeSet<_>>();
        let referenced = referenced_images
            .get(&source.file_id)
            .cloned()
            .unwrap_or_default();
        ensure!(
            referenced.is_subset(&images),
            "referenced image set contains a non-image record"
        );
        source_coverage.push(TextureCallSourceCoverage {
            file_id: source.file_id,
            iso_path: source.iso_path.clone(),
            image_records: images.len(),
            referenced_image_records: referenced.len(),
            unreferenced_image_records: images.difference(&referenced).copied().collect(),
        });
    }
    Ok(FileRecordAudit {
        source_images: source_coverage
            .iter()
            .map(|source| source.image_records)
            .sum(),
        referenced_images: source_coverage
            .iter()
            .map(|source| source.referenced_image_records)
            .sum(),
        unreferenced_images: source_coverage
            .iter()
            .map(|source| source.unreferenced_image_records.len())
            .sum(),
        source_coverage,
        invalid_record_references,
    })
}

fn table_records(
    file: &object::File<'_>,
    census: &TextureLoadCensus,
    symbol: &str,
) -> Result<Vec<(u32, usize)>> {
    if let Some(candidate) = census
        .candidates
        .iter()
        .find(|candidate| candidate.symbol == symbol)
    {
        return match &candidate.classification {
            TextureTableClassification::LoadTable { records, .. } => Ok(records
                .iter()
                .map(|record| (record.operation, record.file_record))
                .collect()),
            TextureTableClassification::EmptyLoadTable { .. } => Ok(Vec::new()),
            _ => anyhow::bail!("image-file call names a non-load candidate: {symbol}"),
        };
    }
    let (_, bytes) = read_symbol_bytes(file, symbol)?;
    parse_load_records(&bytes).map(|records| {
        records
            .into_iter()
            .map(|record| (record.operation, record.file_record))
            .collect()
    })
}

fn file_record_matches_operation(
    operation: u32,
    width: u32,
    height: u32,
    stored_byte_length: u32,
) -> bool {
    match operation {
        0 => true,
        1 => width > 0 && height > 0,
        2 => width == 0 && height == 0 && stored_byte_length == 256 * 4,
        _ => false,
    }
}

#[cfg(test)]
#[path = "record_coverage_tests.rs"]
mod record_coverage_tests;
