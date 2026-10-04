use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};

const REGINFO_SIZE: usize = 24;
const GP_VALUE_OFFSET: usize = 20;

pub(crate) struct ElfGpIdentity {
    pub(crate) value: u32,
    pub(crate) file_offset: u64,
    pub(crate) source_sha256: String,
}

pub(crate) struct FileBackedTextCorpus {
    pub(crate) symbol_definitions: usize,
    pub(crate) symbols: Vec<FileBackedTextSymbol>,
}

pub(crate) struct FileBackedTextSymbol {
    pub(crate) address: u64,
    pub(crate) bytes: Vec<u8>,
    pub(crate) owners: BTreeSet<String>,
}

pub(crate) struct FileBackedTextWord {
    pub(crate) address: u64,
    pub(crate) bytes: [u8; 4],
    pub(crate) owners: BTreeSet<String>,
}

pub(crate) fn collect_file_backed_text(file: &object::File<'_>) -> Result<FileBackedTextCorpus> {
    let mut symbol_definitions = 0;
    let mut physical = BTreeMap::<(u64, usize), FileBackedTextSymbol>::new();
    for symbol in file.symbols() {
        if symbol.kind() != SymbolKind::Text {
            continue;
        }
        let Some((_, byte_length)) =
            crate::executable_string_census::symbol_file_range(file, &symbol)?
        else {
            continue;
        };
        let name = symbol.name().context("text symbol name is invalid")?;
        ensure!(
            symbol.address().is_multiple_of(4) && byte_length.is_multiple_of(4),
            "text symbol is not instruction aligned: {name}"
        );
        let section_index = symbol
            .section_index()
            .context("file-backed text symbol has no section")?;
        let section = file.section_by_index(section_index)?;
        let relative = usize::try_from(
            symbol
                .address()
                .checked_sub(section.address())
                .context("text symbol precedes its section")?,
        )?;
        let end = relative
            .checked_add(byte_length)
            .context("text symbol range overflow")?;
        let bytes = section
            .data()?
            .get(relative..end)
            .context("text symbol exceeds its section")?;
        symbol_definitions += 1;
        let entry = physical
            .entry((symbol.address(), byte_length))
            .or_insert_with(|| FileBackedTextSymbol {
                address: symbol.address(),
                bytes: bytes.to_vec(),
                owners: BTreeSet::new(),
            });
        ensure!(
            entry.bytes == bytes,
            "aliased text symbols disagree at {:#010x}",
            symbol.address()
        );
        entry.owners.insert(name.to_string());
    }
    Ok(FileBackedTextCorpus {
        symbol_definitions,
        symbols: physical.into_values().collect(),
    })
}

pub(crate) fn read_elf_gp_identity(file: &object::File<'_>) -> Result<ElfGpIdentity> {
    let section = file
        .section_by_name(".reginfo")
        .context("executable has no .reginfo section")?;
    let data = section.data()?;
    ensure!(data.len() == REGINFO_SIZE, ".reginfo size changed");
    let gp = u32::from_le_bytes(
        data[GP_VALUE_OFFSET..GP_VALUE_OFFSET + 4]
            .try_into()
            .expect("fixed GP field length"),
    );
    let (file_offset, file_size) = section.file_range().context(".reginfo has no file range")?;
    ensure!(
        file_size == u64::try_from(REGINFO_SIZE)?,
        ".reginfo file range changed"
    );
    Ok(ElfGpIdentity {
        value: gp,
        file_offset,
        source_sha256: crate::indexed_texture::sha256(data),
    })
}

impl FileBackedTextCorpus {
    pub(crate) fn unique_words(&self) -> Result<BTreeMap<u64, FileBackedTextWord>> {
        let mut words = BTreeMap::<u64, FileBackedTextWord>::new();
        for symbol in &self.symbols {
            for (index, chunk) in symbol.bytes.chunks_exact(4).enumerate() {
                let offset = index.checked_mul(4).context("text word offset overflow")?;
                let address = symbol.address + u64::try_from(offset)?;
                let bytes: [u8; 4] = chunk.try_into().expect("four-byte text word");
                let entry = words.entry(address).or_insert_with(|| FileBackedTextWord {
                    address,
                    bytes,
                    owners: BTreeSet::new(),
                });
                ensure!(
                    entry.bytes == bytes,
                    "overlapping text symbols disagree at {address:#010x}"
                );
                entry.owners.extend(symbol.owners.iter().cloned());
            }
        }
        Ok(words)
    }
}
