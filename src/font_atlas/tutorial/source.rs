use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::executable::disassemble_symbol;
use crate::file_list_mapping::validate_file_id_mapping;
use crate::typed_evidence::{require_three_general, validate_jump_target, validate_loaded_address};

pub(super) fn validate_source_path(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<TutorialFontSourceEvidence> {
    let mapped_iso_path = validate_file_id_mapping(
        file,
        LOAD_FILE_ID,
        IMAGE_CONTAINER_ISO_PATH,
        FILE_LIST_SYMBOL,
        "tutorial texture",
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
        "tutorial texture load table",
    )?;
    instructions.extend([
        require_three_general(
            &loader_bytes,
            loader_address,
            0x0c,
            InstructionForm::Daddu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            GeneralRegister::ZERO,
            "tutorial image file id zero",
        )?,
        validate_jump_target(
            &loader_bytes,
            loader_address,
            0x10,
            read_file_address,
            "tutorial image file read",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));

    let expected_functions = [
        (LOADER_SYMBOL, 16_usize),
        (REGISTRATION_OWNER_SYMBOL, 147),
        (RENDERER_SYMBOL, 136),
        (LOOKUP_SYMBOL, 60),
    ];
    let functions = expected_functions
        .into_iter()
        .map(|(symbol, expected_instructions)| {
            let disassembly = disassemble_symbol(executable, symbol)?;
            ensure!(
                disassembly.instructions.len() == expected_instructions,
                "{symbol} instruction population changed"
            );
            Ok(TutorialTypedFunctionEvidence {
                symbol: symbol.to_string(),
                instruction_count: disassembly.instructions.len(),
            })
        })
        .collect::<Result<_>>()?;

    Ok(TutorialFontSourceEvidence {
        load_file_id: LOAD_FILE_ID,
        mapped_iso_path,
        file_list_symbol: FILE_LIST_SYMBOL.to_string(),
        load_table_symbol: LOAD_TABLE_SYMBOL.to_string(),
        loader_symbol: LOADER_SYMBOL.to_string(),
        read_file_symbol: READ_FILE_SYMBOL.to_string(),
        image_loads: 0,
        palette_loads: 0,
        auxiliary_loads: Vec::new(),
        instructions,
        functions,
    })
}
