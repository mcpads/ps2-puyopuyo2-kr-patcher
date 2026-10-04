use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use super::*;
use crate::cpx::{read_symbol_bytes, read_u32_at};
use crate::indexed_texture::{TextureRegistrationPair, parse_texture_registration_pairs};

struct ConsumerPairDescriptor {
    load_table_symbol: &'static str,
    pointer_table_symbol: Option<&'static str>,
    registration_set_symbols: &'static [&'static str],
    expected_unpaired_image_load_indices: &'static [usize],
}

const TITLE_REGISTRATION_SETS: &[&str] = &["TitleTex_0", "TitleTex_1", "TitleTex_2"];
const DEMO_4D_REGISTRATION_SETS: &[&str] =
    &["DEMOTex4D_1", "DEMOTex4D_2", "DEMOTex4D_3", "DEMOTex4D_4"];
const NAZO_REGISTRATION_SETS: &[&str] = &[
    "Nazo_TexReg_Menu",
    "Nazo_TexReg_Nazo",
    "Nazo_TexReg_Game",
    "Nazo_TexReg_Edit_0",
    "Nazo_TexReg_Edit_1",
    "Nazo_TexReg_Edit_2",
    "Nazo_TexReg_Edit_4",
    "Nazo_TexReg_Present",
];

const CONSUMER_PAIR_DESCRIPTORS: &[ConsumerPairDescriptor] = &[
    ConsumerPairDescriptor {
        load_table_symbol: "TitleTexTbl",
        pointer_table_symbol: None,
        registration_set_symbols: TITLE_REGISTRATION_SETS,
        expected_unpaired_image_load_indices: &[],
    },
    ConsumerPairDescriptor {
        load_table_symbol: "DEMOTexTbl4D",
        pointer_table_symbol: Some("DEMOTex4D"),
        registration_set_symbols: DEMO_4D_REGISTRATION_SETS,
        expected_unpaired_image_load_indices: &[],
    },
    ConsumerPairDescriptor {
        load_table_symbol: "NazoTexTbl",
        pointer_table_symbol: None,
        registration_set_symbols: NAZO_REGISTRATION_SETS,
        expected_unpaired_image_load_indices: &[12, 19],
    },
];

pub(super) struct ConsumerPairResolution {
    pub pairs: Vec<PreviewKey>,
    pub evidence: ConsumerPairTable,
}

pub(super) fn resolve_consumer_pairs(
    file: &object::File<'_>,
    load_table_symbol: &str,
    file_id: usize,
    load_records: &[TextureLoadRecord],
) -> Result<Option<ConsumerPairResolution>> {
    let Some(descriptor) = CONSUMER_PAIR_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.load_table_symbol == load_table_symbol)
    else {
        return Ok(None);
    };
    let mut registration_sets = Vec::new();
    let mut pair_instances = Vec::new();
    let mut set_addresses = Vec::new();
    for symbol in descriptor.registration_set_symbols {
        let (address, bytes) = read_symbol_bytes(file, symbol)?;
        let pairs = parse_texture_registration_pairs(symbol, &bytes)?;
        set_addresses.push(u32::try_from(address)?);
        pair_instances.extend(pairs.iter().copied());
        registration_sets.push(ConsumerRegistrationSet {
            symbol: (*symbol).to_string(),
            pairs: pairs
                .into_iter()
                .map(|pair| ConsumerRegistrationPair {
                    image_load_index: pair.image_load_index,
                    palette_slot: pair.palette_slot,
                })
                .collect(),
        });
    }
    if let Some(pointer_table_symbol) = descriptor.pointer_table_symbol {
        validate_pointer_table(file, pointer_table_symbol, &set_addresses)?;
    }
    let mapped = map_registration_pairs(file_id, load_records, &pair_instances)?;
    let image_load_count = load_records
        .iter()
        .filter(|record| record.operation == 1)
        .count();
    let palette_load_count = load_records
        .iter()
        .filter(|record| record.operation == 2)
        .count();
    let paired_image_indices = pair_instances
        .iter()
        .map(|pair| pair.image_load_index)
        .collect::<BTreeSet<_>>();
    let unpaired_image_load_indices = (0..image_load_count)
        .filter(|index| !paired_image_indices.contains(index))
        .collect::<Vec<_>>();
    let images = load_records
        .iter()
        .filter(|record| record.operation == 1)
        .collect::<Vec<_>>();
    let unpaired_image_file_records = unpaired_image_load_indices
        .iter()
        .map(|index| images[*index].file_record)
        .collect::<Vec<_>>();
    ensure!(
        unpaired_image_load_indices == descriptor.expected_unpaired_image_load_indices,
        "{load_table_symbol} consumer registration coverage changed"
    );
    ensure!(
        mapped.iter().copied().collect::<BTreeSet<_>>().len()
            == pair_instances
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len(),
        "{load_table_symbol} registration pairs collapse after load mapping"
    );
    let unique_pairs = pair_instances
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len();
    Ok(Some(ConsumerPairResolution {
        pairs: mapped,
        evidence: ConsumerPairTable {
            symbol: load_table_symbol.to_string(),
            pointer_table_symbol: descriptor.pointer_table_symbol.map(str::to_string),
            registration_sets,
            image_loads: image_load_count,
            palette_loads: palette_load_count,
            pair_instances: pair_instances.len(),
            unique_pairs,
            paired_image_loads: paired_image_indices.len(),
            unpaired_image_load_indices,
            unpaired_image_file_records,
        },
    }))
}

fn map_registration_pairs(
    file_id: usize,
    load_records: &[TextureLoadRecord],
    pairs: &[TextureRegistrationPair],
) -> Result<Vec<PreviewKey>> {
    let images = load_records
        .iter()
        .filter(|record| record.operation == 1)
        .collect::<Vec<_>>();
    let palettes = load_records
        .iter()
        .filter(|record| record.operation == 2)
        .collect::<Vec<_>>();
    pairs
        .iter()
        .map(|pair| {
            let image = images.get(pair.image_load_index).with_context(|| {
                format!(
                    "registration image index is out of range: {}",
                    pair.image_load_index
                )
            })?;
            let palette = palettes.get(pair.palette_slot).with_context(|| {
                format!(
                    "registration palette slot is out of range: {}",
                    pair.palette_slot
                )
            })?;
            Ok(PreviewKey {
                file_id,
                image_file_record: image.file_record,
                palette_file_record: palette.file_record,
                clear_first_alpha: palette.flags & 2 != 0,
            })
        })
        .collect()
}

fn validate_pointer_table(
    file: &object::File<'_>,
    symbol: &str,
    expected_addresses: &[u32],
) -> Result<()> {
    let (_, bytes) = read_symbol_bytes(file, symbol)?;
    ensure!(
        bytes.len() == expected_addresses.len() * 4,
        "{symbol} pointer count changed"
    );
    let addresses = bytes
        .chunks_exact(4)
        .map(|bytes| read_u32_at(bytes, 0))
        .collect::<Result<Vec<_>>>()?;
    ensure!(addresses == expected_addresses, "{symbol} pointers changed");
    Ok(())
}

#[cfg(test)]
#[path = "consumer_pairs_tests.rs"]
mod consumer_pairs_tests;
