use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::*;

struct ResidualPairDescriptor {
    file_id: usize,
    image_file_record: usize,
    palette_file_record: usize,
}

const RESIDUAL_PAIR_DESCRIPTORS: &[ResidualPairDescriptor] = &[
    ResidualPairDescriptor {
        file_id: 0,
        image_file_record: 2,
        palette_file_record: 61,
    },
    ResidualPairDescriptor {
        file_id: 0,
        image_file_record: 23,
        palette_file_record: 83,
    },
    ResidualPairDescriptor {
        file_id: 0,
        image_file_record: 24,
        palette_file_record: 84,
    },
    ResidualPairDescriptor {
        file_id: 0,
        image_file_record: 33,
        palette_file_record: 93,
    },
    ResidualPairDescriptor {
        file_id: 0,
        image_file_record: 42,
        palette_file_record: 102,
    },
    ResidualPairDescriptor {
        file_id: 1,
        image_file_record: 92,
        palette_file_record: 215,
    },
    ResidualPairDescriptor {
        file_id: 1,
        image_file_record: 99,
        palette_file_record: 222,
    },
    ResidualPairDescriptor {
        file_id: 1,
        image_file_record: 106,
        palette_file_record: 229,
    },
    ResidualPairDescriptor {
        file_id: 2,
        image_file_record: 71,
        palette_file_record: 401,
    },
];

pub(super) struct PlannedResidualPreview {
    pub key: PreviewKey,
    pub owner_tables: Vec<String>,
}

pub(super) struct ResidualPreviewPlan {
    pub planned: Vec<PlannedResidualPreview>,
    pub evidence: Vec<ResidualPreviewPair>,
}

pub(super) fn plan_residual_pairs(
    census: &crate::texture_load_census::TextureLoadCensus,
    consumer_pair_tables: &[ConsumerPairTable],
    containers: &BTreeMap<usize, LoadedContainer<'_>>,
) -> Result<ResidualPreviewPlan> {
    let unloaded = census
        .source_coverage
        .iter()
        .flat_map(|source| {
            source
                .unloaded_image_records
                .iter()
                .map(move |record| (source.file_id, *record))
        })
        .collect::<BTreeSet<_>>();
    let mut unregistered = BTreeMap::new();
    for table in consumer_pair_tables {
        ensure!(
            table.unpaired_image_load_indices.len() == table.unpaired_image_file_records.len(),
            "consumer-pair table has inconsistent unpaired image evidence"
        );
        for (load_index, file_record) in table
            .unpaired_image_load_indices
            .iter()
            .zip(&table.unpaired_image_file_records)
        {
            ensure!(
                unregistered
                    .insert(
                        (source_id_for_table(census, &table.symbol)?, *file_record),
                        (table.symbol.clone(), *load_index),
                    )
                    .is_none(),
                "an unregistered image belongs to multiple consumer-pair tables"
            );
        }
    }
    ensure!(
        unloaded.is_disjoint(&unregistered.keys().copied().collect()),
        "an image cannot be both unloaded and loaded-but-unregistered"
    );
    let residual_population = unloaded
        .iter()
        .copied()
        .chain(unregistered.keys().copied())
        .collect::<BTreeSet<_>>();
    let described_population = RESIDUAL_PAIR_DESCRIPTORS
        .iter()
        .map(|descriptor| (descriptor.file_id, descriptor.image_file_record))
        .collect::<BTreeSet<_>>();
    ensure!(
        described_population.len() == RESIDUAL_PAIR_DESCRIPTORS.len(),
        "residual pair descriptors contain duplicate images"
    );
    ensure!(
        residual_population == described_population,
        "residual image population changed"
    );

    let palette_flags = collect_palette_load_flags(census)?;
    let mut planned = Vec::new();
    let mut evidence = Vec::new();
    for descriptor in RESIDUAL_PAIR_DESCRIPTORS {
        let container = containers
            .get(&descriptor.file_id)
            .with_context(|| format!("residual container is missing: {}", descriptor.file_id))?;
        let image = container
            .file_records
            .get(descriptor.image_file_record)
            .context("residual image record is out of range")?;
        let palette = container
            .file_records
            .get(descriptor.palette_file_record)
            .context("residual palette record is out of range")?;
        ensure!(
            image.width > 0 && image.height > 0,
            "residual image descriptor does not name an image"
        );
        ensure!(
            palette.width == 0 && palette.height == 0 && palette.stored_byte_length == 256 * 4,
            "residual palette descriptor does not name an RGBA32 palette"
        );
        let flags = palette_flags
            .get(&(descriptor.file_id, descriptor.palette_file_record))
            .cloned()
            .unwrap_or_default();
        let alpha_variants = alpha_variants_for_load_flags(&flags);
        let reason = if unloaded.contains(&(descriptor.file_id, descriptor.image_file_record)) {
            ResidualImageReason::NotInLoadTable
        } else {
            let (load_table_symbol, image_load_index) = unregistered
                .get(&(descriptor.file_id, descriptor.image_file_record))
                .context("residual image has no reason")?;
            ResidualImageReason::LoadedButNotRegistered {
                load_table_symbol: load_table_symbol.clone(),
                image_load_index: *image_load_index,
            }
        };
        let owner_tables = match &reason {
            ResidualImageReason::NotInLoadTable => Vec::new(),
            ResidualImageReason::LoadedButNotRegistered {
                load_table_symbol, ..
            } => vec![load_table_symbol.clone()],
        };
        for clear_first_alpha in &alpha_variants {
            planned.push(PlannedResidualPreview {
                key: PreviewKey {
                    file_id: descriptor.file_id,
                    image_file_record: descriptor.image_file_record,
                    palette_file_record: descriptor.palette_file_record,
                    clear_first_alpha: *clear_first_alpha,
                },
                owner_tables: owner_tables.clone(),
            });
        }
        evidence.push(ResidualPreviewPair {
            file_id: descriptor.file_id,
            image_file_record: descriptor.image_file_record,
            palette_file_record: descriptor.palette_file_record,
            reason,
            palette_load_flags: flags.into_iter().collect(),
            preview_alpha_variants: alpha_variants,
            pairing_basis: "explicit parallel image/palette file-list position; visual survey only"
                .to_string(),
        });
    }
    Ok(ResidualPreviewPlan { planned, evidence })
}

