use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::executable::disassemble_symbol;
use crate::file_list_mapping::validate_file_id_mapping;
use crate::texture_registration::validate_pattern_buffer_layout;
use crate::typed_evidence::{
    require_signed_immediate, validate_jump_target, validate_loaded_address,
};

pub(super) fn validate_continue_texture_source(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<ContinueTextureSourceEvidence> {
    let mapped_iso_path = validate_file_id_mapping(
        file,
        LOAD_FILE_ID,
        IMAGE_CONTAINER_ISO_PATH,
        FILE_LIST_SYMBOL,
        "continue texture",
    )?;
    let (loader_address, loader_bytes) = read_symbol_bytes(file, LOADER_SYMBOL)?;
    let (load_table_address, _) = read_symbol_bytes(file, LOAD_TABLE_SYMBOL)?;
    let (read_file_address, _) = read_symbol_bytes(file, READ_FILE_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &loader_bytes,
        loader_address,
        0x04,
        0x14,
        GeneralRegister::A1,
        load_table_address,
        "continue texture load table",
    )?;
    instructions.extend([
        require_signed_immediate(
            &loader_bytes,
            loader_address,
            0x0c,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            i16::try_from(LOAD_FILE_ID)?,
            "continue image file id",
        )?,
        validate_jump_target(
            &loader_bytes,
            loader_address,
            0x10,
            read_file_address,
            "continue image file read",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (registration_owner_address, registration_owner_bytes) =
        read_symbol_bytes(file, REGISTRATION_OWNER_SYMBOL)?;
    let (registrar_address, _) = read_symbol_bytes(file, REGISTRAR_SYMBOL)?;
    let mut registration_instructions = validate_loaded_address(
        &registration_owner_bytes,
        registration_owner_address,
        0x80,
        0x8c,
        GeneralRegister::A1,
        load_table_address,
        "continue registered load table",
    )?;
    registration_instructions.extend([
        require_signed_immediate(
            &registration_owner_bytes,
            registration_owner_address,
            0x84,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            i16::try_from(LOAD_FILE_ID)?,
            "continue registration file id",
        )?,
        validate_jump_target(
            &registration_owner_bytes,
            registration_owner_address,
            0x88,
            registrar_address,
            "continue texture registration",
        )?,
    ]);
    registration_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (pattern_consumer_address, _) = read_symbol_bytes(file, PATTERN_CONSUMER_SYMBOL)?;
    let (screen_pattern_address, _) = read_symbol_bytes(file, SCREEN_PATTERN_SYMBOL)?;
    let (screen_owner_address, screen_owner_bytes) =
        read_symbol_bytes(file, SCREEN_PATTERN_OWNER_SYMBOL)?;
    let mut screen_pattern_instructions = validate_loaded_address(
        &screen_owner_bytes,
        screen_owner_address,
        0x58,
        0x60,
        GeneralRegister::V0,
        screen_pattern_address,
        "continue screen pattern base",
    )?;
    screen_pattern_instructions.push(validate_jump_target(
        &screen_owner_bytes,
        screen_owner_address,
        0x64,
        pattern_consumer_address,
        "continue screen pattern consumer",
    )?);
    for (upper_offset, lower_offset, call_offset, pattern_offset) in [
        (0xe8, 0xec, 0xf0, 0x18_u64),
        (0x174, 0x178, 0x17c, 0x18),
        (0x204, 0x208, 0x20c, 0),
    ] {
        screen_pattern_instructions.extend(validate_loaded_address(
            &screen_owner_bytes,
            screen_owner_address,
            upper_offset,
            lower_offset,
            GeneralRegister::A1,
            screen_pattern_address + pattern_offset,
            "continue direct screen pattern",
        )?);
        screen_pattern_instructions.push(validate_jump_target(
            &screen_owner_bytes,
            screen_owner_address,
            call_offset,
            pattern_consumer_address,
            "continue direct screen pattern consumer",
        )?);
    }
    screen_pattern_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (number_pattern_address, _) = read_symbol_bytes(file, NUMBER_PATTERN_SYMBOL)?;
    let (number_owner_address, number_owner_bytes) =
        read_symbol_bytes(file, NUMBER_PATTERN_OWNER_SYMBOL)?;
    let mut number_pattern_instructions = validate_loaded_address(
        &number_owner_bytes,
        number_owner_address,
        0xac,
        0xb4,
        GeneralRegister::V0,
        number_pattern_address,
        "continue number pattern base",
    )?;
    number_pattern_instructions.push(validate_jump_target(
        &number_owner_bytes,
        number_owner_address,
        0xe4,
        pattern_consumer_address,
        "continue number pattern consumer",
    )?);
    number_pattern_instructions.sort_by(|left, right| left.address.cmp(&right.address));
    let pattern_buffer_layout_instructions = validate_pattern_buffer_layout(file, executable)?;

    let expected_functions = [
        (LOADER_SYMBOL, 18_usize),
        (REGISTRATION_OWNER_SYMBOL, 279),
        (SCREEN_PATTERN_OWNER_SYMBOL, 175),
        (NUMBER_PATTERN_OWNER_SYMBOL, 385),
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
            Ok(ContinueTextureFunctionEvidence {
                symbol: symbol.to_string(),
                instruction_count: disassembly.instructions.len(),
            })
        })
        .collect::<Result<_>>()?;

    Ok(ContinueTextureSourceEvidence {
        load_file_id: LOAD_FILE_ID,
        mapped_iso_path,
        file_list_symbol: FILE_LIST_SYMBOL.to_string(),
        load_table_symbol: LOAD_TABLE_SYMBOL.to_string(),
        loader_symbol: LOADER_SYMBOL.to_string(),
        read_file_symbol: READ_FILE_SYMBOL.to_string(),
        registration_owner_symbol: REGISTRATION_OWNER_SYMBOL.to_string(),
        registrar_symbol: REGISTRAR_SYMBOL.to_string(),
        pattern_consumer_symbol: PATTERN_CONSUMER_SYMBOL.to_string(),
        instructions,
        registration_instructions,
        screen_pattern_instructions,
        number_pattern_instructions,
        pattern_buffer_layout_instructions,
        functions,
    })
}
