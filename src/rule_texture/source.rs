use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::executable::disassemble_symbol;
use crate::file_list_mapping::validate_file_id_mapping;
use crate::texture_registration::validate_pattern_buffer_layout;
use crate::typed_evidence::{require_three_general, validate_jump_target, validate_loaded_address};

pub(super) fn validate_rule_texture_source(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<RuleTextureSourceEvidence> {
    let mapped_iso_path = validate_file_id_mapping(
        file,
        LOAD_FILE_ID,
        IMAGE_CONTAINER_ISO_PATH,
        FILE_LIST_SYMBOL,
        "rule texture",
    )?;
    let (loader_address, loader_bytes) = read_symbol_bytes(file, LOADER_SYMBOL)?;
    let (load_table_address, _) = read_symbol_bytes(file, LOAD_TABLE_SYMBOL)?;
    let (read_file_address, _) = read_symbol_bytes(file, READ_FILE_SYMBOL)?;
    let mut load_instructions = validate_loaded_address(
        &loader_bytes,
        loader_address,
        0x04,
        0x14,
        GeneralRegister::A1,
        load_table_address,
        "rule texture load table",
    )?;
    load_instructions.extend([
        require_three_general(
            &loader_bytes,
            loader_address,
            0x0c,
            InstructionForm::Daddu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            GeneralRegister::ZERO,
            "rule image file id",
        )?,
        validate_jump_target(
            &loader_bytes,
            loader_address,
            0x10,
            read_file_address,
            "rule image file read",
        )?,
    ]);
    load_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (owner_address, owner_bytes) = read_symbol_bytes(file, OWNER_SYMBOL)?;
    let (registrar_address, _) = read_symbol_bytes(file, REGISTRAR_SYMBOL)?;
    let mut registration_instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        0x88,
        0x94,
        GeneralRegister::A1,
        load_table_address,
        "rule registered load table",
    )?;
    registration_instructions.extend([
        require_three_general(
            &owner_bytes,
            owner_address,
            0x8c,
            InstructionForm::Daddu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            GeneralRegister::ZERO,
            "rule registration file id",
        )?,
        validate_jump_target(
            &owner_bytes,
            owner_address,
            0x90,
            registrar_address,
            "rule texture registration",
        )?,
    ]);
    registration_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (select_pattern_address, _) = read_symbol_bytes(file, SELECT_PATTERN_SYMBOL)?;
    let (display_consumer_address, _) = read_symbol_bytes(file, DISPLAY_CONSUMER_SYMBOL)?;
    let (pattern_consumer_address, _) = read_symbol_bytes(file, PATTERN_CONSUMER_SYMBOL)?;
    let mut select_pattern_instructions = Vec::new();
    for (upper_offset, lower_offset, call_offset, register, pattern_offset, consumer) in [
        (
            0x98,
            0xac,
            0xb0,
            GeneralRegister::A3,
            0_u64,
            display_consumer_address,
        ),
        (
            0xb8,
            0xcc,
            0xc8,
            GeneralRegister::A3,
            24,
            display_consumer_address,
        ),
        (
            0xfc,
            0x100,
            0x104,
            GeneralRegister::A1,
            14 * 24,
            pattern_consumer_address,
        ),
        (
            0x208,
            0x20c,
            0x210,
            GeneralRegister::A1,
            8 * 24,
            pattern_consumer_address,
        ),
    ] {
        select_pattern_instructions.extend(validate_loaded_address(
            &owner_bytes,
            owner_address,
            upper_offset,
            lower_offset,
            register,
            select_pattern_address + pattern_offset,
            "rule direct selection pattern",
        )?);
        select_pattern_instructions.push(validate_jump_target(
            &owner_bytes,
            owner_address,
            call_offset,
            consumer,
            "rule direct selection pattern consumer",
        )?);
    }
    select_pattern_instructions.extend(validate_loaded_address(
        &owner_bytes,
        owner_address,
        0x180,
        0x188,
        GeneralRegister::V0,
        select_pattern_address,
        "rule dynamic selection pattern base",
    )?);
    select_pattern_instructions.push(validate_jump_target(
        &owner_bytes,
        owner_address,
        0x194,
        pattern_consumer_address,
        "rule dynamic selection pattern consumer",
    )?);
    select_pattern_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (character_pattern_address, _) = read_symbol_bytes(file, CHARACTER_PATTERN_SYMBOL)?;
    let (cell_consumer_address, _) = read_symbol_bytes(file, CELL_CONSUMER_SYMBOL)?;
    let mut character_pattern_instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        0x274,
        0x278,
        GeneralRegister::A1,
        character_pattern_address,
        "rule character pattern base",
    )?;
    character_pattern_instructions.push(validate_jump_target(
        &owner_bytes,
        owner_address,
        0x27c,
        cell_consumer_address,
        "rule character pattern consumer",
    )?);
    character_pattern_instructions.sort_by(|left, right| left.address.cmp(&right.address));
    let pattern_buffer_layout_instructions = validate_pattern_buffer_layout(file, executable)?;

    let expected_functions = [
        (LOADER_SYMBOL, 27_usize),
        (OWNER_SYMBOL, 384),
        (REGISTRAR_SYMBOL, 138),
        (DISPLAY_CONSUMER_SYMBOL, 43),
        (PATTERN_CONSUMER_SYMBOL, 28),
        (CELL_CONSUMER_SYMBOL, 6),
    ];
    let functions = expected_functions
        .into_iter()
        .map(|(symbol, expected_instructions)| {
            let disassembly = disassemble_symbol(executable, symbol)?;
            ensure!(
                disassembly.instructions.len() == expected_instructions,
                "{symbol} instruction population changed"
            );
            Ok(RuleTextureFunctionEvidence {
                symbol: symbol.to_string(),
                instruction_count: disassembly.instructions.len(),
            })
        })
        .collect::<Result<_>>()?;

    Ok(RuleTextureSourceEvidence {
        load_file_id: LOAD_FILE_ID,
        mapped_iso_path,
        file_list_symbol: FILE_LIST_SYMBOL.to_string(),
        load_table_symbol: LOAD_TABLE_SYMBOL.to_string(),
        loader_symbol: LOADER_SYMBOL.to_string(),
        owner_symbol: OWNER_SYMBOL.to_string(),
        read_file_symbol: READ_FILE_SYMBOL.to_string(),
        registrar_symbol: REGISTRAR_SYMBOL.to_string(),
        display_consumer_symbol: DISPLAY_CONSUMER_SYMBOL.to_string(),
        pattern_consumer_symbol: PATTERN_CONSUMER_SYMBOL.to_string(),
        cell_consumer_symbol: CELL_CONSUMER_SYMBOL.to_string(),
        load_instructions,
        registration_instructions,
        select_pattern_instructions,
        character_pattern_instructions,
        pattern_buffer_layout_instructions,
        functions,
    })
}
