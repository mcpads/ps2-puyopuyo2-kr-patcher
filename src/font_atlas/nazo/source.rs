use anyhow::Result;
use r5900::{GeneralRegister, InstructionForm};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::executable::disassemble_symbol;
use crate::file_list_mapping::validate_file_id_mapping;
use crate::typed_evidence::{
    require_signed_immediate, validate_jump_target, validate_loaded_address,
};

pub(super) fn validate_source_path(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<NazoFontSourceEvidence> {
    let mapped_iso_path = validate_file_id_mapping(
        file,
        LOAD_FILE_ID,
        IMAGE_CONTAINER_ISO_PATH,
        FILE_LIST_SYMBOL,
        "Nazo texture",
    )?;

    let (loader_address, loader_bytes) = read_symbol_bytes(file, LOADER_SYMBOL)?;
    let (load_table_address, _) = read_symbol_bytes(file, LOAD_TABLE_SYMBOL)?;
    let (read_file_address, _) = read_symbol_bytes(file, READ_FILE_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &loader_bytes,
        loader_address,
        0x10,
        0x1c,
        GeneralRegister::A1,
        load_table_address,
        "Nazo texture load table",
    )?;
    instructions.push(require_signed_immediate(
        &loader_bytes,
        loader_address,
        0x14,
        InstructionForm::Addiu,
        GeneralRegister::A0,
        GeneralRegister::ZERO,
        i16::try_from(LOAD_FILE_ID)?,
        "Nazo image file id",
    )?);
    instructions.push(validate_jump_target(
        &loader_bytes,
        loader_address,
        0x18,
        read_file_address,
        "Nazo image file read",
    )?);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let mut symbols = vec![LOADER_SYMBOL, REGISTRAR_SYMBOL, SPRITE_PACKET_SYMBOL];
    symbols.extend(REGISTRATION_OWNER_SYMBOLS);
    symbols.extend([LOOKUP_SYMBOL, RENDERER_SYMBOL]);
    let functions = symbols
        .into_iter()
        .map(|symbol| {
            let disassembly = disassemble_symbol(executable, symbol)?;
            Ok(TypedFunctionEvidence {
                symbol: symbol.to_string(),
                instruction_count: disassembly.instructions.len(),
            })
        })
        .collect::<Result<_>>()?;

    Ok(NazoFontSourceEvidence {
        load_file_id: LOAD_FILE_ID,
        mapped_iso_path,
        file_list_symbol: FILE_LIST_SYMBOL.to_string(),
        load_table_symbol: LOAD_TABLE_SYMBOL.to_string(),
        loader_symbol: LOADER_SYMBOL.to_string(),
        read_file_symbol: READ_FILE_SYMBOL.to_string(),
        instructions,
        functions,
    })
}
