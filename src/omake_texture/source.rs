use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::executable::disassemble_symbol;
use crate::file_list_mapping::validate_file_id_mapping;
use crate::texture_registration::validate_pattern_buffer_layout;
use crate::typed_evidence::{require_three_general, validate_jump_target, validate_loaded_address};

pub(super) fn validate_omake_texture_source(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<OmakeTextureSourceEvidence> {
    let mapped_iso_path = validate_file_id_mapping(
        file,
        LOAD_FILE_ID,
        IMAGE_CONTAINER_ISO_PATH,
        FILE_LIST_SYMBOL,
        "Omake texture",
    )?;
    let (owner_address, owner_bytes) = read_symbol_bytes(file, OWNER_SYMBOL)?;
    let (load_table_address, _) = read_symbol_bytes(file, LOAD_TABLE_SYMBOL)?;
    let (read_file_address, _) = read_symbol_bytes(file, READ_FILE_SYMBOL)?;
    let mut load_instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        0x68,
        0x74,
        GeneralRegister::A1,
        load_table_address,
        "Omake texture load table",
    )?;
    load_instructions.extend([
        require_three_general(
            &owner_bytes,
            owner_address,
            0x6c,
            InstructionForm::Daddu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            GeneralRegister::ZERO,
            "Omake image file id",
        )?,
        validate_jump_target(
            &owner_bytes,
            owner_address,
            0x70,
            read_file_address,
            "Omake image file read",
        )?,
    ]);
    load_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (registrar_address, _) = read_symbol_bytes(file, REGISTRAR_SYMBOL)?;
    let mut registration_instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        0xb0,
        0xbc,
        GeneralRegister::A1,
        load_table_address,
        "Omake registered load table",
    )?;
    registration_instructions.extend([
        require_three_general(
            &owner_bytes,
            owner_address,
            0xb4,
            InstructionForm::Daddu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            GeneralRegister::ZERO,
            "Omake registration file id",
        )?,
        validate_jump_target(
            &owner_bytes,
            owner_address,
            0xb8,
            registrar_address,
            "Omake texture registration",
        )?,
    ]);
    registration_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (pattern_address, _) = read_symbol_bytes(file, PATTERN_TABLE_SYMBOL)?;
    let (pattern_owner_address, pattern_owner_bytes) =
        read_symbol_bytes(file, PATTERN_OWNER_SYMBOL)?;
    let (pattern_consumer_address, _) = read_symbol_bytes(file, PATTERN_CONSUMER_SYMBOL)?;
    let mut pattern_instructions = validate_loaded_address(
        &pattern_owner_bytes,
        pattern_owner_address,
        0x48,
        0x50,
        GeneralRegister::V0,
        pattern_address,
        "Omake dynamic pattern base",
    )?;
    pattern_instructions.push(validate_jump_target(
        &pattern_owner_bytes,
        pattern_owner_address,
        0x54,
        pattern_consumer_address,
        "Omake dynamic pattern consumer",
    )?);
    for (upper_offset, lower_offset, call_offset, pattern_offset) in [
        (0xe8, 0xec, 0xf0, 25_u64 * 24),
        (0x14c, 0x150, 0x154, 27_u64 * 24),
    ] {
        pattern_instructions.extend(validate_loaded_address(
            &pattern_owner_bytes,
            pattern_owner_address,
            upper_offset,
            lower_offset,
            GeneralRegister::A1,
            pattern_address + pattern_offset,
            "Omake direct pattern",
        )?);
        pattern_instructions.push(validate_jump_target(
            &pattern_owner_bytes,
            pattern_owner_address,
            call_offset,
            pattern_consumer_address,
            "Omake direct pattern consumer",
        )?);
    }
    pattern_instructions.sort_by(|left, right| left.address.cmp(&right.address));
    let pattern_buffer_layout_instructions = validate_pattern_buffer_layout(file, executable)?;

    let expected_functions = [
        (OWNER_SYMBOL, 224_usize),
        (PATTERN_OWNER_SYMBOL, 110),
        (REGISTRAR_SYMBOL, 138),
        (PATTERN_CONSUMER_SYMBOL, 28),
    ];
    let functions = expected_functions
        .into_iter()
        .map(|(symbol, expected_instructions)| {
            let disassembly = disassemble_symbol(executable, symbol)?;
            ensure!(
                disassembly.instructions.len() == expected_instructions,
                "{symbol} instruction population changed"
            );
            Ok(OmakeTextureFunctionEvidence {
                symbol: symbol.to_string(),
                instruction_count: disassembly.instructions.len(),
            })
        })
        .collect::<Result<_>>()?;

    Ok(OmakeTextureSourceEvidence {
        load_file_id: LOAD_FILE_ID,
        mapped_iso_path,
        file_list_symbol: FILE_LIST_SYMBOL.to_string(),
        load_table_symbol: LOAD_TABLE_SYMBOL.to_string(),
        owner_symbol: OWNER_SYMBOL.to_string(),
        read_file_symbol: READ_FILE_SYMBOL.to_string(),
        registrar_symbol: REGISTRAR_SYMBOL.to_string(),
        pattern_owner_symbol: PATTERN_OWNER_SYMBOL.to_string(),
        pattern_consumer_symbol: PATTERN_CONSUMER_SYMBOL.to_string(),
        load_instructions,
        registration_instructions,
        pattern_instructions,
        pattern_buffer_layout_instructions,
        functions,
    })
}
