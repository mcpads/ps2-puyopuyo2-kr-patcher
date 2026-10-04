use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use crate::cpx::read_symbol_bytes;
use crate::executable::disassemble_symbol;
use crate::typed_evidence::{
    TypedInstructionEvidence, require_memory, require_shift, require_three_general, symbol_address,
    validate_loaded_address,
};

pub(crate) const REGISTRAR_SYMBOL: &str = "ActRegList_Regist3";
pub(crate) const REGISTRATION_BUFFER_SYMBOL: &str = "RegBuff";
pub(crate) const PATTERN_SETTER_SYMBOL: &str = "Act_SetPtn";

pub(crate) fn validate_registration_buffer_layout(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<(u64, Vec<TypedInstructionEvidence>)> {
    let (registrar_address, registrar_bytes) = read_symbol_bytes(file, REGISTRAR_SYMBOL)?;
    let buffer_address = symbol_address(file, REGISTRATION_BUFFER_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &registrar_bytes,
        registrar_address,
        0x6c,
        0x70,
        GeneralRegister::A0,
        buffer_address,
        "texture registration buffer",
    )?;
    instructions.extend([
        require_shift(
            &registrar_bytes,
            registrar_address,
            0x74,
            InstructionForm::Sll,
            GeneralRegister::A1,
            GeneralRegister::A2,
            3,
            "registration index times eight",
        )?,
        require_three_general(
            &registrar_bytes,
            registrar_address,
            0x7c,
            InstructionForm::Addu,
            GeneralRegister::A1,
            GeneralRegister::A1,
            GeneralRegister::A2,
            "registration index times nine",
        )?,
        require_shift(
            &registrar_bytes,
            registrar_address,
            0x84,
            InstructionForm::Sll,
            GeneralRegister::A1,
            GeneralRegister::A1,
            4,
            "registration record byte stride",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));
    ensure!(
        disassemble_symbol(executable, REGISTRAR_SYMBOL)?
            .instructions
            .len()
            == 145,
        "ActRegList_Regist3 instruction population changed"
    );
    Ok((buffer_address, instructions))
}

pub(crate) fn validate_pattern_buffer_layout(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<Vec<TypedInstructionEvidence>> {
    let (setter_address, setter_bytes) = read_symbol_bytes(file, PATTERN_SETTER_SYMBOL)?;
    let buffer_address = symbol_address(file, REGISTRATION_BUFFER_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &setter_bytes,
        setter_address,
        0x08,
        0x0c,
        GeneralRegister::A2,
        buffer_address,
        "pattern registration buffer",
    )?;
    instructions.extend([
        require_memory(
            &setter_bytes,
            setter_address,
            0x18,
            InstructionForm::Lw,
            GeneralRegister::V1,
            GeneralRegister::A1,
            4,
            "pattern texture index load",
        )?,
        require_memory(
            &setter_bytes,
            setter_address,
            0x1c,
            InstructionForm::Sw,
            GeneralRegister::V1,
            GeneralRegister::A0,
            0x2c,
            "actor texture index store",
        )?,
        require_memory(
            &setter_bytes,
            setter_address,
            0x20,
            InstructionForm::Lw,
            GeneralRegister::T0,
            GeneralRegister::A0,
            0x2c,
            "actor texture index reload",
        )?,
        require_shift(
            &setter_bytes,
            setter_address,
            0x28,
            InstructionForm::Sll,
            GeneralRegister::A3,
            GeneralRegister::T0,
            3,
            "pattern texture index times eight",
        )?,
        require_three_general(
            &setter_bytes,
            setter_address,
            0x2c,
            InstructionForm::Addu,
            GeneralRegister::A3,
            GeneralRegister::A3,
            GeneralRegister::T0,
            "pattern texture index times nine",
        )?,
        require_shift(
            &setter_bytes,
            setter_address,
            0x30,
            InstructionForm::Sll,
            GeneralRegister::A3,
            GeneralRegister::A3,
            4,
            "pattern registration record byte stride",
        )?,
        require_three_general(
            &setter_bytes,
            setter_address,
            0x34,
            InstructionForm::Addu,
            GeneralRegister::A2,
            GeneralRegister::A2,
            GeneralRegister::A3,
            "pattern registration record address",
        )?,
        require_memory(
            &setter_bytes,
            setter_address,
            0x38,
            InstructionForm::Sw,
            GeneralRegister::A2,
            GeneralRegister::V1,
            0x38,
            "pattern registration record store",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));
    ensure!(
        disassemble_symbol(executable, PATTERN_SETTER_SYMBOL)?
            .instructions
            .len()
            == 28,
        "Act_SetPtn instruction population changed"
    );
    Ok(instructions)
}
