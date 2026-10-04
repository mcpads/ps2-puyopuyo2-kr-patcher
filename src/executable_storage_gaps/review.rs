use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::{
    ExecutableStorageGapClassification, ExecutableStorageGapReviewCatalog,
    ExecutableStorageGapReviewedRegion,
};
use crate::executable_storage_gaps::scanner::ScannedStorageGap;

#[derive(Debug)]
pub(super) struct ReviewedStorageGaps {
    pub regions: Vec<ExecutableStorageGapReviewedRegion>,
    pub candidates: Vec<ReviewedGapCandidate>,
}

#[derive(Debug)]
pub(super) struct ReviewedGapCandidate {
    pub address: u64,
    pub file_offset: u64,
    pub bytes: Vec<u8>,
    pub display_text: String,
    pub japanese_characters: usize,
    pub review_region_id: String,
    pub classification: ExecutableStorageGapClassification,
    pub review_note: String,
}

struct ValidatedRegion {
    region_id: String,
    start: u64,
    end: u64,
    source_sha256: String,
    classification: ExecutableStorageGapClassification,
    note: String,
    candidate_count: usize,
}

pub(super) fn validate_review_catalog(
    catalog: &ExecutableStorageGapReviewCatalog,
    executable_sha256: &str,
    section_address: u64,
    section_data: &[u8],
    candidates: &[ScannedStorageGap],
) -> Result<ReviewedStorageGaps> {
    ensure!(catalog.schema == 1, "unsupported storage-gap review schema");
    ensure!(
        catalog.executable_sha256 == executable_sha256,
        "storage-gap review executable hash changed"
    );
    ensure!(
        !catalog.review_basis.trim().is_empty(),
        "storage-gap review basis is empty"
    );
    let section_end = section_address + u64::try_from(section_data.len())?;
    let mut ids = BTreeSet::new();
    let mut regions = catalog
        .regions
        .iter()
        .map(|region| {
            ensure!(
                ids.insert(region.region_id.clone()),
                "duplicate storage-gap review region: {}",
                region.region_id
            );
            ensure!(
                !region.note.trim().is_empty(),
                "storage-gap review note is empty: {}",
                region.region_id
            );
            let start = parse_address(&region.start_address)?;
            let end = parse_address(&region.end_address)?;
            ensure!(
                section_address <= start && start < end && end <= section_end,
                "storage-gap review region is outside main: {}",
                region.region_id
            );
            let bytes = &section_data[usize::try_from(start - section_address)?
                ..usize::try_from(end - section_address)?];
            ensure!(
                crate::indexed_texture::sha256(bytes) == region.source_sha256,
                "storage-gap review region hash changed: {}",
                region.region_id
            );
            Ok(ValidatedRegion {
                region_id: region.region_id.clone(),
                start,
                end,
                source_sha256: region.source_sha256.clone(),
                classification: region.classification,
                note: region.note.clone(),
                candidate_count: 0,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    regions.sort_by_key(|region| (region.start, region.end));
    for pair in regions.windows(2) {
        ensure!(
            pair[0].end <= pair[1].start,
            "storage-gap review regions overlap: {} and {}",
            pair[0].region_id,
            pair[1].region_id
        );
    }

    let mut reviewed_candidates = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let candidate_end = candidate.address + u64::try_from(candidate.bytes.len())?;
        let matches = regions
            .iter_mut()
            .filter(|region| region.start <= candidate.address && candidate_end <= region.end)
            .collect::<Vec<_>>();
        ensure!(
            matches.len() == 1,
            "storage-gap candidate at {:#010x} has {} review regions",
            candidate.address,
            matches.len()
        );
        let region = matches.into_iter().next().expect("exactly one region");
        region.candidate_count += 1;
        reviewed_candidates.push(ReviewedGapCandidate {
            address: candidate.address,
            file_offset: candidate.file_offset,
            bytes: candidate.bytes.clone(),
            display_text: candidate.display_text.clone(),
            japanese_characters: candidate.japanese_characters,
            review_region_id: region.region_id.clone(),
            classification: region.classification,
            review_note: region.note.clone(),
        });
    }
    ensure!(
        regions.iter().all(|region| region.candidate_count > 0),
        "storage-gap review contains a region without candidates"
    );
    let report_regions = regions
        .into_iter()
        .map(|region| {
            Ok(ExecutableStorageGapReviewedRegion {
                region_id: region.region_id,
                start_address: format!("{:#010x}", region.start),
                end_address: format!("{:#010x}", region.end),
                byte_length: usize::try_from(region.end - region.start)
                    .context("storage-gap review region length exceeds usize")?,
                source_sha256: region.source_sha256,
                classification: region.classification,
                candidate_count: region.candidate_count,
                note: region.note,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ReviewedStorageGaps {
        regions: report_regions,
        candidates: reviewed_candidates,
    })
}

fn parse_address(value: &str) -> Result<u64> {
    let digits = value
        .strip_prefix("0x")
        .with_context(|| format!("address lacks 0x prefix: {value}"))?;
    u64::from_str_radix(digits, 16).with_context(|| format!("invalid address: {value}"))
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod review_tests;
