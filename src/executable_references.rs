use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection, ObjectSymbol};
use r5900::{InstructionForm, Operands};
use serde::Serialize;

use crate::executable::validate_r5900_instruction;
use crate::executable_text::{FileBackedTextSymbol, collect_file_backed_text};
use crate::typed_evidence::{TypedInstructionEvidence, instruction_evidence};

const ADDRESS_MATERIALIZATION_WINDOW: usize = 20;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StaticReference {
    pub owner_symbol: String,
    pub address: String,
    pub kind: StaticReferenceKind,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticReferenceKind {
    TypedAddressMaterialization,
    Pointer,
}

#[derive(Clone, Debug, Serialize)]
pub struct DirectCallReference {
    pub owner_symbols: Vec<String>,
    pub address: String,
    pub target_symbol: String,
    pub instruction: TypedInstructionEvidence,
}

pub(crate) fn find_static_references(
    file: &object::File<'_>,
    target_addresses: &BTreeMap<u32, String>,
) -> Result<BTreeMap<String, Vec<StaticReference>>> {
    let mut references = BTreeMap::<String, BTreeSet<StaticReference>>::new();
    for symbol in file.symbols() {
        let Some((address, bytes)) = read_file_backed_symbol(file, &symbol)? else {
            continue;
        };
        let Ok(owner_symbol) = symbol.name() else {
            continue;
        };
        find_static_references_in_symbol(
            &bytes,
            address,
            owner_symbol,
            target_addresses,
            &mut references,
        )?;
    }
    Ok(references
        .into_iter()
        .map(|(symbol, references)| (symbol, references.into_iter().collect()))
        .collect())
}

pub(crate) fn find_direct_calls(
    file: &object::File<'_>,
    target_addresses: &BTreeMap<u32, String>,
) -> Result<Vec<DirectCallReference>> {
    let corpus = collect_file_backed_text(file)?;
    let mut calls = Vec::new();
    for symbol in &corpus.symbols {
        find_direct_calls_in_symbol(symbol, target_addresses, &mut calls)?;
    }
    calls.sort_by_key(|call| parse_hex_address(&call.address));
    Ok(calls)
}

fn find_static_references_in_symbol(
    source: &[u8],
    start_address: u64,
    owner_symbol: &str,
    target_addresses: &BTreeMap<u32, String>,
    references: &mut BTreeMap<String, BTreeSet<StaticReference>>,
) -> Result<()> {
    find_pointer_references(
        source,
        start_address,
        owner_symbol,
        target_addresses,
        references,
    )?;
    find_address_materializations(
        source,
        start_address,
        owner_symbol,
        target_addresses,
        references,
    )
}

fn find_pointer_references(
    source: &[u8],
    start_address: u64,
    owner_symbol: &str,
    target_addresses: &BTreeMap<u32, String>,
    references: &mut BTreeMap<String, BTreeSet<StaticReference>>,
) -> Result<()> {
    for offset in (0..source.len().saturating_sub(3)).step_by(4) {
        let value = u32::from_le_bytes(source[offset..offset + 4].try_into()?);
        let Some(target_symbol) = target_addresses.get(&value) else {
            continue;
        };
        references
            .entry(target_symbol.clone())
            .or_default()
            .insert(StaticReference {
                owner_symbol: owner_symbol.to_string(),
                address: format!("{:#010x}", start_address + u64::try_from(offset)?),
                kind: StaticReferenceKind::Pointer,
            });
    }
    Ok(())
}

fn find_address_materializations(
    source: &[u8],
    start_address: u64,
    owner_symbol: &str,
    target_addresses: &BTreeMap<u32, String>,
    references: &mut BTreeMap<String, BTreeSet<StaticReference>>,
) -> Result<()> {
    for upper_offset in (0..source.len().saturating_sub(3)).step_by(4) {
        let upper_word = u32::from_le_bytes(source[upper_offset..upper_offset + 4].try_into()?);
        if upper_word >> 26 != 0x0f {
            continue;
        }
        let upper_register = (upper_word >> 16) & 0x1f;
        let upper_immediate = upper_word as u16;
        let search_end = (upper_offset + ADDRESS_MATERIALIZATION_WINDOW + 4).min(source.len());
        for lower_offset in ((upper_offset + 4)..search_end).step_by(4) {
            let lower_word = u32::from_le_bytes(source[lower_offset..lower_offset + 4].try_into()?);
            if lower_word >> 26 != 0x09
                || (lower_word >> 21) & 0x1f != upper_register
                || (lower_word >> 16) & 0x1f != upper_register
            {
                continue;
            }
            let lower_immediate = lower_word as u16 as i16;
            let resolved =
                (u32::from(upper_immediate) << 16).wrapping_add_signed(i32::from(lower_immediate));
            let Some(target_symbol) = target_addresses.get(&resolved) else {
                continue;
            };
            validate_typed_address_pair(
                source,
                start_address,
                upper_offset,
                lower_offset,
                upper_immediate,
                lower_immediate,
            )?;
            references
                .entry(target_symbol.clone())
                .or_default()
                .insert(StaticReference {
                    owner_symbol: owner_symbol.to_string(),
                    address: format!("{:#010x}", start_address + u64::try_from(upper_offset)?),
                    kind: StaticReferenceKind::TypedAddressMaterialization,
                });
        }
    }
    Ok(())
}

fn find_direct_calls_in_symbol(
    symbol: &FileBackedTextSymbol,
    target_addresses: &BTreeMap<u32, String>,
    calls: &mut Vec<DirectCallReference>,
) -> Result<()> {
    for (index, chunk) in symbol.bytes.chunks_exact(4).enumerate() {
        let bytes: [u8; 4] = chunk.try_into().expect("four-byte instruction");
        let word = u32::from_le_bytes(bytes);
        if word >> 26 != 0x03 {
            continue;
        }
        let offset = index.checked_mul(4).context("call offset overflow")?;
        let address = symbol.address + u64::try_from(offset)?;
        let instruction = validate_r5900_instruction(&bytes, address)?;
        let Operands::Jump { target } = instruction.operands() else {
            anyhow::bail!("raw jal candidate does not have a typed jump operand")
        };
        ensure!(
            instruction.form() == InstructionForm::Jal,
            "raw jal candidate is not typed as jal"
        );
        let Some(target_symbol) = target_addresses.get(target) else {
            continue;
        };
        calls.push(DirectCallReference {
            owner_symbols: symbol.owners.iter().cloned().collect(),
            address: format!("{address:#010x}"),
            target_symbol: target_symbol.clone(),
            instruction: instruction_evidence(&bytes, address, &instruction),
        });
    }
    Ok(())
}

fn validate_typed_address_pair(
    source: &[u8],
    start_address: u64,
    upper_offset: usize,
    lower_offset: usize,
    expected_upper: u16,
    expected_lower: i16,
) -> Result<()> {
    let upper = validate_r5900_instruction(
        &source[upper_offset..upper_offset + 4],
        start_address + u64::try_from(upper_offset)?,
    )?;
    let lower = validate_r5900_instruction(
        &source[lower_offset..lower_offset + 4],
        start_address + u64::try_from(lower_offset)?,
    )?;
    let Operands::LoadUpper {
        destination,
        immediate: upper_immediate,
    } = upper.operands()
    else {
        anyhow::bail!("typed upper address is not load-upper")
    };
    let Operands::SignedImmediate {
        destination: lower_destination,
        source: lower_source,
        immediate: lower_immediate,
    } = lower.operands()
    else {
        anyhow::bail!("typed lower address is not signed-immediate")
    };
    ensure!(
        upper.form() == InstructionForm::Lui
            && lower.form() == InstructionForm::Addiu
            && destination == lower_destination
            && destination == lower_source
            && *upper_immediate == expected_upper
            && *lower_immediate == expected_lower,
        "typed address operands differ from raw candidate"
    );
    Ok(())
}

fn read_file_backed_symbol(
    file: &object::File<'_>,
    symbol: &object::Symbol<'_, '_>,
) -> Result<Option<(u64, Vec<u8>)>> {
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
    let end = start
        .checked_add(usize::try_from(symbol.size())?)
        .context("reference owner range overflow")?;
    Ok(section
        .data()?
        .get(start..end)
        .map(|bytes| (symbol.address(), bytes.to_vec())))
}

fn parse_hex_address(address: &str) -> u64 {
    u64::from_str_radix(address.trim_start_matches("0x"), 16)
        .expect("internally formatted hexadecimal address")
}

#[cfg(test)]
#[path = "executable_references/executable_references_tests.rs"]
mod executable_references_tests;
