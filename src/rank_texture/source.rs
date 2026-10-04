use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::executable::disassemble_symbol;
use crate::file_list_mapping::validate_file_id_mapping;
use crate::texture_registration::validate_registration_buffer_layout;
use crate::typed_evidence::{
    require_signed_immediate, validate_jump_target, validate_loaded_address,
};

type RankTextureSourceValidation = (
    RankTextureSourceEvidence,
    Vec<TypedInstructionEvidence>,
    Vec<TypedInstructionEvidence>,
    Vec<TypedInstructionEvidence>,
);

pub(super) fn validate_rank_texture_source(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<RankTextureSourceValidation> {
    let mapped_iso_path = validate_file_id_mapping(
        file,
        LOAD_FILE_ID,
        IMAGE_CONTAINER_ISO_PATH,
        FILE_LIST_SYMBOL,
        "ranking texture",
    )?;
    let (loader_address, loader_bytes) = read_symbol_bytes(file, LOADER_SYMBOL)?;
    let (load_table_address, _) = read_symbol_bytes(file, LOAD_TABLE_SYMBOL)?;
    let (read_file_address, _) = read_symbol_bytes(file, READ_FILE_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &loader_bytes,
        loader_address,
        0x28,
        0x34,
        GeneralRegister::A1,
        load_table_address,
        "ranking texture load table",
    )?;
    instructions.extend([
        require_signed_immediate(
            &loader_bytes,
            loader_address,
            0x2c,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            i16::try_from(LOAD_FILE_ID)?,
            "ranking image file id",
        )?,
        validate_jump_target(
            &loader_bytes,
            loader_address,
            0x30,
            read_file_address,
            "ranking image file read",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (owner_address, owner_bytes) = read_symbol_bytes(file, REGISTRATION_OWNER_SYMBOL)?;
    let (registration_set_address, _) = read_symbol_bytes(file, REGISTRATION_SET_SYMBOL)?;
    let (registrar_address, _) = read_symbol_bytes(file, REGISTRAR_SYMBOL)?;
    let mut call_instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        0xb8,
        0xc4,
        GeneralRegister::A1,
        load_table_address,
        "ranking registered load table",
    )?;
    call_instructions.extend(validate_loaded_address(
        &owner_bytes,
        owner_address,
        0xbc,
        0xcc,
        GeneralRegister::A2,
        registration_set_address,
        "ranking texture registration set",
    )?);
    call_instructions.extend([
        require_signed_immediate(
            &owner_bytes,
            owner_address,
            0xc0,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            i16::try_from(LOAD_FILE_ID)?,
            "ranking registration file id",
        )?,
        validate_jump_target(
            &owner_bytes,
            owner_address,
            0xc8,
            registrar_address,
            "ranking texture registration",
        )?,
    ]);
    call_instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let (pattern_address, _) = read_symbol_bytes(file, PATTERN_TABLE_SYMBOL)?;
    let (pattern_consumer_address, _) = read_symbol_bytes(file, PATTERN_CONSUMER_SYMBOL)?;
    let mut pattern_instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        0x148,
        0x15c,
        GeneralRegister::A3,
        pattern_address,
        "ranking display pattern",
    )?;
    pattern_instructions.push(validate_jump_target(
        &owner_bytes,
        owner_address,
        0x158,
        pattern_consumer_address,
        "ranking display pattern consumer",
    )?);
    pattern_instructions.sort_by(|left, right| left.address.cmp(&right.address));
    let (_, buffer_layout_instructions) = validate_registration_buffer_layout(file, executable)?;

    let expected_functions = [(LOADER_SYMBOL, 26_usize), (REGISTRATION_OWNER_SYMBOL, 467)];
    let functions = expected_functions
        .into_iter()
        .map(|(symbol, expected_instructions)| {
            let disassembly = disassemble_symbol(executable, symbol)?;
            ensure!(
                disassembly.instructions.len() == expected_instructions,
                "{symbol} instruction population changed"
            );
            Ok(RankTextureFunctionEvidence {
                symbol: symbol.to_string(),
                instruction_count: disassembly.instructions.len(),
            })
        })
        .collect::<Result<_>>()?;

    Ok((
        RankTextureSourceEvidence {
            load_file_id: LOAD_FILE_ID,
            mapped_iso_path,
            file_list_symbol: FILE_LIST_SYMBOL.to_string(),
            load_table_symbol: LOAD_TABLE_SYMBOL.to_string(),
            loader_symbol: LOADER_SYMBOL.to_string(),
            read_file_symbol: READ_FILE_SYMBOL.to_string(),
            instructions,
            functions,
        },
        call_instructions,
        pattern_instructions,
        buffer_layout_instructions,
    ))
}
