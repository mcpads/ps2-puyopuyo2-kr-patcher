use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection};

use crate::cpx::{read_symbol_bytes, read_u32_at};
use crate::typed_evidence::symbol_address;

pub(crate) fn validate_file_id_mapping(
    file: &object::File<'_>,
    file_id: usize,
    expected_iso_path: &str,
    expected_file_list_symbol: &str,
    role: &str,
) -> Result<String> {
    let (_, name_table) = read_symbol_bytes(file, "FileListName")?;
    let (_, list_table) = read_symbol_bytes(file, "FileListList")?;
    ensure!(
        name_table.len() == list_table.len() && name_table.len().is_multiple_of(4),
        "file-list pointer arrays differ"
    );
    ensure!(
        file_id < name_table.len() / 4,
        "{role} file id is outside the file-list pointer arrays"
    );
    let name_pointer = u64::from(read_u32_at(&name_table, file_id * 4)?);
    let list_pointer = u64::from(read_u32_at(&list_table, file_id * 4)?);
    let mapped_iso_path = read_elf_string(file, name_pointer)?.replace('\\', "/");
    let file_list_address = symbol_address(file, expected_file_list_symbol)?;
    ensure!(
        mapped_iso_path == expected_iso_path && list_pointer == file_list_address,
        "{role} file id no longer maps to {expected_iso_path} and {expected_file_list_symbol}"
    );
    Ok(mapped_iso_path)
}

fn read_elf_string(file: &object::File<'_>, address: u64) -> Result<String> {
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
