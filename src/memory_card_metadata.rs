use anyhow::{Context, Result, ensure};
use encoding_rs::SHIFT_JIS;
use object::{Architecture, BinaryFormat, Endianness, Object, ObjectSymbol};
use serde::Serialize;

use crate::executable::read_executable_symbol;
use crate::typed_evidence::TypedInstructionEvidence;

mod validation;

use validation::{SymbolStorageIdentity, validate_metadata_pipeline};

const TITLE_SYMBOL: &str = "@2081";
const ICON_STRUCTURE_SYMBOL: &str = "mc_IconSys";
const ICON_STRUCTURE_SIZE: usize = 0x3c4;
const TITLE_FIELD_OFFSET: usize = 0xc0;
const TITLE_FIELD_CODE_UNITS: usize = 34;

pub struct MemoryCardMetadataInputs<'a> {
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub executable: &'a [u8],
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardMetadataReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub population: MemoryCardMetadataPopulation,
    pub title: MemoryCardTitleEntry,
    pub icon_structure: MemoryCardIconStructure,
    pub producer: MemoryCardMetadataFunction,
    pub creation_queue: MemoryCardCreationQueue,
    pub dispatcher: MemoryCardDispatcher,
    pub path_builder: MemoryCardPathBuilder,
    pub format_reference: MemoryCardFormatReference,
    pub translation_scope: MemoryCardMetadataTranslationScope,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardMetadataPopulation {
    pub status: &'static str,
    pub japanese_title_entries: usize,
    pub unresolved_entries: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardTitleEntry {
    pub metadata_id: &'static str,
    pub symbol: &'static str,
    pub address: String,
    pub file_offset: String,
    pub storage_byte_length: usize,
    pub source_sha256: String,
    pub source_hex: String,
    pub display_text: String,
    pub japanese_characters: usize,
    pub protected_ascii_runs: usize,
    pub destination_field_offset: String,
    pub destination_capacity_bytes: usize,
    pub terminator: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardIconStructure {
    pub symbol: &'static str,
    pub address: String,
    pub byte_length: usize,
    pub header: &'static str,
    pub title_field_offset: String,
    pub title_field_code_units: usize,
    pub title_field_byte_length: usize,
    pub newline_offset: u16,
    pub transparency: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardMetadataFunction {
    pub symbol: &'static str,
    pub address: String,
    pub byte_length: usize,
    pub sha256: String,
    pub instruction_count: usize,
    pub invariants: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardCreationQueue {
    pub function: MemoryCardMetadataFunction,
    pub open_opcode: u8,
    pub path_selector: u8,
    pub write_opcode: u8,
    pub write_buffer_symbol: &'static str,
    pub write_byte_length: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardDispatcher {
    pub function: MemoryCardMetadataFunction,
    pub dispatch_table_symbol: &'static str,
    pub dispatch_table_address: String,
    pub dispatch_entries: usize,
    pub open_handler_address: String,
    pub write_handler_address: String,
    pub write_api_symbol: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardPathBuilder {
    pub function: MemoryCardMetadataFunction,
    pub directory: String,
    pub filename: String,
    pub resolved_path: String,
    pub open_api_symbol: &'static str,
    pub open_flags: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardFormatReference {
    pub repository: &'static str,
    pub commit: &'static str,
    pub path: &'static str,
    pub contract: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryCardMetadataTranslationScope {
    pub target: &'static str,
    pub preserve: &'static str,
    pub report_role: &'static str,
}

pub fn survey_memory_card_metadata(
    inputs: MemoryCardMetadataInputs<'_>,
) -> Result<MemoryCardMetadataReport> {
    ensure!(
        crate::indexed_texture::sha256(inputs.executable) == inputs.executable_sha256,
        "executable bytes differ from the verified inventory hash"
    );
    let file = object::File::parse(inputs.executable).context("failed to parse executable")?;
    ensure!(file.format() == BinaryFormat::Elf, "executable is not ELF");
    ensure!(
        file.architecture() == Architecture::Mips && file.endianness() == Endianness::Little,
        "memory-card metadata survey requires little-endian MIPS ELF"
    );
    let title_storage = read_executable_symbol(inputs.executable, TITLE_SYMBOL)?;
    let (display_text, japanese_characters, protected_ascii_runs) =
        decode_exact_title(&title_storage.bytes)?;
    ensure!(
        japanese_characters > 0,
        "memory-card title has no Japanese script"
    );
    ensure!(
        title_storage.bytes.len() <= TITLE_FIELD_CODE_UNITS * 2,
        "memory-card title exceeds the icon.sys title field"
    );
    let icon_structure = symbol_storage_identity(&file, ICON_STRUCTURE_SYMBOL)?;
    ensure!(
        icon_structure.byte_length == ICON_STRUCTURE_SIZE,
        "memory-card icon structure size changed"
    );
    let pipeline = validate_metadata_pipeline(inputs.executable, &file)?;
    ensure!(
        pipeline.icon_structure == icon_structure,
        "validated memory-card pipeline uses a different icon structure"
    );

    Ok(MemoryCardMetadataReport {
        schema: 1,
        source_bin_sha256: inputs.source_bin_sha256,
        executable_iso_path: inputs.executable_iso_path,
        executable_sha256: inputs.executable_sha256,
        population: MemoryCardMetadataPopulation {
            status: "complete for the Japanese title copied into the named mc_IconSys structure",
            japanese_title_entries: 1,
            unresolved_entries: 0,
        },
        title: MemoryCardTitleEntry {
            metadata_id: "memory-card/icon-system/title",
            symbol: TITLE_SYMBOL,
            address: format!("{:#010x}", title_storage.address),
            file_offset: format!("{:#010x}", title_storage.file_offset),
            storage_byte_length: title_storage.bytes.len(),
            source_sha256: crate::indexed_texture::sha256(&title_storage.bytes),
            source_hex: title_storage
                .bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            display_text,
            japanese_characters,
            protected_ascii_runs,
            destination_field_offset: format!("{TITLE_FIELD_OFFSET:#x}"),
            destination_capacity_bytes: TITLE_FIELD_CODE_UNITS * 2,
            terminator: "nul",
        },
        icon_structure: MemoryCardIconStructure {
            symbol: ICON_STRUCTURE_SYMBOL,
            address: format!("{:#010x}", icon_structure.address),
            byte_length: icon_structure.byte_length,
            header: "PS2D",
            title_field_offset: format!("{TITLE_FIELD_OFFSET:#x}"),
            title_field_code_units: TITLE_FIELD_CODE_UNITS,
            title_field_byte_length: TITLE_FIELD_CODE_UNITS * 2,
            newline_offset: 10,
            transparency: 0x60,
        },
        producer: pipeline.producer,
        creation_queue: MemoryCardCreationQueue {
            function: pipeline.creation_queue,
            open_opcode: 0x0a,
            path_selector: 1,
            write_opcode: 0x0f,
            write_buffer_symbol: ICON_STRUCTURE_SYMBOL,
            write_byte_length: ICON_STRUCTURE_SIZE,
        },
        dispatcher: MemoryCardDispatcher {
            function: pipeline.dispatcher,
            dispatch_table_symbol: "@1762",
            dispatch_table_address: format!("{:#010x}", pipeline.dispatch_table_address),
            dispatch_entries: pipeline.dispatch_entries,
            open_handler_address: format!("{:#010x}", pipeline.open_handler_address),
            write_handler_address: format!("{:#010x}", pipeline.write_handler_address),
            write_api_symbol: "sceMcWrite",
        },
        path_builder: MemoryCardPathBuilder {
            function: pipeline.path_builder,
            directory: pipeline.directory.clone(),
            filename: pipeline.filename.clone(),
            resolved_path: format!("{}/{}", pipeline.directory, pipeline.filename),
            open_api_symbol: "sceMcOpen",
            open_flags: "0x0203".to_string(),
        },
        format_reference: MemoryCardFormatReference {
            repository: "https://github.com/ps2dev/ps2sdk",
            commit: "f08e889fef8ab361f863c44ebe78212ced2839ca",
            path: "ee/rpc/memorycard/include/libmc.h",
            contract: "mcIcon is 0x3c4 bytes; its 68-byte unsigned-short title[34] storage begins at offset 0xc0, and the following icon filename field begins at 0x104",
        },
        translation_scope: MemoryCardMetadataTranslationScope {
            target: "Japanese bytes in the exact reversible Shift-JIS title only",
            preserve: "any ASCII bytes, PS2D header, layout fields, lighting, icon filenames, and every byte outside the title storage",
            report_role: "read-only static storage-to-icon.sys write evidence; not an approved translation asset",
        },
        evidence_boundary: "The report proves exact title storage, typed copy into mc_IconSys +0xc0, queued write of the complete 0x3c4-byte structure, /BISLPM-62400/icon.sys path construction, and dispatch to sceMcWrite. It does not prove that a save-creation branch executes, that the write succeeds on a real card, or that the PS2 browser displays the title.",
    })
}

fn symbol_storage_identity(file: &object::File<'_>, name: &str) -> Result<SymbolStorageIdentity> {
    let symbol = file
        .symbols()
        .find(|symbol| symbol.name().is_ok_and(|candidate| candidate == name))
        .with_context(|| format!("ELF symbol not found: {name}"))?;
    ensure!(symbol.is_definition(), "ELF symbol is not defined: {name}");
    Ok(SymbolStorageIdentity {
        address: symbol.address(),
        byte_length: usize::try_from(symbol.size())?,
    })
}

fn decode_exact_title(source: &[u8]) -> Result<(String, usize, usize)> {
    ensure!(
        source.len() >= 2 && source.last() == Some(&0) && !source[..source.len() - 1].contains(&0),
        "memory-card title is not one exact NUL-terminated string"
    );
    let payload = &source[..source.len() - 1];
    let decoded = SHIFT_JIS
        .decode_without_bom_handling_and_without_replacement(payload)
        .context("memory-card title is not valid Shift-JIS")?
        .into_owned();
    let (encoded, encoding, had_errors) = SHIFT_JIS.encode(&decoded);
    ensure!(
        !had_errors && encoding == SHIFT_JIS && encoded.as_ref() == payload,
        "memory-card title is not byte-reversible Shift-JIS"
    );
    let japanese_characters = decoded
        .chars()
        .filter(|character| is_japanese(*character))
        .count();
    let protected_ascii_runs = decoded
        .split(|character: char| !character.is_ascii())
        .filter(|run| !run.is_empty())
        .count();
    Ok((decoded, japanese_characters, protected_ascii_runs))
}

fn is_japanese(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
            | '\u{ff61}'..='\u{ff9f}'
    )
}

#[cfg(test)]
#[path = "memory_card_metadata/memory_card_metadata_tests.rs"]
mod memory_card_metadata_tests;
