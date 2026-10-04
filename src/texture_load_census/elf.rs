use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection, ObjectSymbol};
use r5900::{InstructionForm, Operands};

use super::{
    FileSourceRecords, TextureFileSource, TextureTableReference, TextureTableReferenceKind,
};
use crate::cpx::{read_symbol_bytes, read_u32_at};
use crate::executable::validate_r5900_instruction;
use crate::indexed_texture::parse_file_records;

const FILE_NAME_TABLE_SYMBOL: &str = "FileListName";
const FILE_LIST_TABLE_SYMBOL: &str = "FileListList";
const ADDRESS_MATERIALIZATION_WINDOW: usize = 20;

pub(super) fn read_file_sources(file: &object::File<'_>) -> Result<Vec<FileSourceRecords>> {
    let (_, name_table) = read_symbol_bytes(file, FILE_NAME_TABLE_SYMBOL)?;
    let (_, list_table) = read_symbol_bytes(file, FILE_LIST_TABLE_SYMBOL)?;
    ensure!(
        name_table.len() == list_table.len() && name_table.len().is_multiple_of(4),
        "file-list pointer arrays differ"
    );
    let mut sources = Vec::new();
    for file_id in 0..name_table.len() / 4 {
        let name_address = u64::from(read_u32_at(&name_table, file_id * 4)?);
        let list_address = u64::from(read_u32_at(&list_table, file_id * 4)?);
        if name_address == 0 && list_address == 0 {
            continue;
        }
        ensure!(
            name_address != 0 && list_address != 0,
            "file-list pointer pair is incomplete at id {file_id}"
        );
        let iso_path = read_c_string(file, name_address)?.replace('\\', "/");
        if !iso_path.starts_with("/IMAGE_") {
            continue;
        }
        let (file_list_symbol, bytes) = read_symbol_by_address(file, list_address)?;
        let records = parse_file_records(&bytes)?;
        sources.push(FileSourceRecords {
            evidence: TextureFileSource {
                file_id,
                iso_path,
                file_list_symbol,
                file_records: records.len(),
            },
            records,
        });
    }
    ensure!(!sources.is_empty(), "ELF has no IMAGE file-list sources");
    Ok(sources)
}

fn read_symbol_by_address(file: &object::File<'_>, address: u64) -> Result<(String, Vec<u8>)> {
    let symbol = file
        .symbols()
        .find(|symbol| symbol.is_definition() && symbol.address() == address && symbol.size() > 0)
        .with_context(|| format!("ELF has no symbol at {address:#x}"))?;
    let name = symbol
        .name()
        .context("ELF address-matched symbol name is invalid")?
        .to_string();
    let (_, bytes) = read_symbol_bytes(file, &name)?;
    Ok((name, bytes))
}

fn read_c_string(file: &object::File<'_>, address: u64) -> Result<String> {
    let section = file
        .sections()
        .find(|section| {
            address >= section.address()
                && address < section.address().saturating_add(section.size())
        })
        .with_context(|| format!("ELF string address has no section: {address:#x}"))?;
    let offset = usize::try_from(address - section.address())?;
    let remainder = section
        .data()?
        .get(offset..)
        .with_context(|| format!("ELF string address exceeds section: {address:#x}"))?;
    let end = remainder
        .iter()
        .position(|byte| *byte == 0)
        .with_context(|| format!("ELF string is not terminated: {address:#x}"))?;
    Ok(std::str::from_utf8(&remainder[..end])
        .context("ELF file-list name is not UTF-8")?
        .to_string())
}

