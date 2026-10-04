use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection, ObjectSymbol};
use r5900::{GeneralRegister, InstructionForm};

use super::{MovieConsumerEvidence, MovieTableEntry};
use crate::cpx::{read_symbol_bytes, read_u32_at};
use crate::typed_evidence::{
    require_memory, require_shift, require_three_general, symbol_address, validate_jump_target,
    validate_loaded_address,
};

const TABLE_SYMBOL: &str = "MovieFile_Tbl";
const OWNER_SYMBOL: &str = "MovieVideo_Run";
const PLAYER_SYMBOL: &str = "Movie_Play";

pub(super) fn survey_movie_consumer(
    executable: &[u8],
    expected_paths: &BTreeSet<String>,
) -> Result<MovieConsumerEvidence> {
    let file = object::File::parse(executable).context("failed to parse movie executable")?;
    let (table_address, table_bytes) = read_symbol_bytes(&file, TABLE_SYMBOL)?;
    ensure!(
        table_bytes.len().is_multiple_of(4),
        "movie file table is not pointer-aligned"
    );
    ensure!(
        table_bytes.len() / 4 == expected_paths.len(),
        "movie file table does not match the ISO movie population"
    );
    let mut entries = Vec::with_capacity(expected_paths.len());
    for index in 0..table_bytes.len() / 4 {
        let address = u64::from(read_u32_at(&table_bytes, index * 4)?);
        let expected_symbol = format!("MovieFile_{index:02}");
        let actual_symbol = file
            .symbols()
            .find(|symbol| symbol.is_definition() && symbol.address() == address)
            .with_context(|| format!("movie path pointer has no ELF symbol: {address:#x}"))?;
        ensure!(
            actual_symbol.name()? == expected_symbol,
            "movie path symbol order changed at index {index}"
        );
        let raw_path = read_elf_string(&file, address)?;
        let iso_path = normalize_cdrom_path(&raw_path)?;
        ensure!(
            expected_paths.contains(&iso_path),
            "movie table path is outside the ISO movie population: {iso_path}"
        );
        entries.push(MovieTableEntry {
            index,
            path_symbol: expected_symbol,
            path_address: format!("{address:#010x}"),
            iso_path,
        });
    }
    ensure!(
        entries
            .iter()
            .map(|entry| entry.iso_path.clone())
            .collect::<BTreeSet<_>>()
            == *expected_paths,
        "movie table does not exactly cover the ISO movie population"
    );

    let (owner_address, owner_bytes) = read_symbol_bytes(&file, OWNER_SYMBOL)?;
    let player_address = symbol_address(&file, PLAYER_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        0x18,
        0x1c,
        GeneralRegister::V0,
        table_address,
        "movie file table",
    )?;
    instructions.extend([
        require_shift(
            &owner_bytes,
            owner_address,
            0x20,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            2,
            "movie table index scaling",
        )?,
        require_three_general(
            &owner_bytes,
            owner_address,
            0x24,
            InstructionForm::Addu,
            GeneralRegister::V0,
            GeneralRegister::V0,
            GeneralRegister::V1,
            "movie table entry address",
        )?,
        validate_jump_target(
            &owner_bytes,
            owner_address,
            0x28,
            player_address,
            "movie player",
        )?,
        require_memory(
            &owner_bytes,
            owner_address,
            0x2c,
            InstructionForm::Lw,
            GeneralRegister::A0,
            GeneralRegister::V0,
            0,
            "movie path load",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));

    Ok(MovieConsumerEvidence {
        table_symbol: TABLE_SYMBOL.to_string(),
        table_address: format!("{table_address:#010x}"),
        entries,
        owner_symbol: OWNER_SYMBOL.to_string(),
        player_symbol: PLAYER_SYMBOL.to_string(),
        instructions,
    })
}

fn read_elf_string(file: &object::File<'_>, address: u64) -> Result<String> {
    let section = file
        .sections()
        .find(|section| {
            address >= section.address()
                && address < section.address().saturating_add(section.size())
        })
        .with_context(|| format!("movie string address has no section: {address:#x}"))?;
    let offset = usize::try_from(address - section.address())?;
    let remainder = section
        .data()?
        .get(offset..)
        .with_context(|| format!("movie string address exceeds section: {address:#x}"))?;
    let end = remainder
        .iter()
        .position(|byte| *byte == 0)
        .with_context(|| format!("movie string is not terminated: {address:#x}"))?;
    Ok(std::str::from_utf8(&remainder[..end])
        .context("movie path is not UTF-8")?
        .to_string())
}

fn normalize_cdrom_path(path: &str) -> Result<String> {
    let relative = path
        .strip_prefix("cdrom0:\\")
        .with_context(|| format!("movie path has an unexpected device prefix: {path}"))?;
    Ok(format!("/{}", relative.replace('\\', "/")))
}