fn alpha_variants_for_load_flags(flags: &BTreeSet<u32>) -> Vec<bool> {
    if flags.is_empty() {
        vec![false, true]
    } else {
        flags
            .iter()
            .map(|flags| flags & 2 != 0)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

fn source_id_for_table(
    census: &crate::texture_load_census::TextureLoadCensus,
    table_symbol: &str,
) -> Result<usize> {
    let candidate = census
        .candidates
        .iter()
        .find(|candidate| candidate.symbol == table_symbol)
        .with_context(|| format!("consumer-pair load table is absent: {table_symbol}"))?;
    let TextureTableClassification::LoadTable {
        compatible_sources, ..
    } = &candidate.classification
    else {
        anyhow::bail!("consumer-pair symbol is not a load table: {table_symbol}");
    };
    let [source] = compatible_sources.as_slice() else {
        anyhow::bail!("consumer-pair load table does not have exactly one source: {table_symbol}");
    };
    Ok(source.file_id)
}

fn collect_palette_load_flags(
    census: &crate::texture_load_census::TextureLoadCensus,
) -> Result<BTreeMap<(usize, usize), BTreeSet<u32>>> {
    let mut flags = BTreeMap::<(usize, usize), BTreeSet<u32>>::new();
    for candidate in &census.candidates {
        let TextureTableClassification::LoadTable {
            records,
            compatible_sources,
            ..
        } = &candidate.classification
        else {
            continue;
        };
        let [source] = compatible_sources.as_slice() else {
            anyhow::bail!("texture load table does not have exactly one source");
        };
        for record in records.iter().filter(|record| record.operation == 2) {
            flags
                .entry((source.file_id, record.file_record))
                .or_default()
                .insert(record.flags);
        }
    }
    Ok(flags)
}

#[cfg(test)]
#[path = "residual_pairs_tests.rs"]
mod residual_pairs_tests;