pub(super) fn find_table_references(
    file: &object::File<'_>,
    table_addresses: &BTreeMap<u32, String>,
) -> Result<BTreeMap<String, Vec<TextureTableReference>>> {
    let mut references = BTreeMap::<String, BTreeSet<TextureTableReference>>::new();
    for symbol in file.symbols() {
        if !symbol.is_definition() || symbol.size() == 0 {
            continue;
        }
        let Ok(owner_symbol) = symbol.name() else {
            continue;
        };
        let Some(section_index) = symbol.section_index() else {
            continue;
        };
        let section = file.section_by_index(section_index)?;
        let relative = usize::try_from(
            symbol
                .address()
                .checked_sub(section.address())
                .with_context(|| format!("ELF symbol precedes its section: {owner_symbol}"))?,
        )?;
        let byte_length = usize::try_from(symbol.size())?;
        let Some((_, section_file_size)) = section.file_range() else {
            continue;
        };
        if u64::try_from(relative)? + symbol.size() > section_file_size {
            continue;
        }
        let Some(end) = relative.checked_add(byte_length) else {
            continue;
        };
        let section_data = section.data()?;
        let Some(bytes) = section_data.get(relative..end) else {
            continue;
        };
        find_pointer_references(
            bytes,
            symbol.address(),
            owner_symbol,
            table_addresses,
            &mut references,
        )?;
        find_address_materializations(
            bytes,
            symbol.address(),
            owner_symbol,
            table_addresses,
            &mut references,
        )?;
    }
    Ok(references
        .into_iter()
        .map(|(symbol, references)| (symbol, references.into_iter().collect()))
        .collect())
}

pub(super) fn find_pointer_references(
    source: &[u8],
    start_address: u64,
    owner_symbol: &str,
    table_addresses: &BTreeMap<u32, String>,
    references: &mut BTreeMap<String, BTreeSet<TextureTableReference>>,
) -> Result<()> {
    for offset in (0..source.len().saturating_sub(3)).step_by(4) {
        let value = u32::from_le_bytes(source[offset..offset + 4].try_into()?);
        let Some(table_symbol) = table_addresses.get(&value) else {
            continue;
        };
        references
            .entry(table_symbol.clone())
            .or_default()
            .insert(TextureTableReference {
                owner_symbol: owner_symbol.to_string(),
                address: format!("{:#010x}", start_address + u64::try_from(offset)?),
                kind: TextureTableReferenceKind::Pointer,
            });
    }
    Ok(())
}

pub(super) fn find_address_materializations(
    source: &[u8],
    start_address: u64,
    owner_symbol: &str,
    table_addresses: &BTreeMap<u32, String>,
    references: &mut BTreeMap<String, BTreeSet<TextureTableReference>>,
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
            let raw_lower_source = (lower_word >> 21) & 0x1f;
            let raw_lower_destination = (lower_word >> 16) & 0x1f;
            if lower_word >> 26 != 0x09
                || raw_lower_source != upper_register
                || raw_lower_destination != upper_register
            {
                continue;
            }
            let raw_lower_immediate = lower_word as u16 as i16;
            let resolved = (u32::from(upper_immediate) << 16)
                .wrapping_add_signed(i32::from(raw_lower_immediate));
            let Some(table_symbol) = table_addresses.get(&resolved) else {
                continue;
            };
            let upper = validate_r5900_instruction(
                &source[upper_offset..upper_offset + 4],
                start_address + u64::try_from(upper_offset)?,
            )?;
            let lower = validate_r5900_instruction(
                &source[lower_offset..lower_offset + 4],
                start_address + u64::try_from(lower_offset)?,
            )?;
            let Operands::SignedImmediate {
                destination: lower_destination,
                source: lower_source,
                immediate: lower_immediate,
            } = lower.operands()
            else {
                continue;
            };
            let Operands::LoadUpper {
                destination,
                immediate,
            } = upper.operands()
            else {
                continue;
            };
            if upper.form() != InstructionForm::Lui
                || lower.form() != InstructionForm::Addiu
                || lower_destination != destination
                || lower_source != destination
            {
                continue;
            }
            ensure!(
                *immediate == upper_immediate && *lower_immediate == raw_lower_immediate,
                "typed address operands differ from their raw candidate"
            );
            references
                .entry(table_symbol.clone())
                .or_default()
                .insert(TextureTableReference {
                    owner_symbol: owner_symbol.to_string(),
                    address: format!("{:#010x}", start_address + u64::try_from(upper_offset)?),
                    kind: TextureTableReferenceKind::TypedAddressMaterialization,
                });
        }
    }
    Ok(())
}
