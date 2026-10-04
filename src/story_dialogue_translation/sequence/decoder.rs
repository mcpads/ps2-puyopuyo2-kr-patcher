use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use super::parser::MESSAGE_WINDOW_OPCODE;
use crate::cpx::{read_address_range, read_symbol_bytes};
use crate::typed_evidence::{
    require_memory, require_shift, require_signed_immediate, require_three_general,
    require_unsigned_immediate, symbol_address, validate_loaded_address,
};

const SEQUENCE_COMMAND_SYMBOL: &str = "SeqCommand";
const SEQUENCE_COMMAND_SIZE_SYMBOL: &str = "SeqComSize";
const MESSAGE_WINDOW_CONSUMER_SYMBOL: &str = "Seq_ActMsgWindow";
const COMMAND_DISPATCH_FIRST_OPCODE: usize = 3;
const COMMAND_DISPATCH_ENTRIES: usize = 53;
const COMMAND_DISPATCH_TABLE_ADDRESS: u64 = 0x004e_fd10;
const MESSAGE_WINDOW_CASE_ADDRESS: u64 = 0x0045_90b8;

pub(super) fn validate_sequence_command_decoder(file: &object::File<'_>) -> Result<()> {
    let (address, source) = read_symbol_bytes(file, SEQUENCE_COMMAND_SYMBOL)?;
    validate_loaded_address(
        &source,
        address,
        0x5c,
        0x64,
        GeneralRegister::A1,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        "sequence command dispatch table",
    )?;
    require_memory(
        &source,
        address,
        0x48,
        InstructionForm::Lw,
        GeneralRegister::V1,
        GeneralRegister::S1,
        556,
        "sequence command pointer",
    )?;
    require_memory(
        &source,
        address,
        0x4c,
        InstructionForm::Lhu,
        GeneralRegister::V0,
        GeneralRegister::V1,
        0,
        "sequence opcode",
    )?;
    require_signed_immediate(
        &source,
        address,
        0x50,
        InstructionForm::Addiu,
        GeneralRegister::A0,
        GeneralRegister::V0,
        -(COMMAND_DISPATCH_FIRST_OPCODE as i16),
        "sequence dispatch index",
    )?;
    require_signed_immediate(
        &source,
        address,
        0x54,
        InstructionForm::Sltiu,
        GeneralRegister::AT,
        GeneralRegister::A0,
        COMMAND_DISPATCH_ENTRIES as i16,
        "sequence dispatch range",
    )?;
    require_shift(
        &source,
        address,
        0x60,
        InstructionForm::Sll,
        GeneralRegister::A0,
        GeneralRegister::A0,
        2,
        "sequence dispatch pointer scale",
    )?;
    require_three_general(
        &source,
        address,
        0x68,
        InstructionForm::Addu,
        GeneralRegister::A0,
        GeneralRegister::A0,
        GeneralRegister::A1,
        "sequence dispatch pointer",
    )?;
    require_memory(
        &source,
        address,
        0x6c,
        InstructionForm::Lw,
        GeneralRegister::A0,
        GeneralRegister::A0,
        0,
        "sequence dispatch target",
    )?;

    let dispatch = read_address_range(
        file,
        COMMAND_DISPATCH_TABLE_ADDRESS,
        u64::try_from(COMMAND_DISPATCH_ENTRIES * 4)?,
    )?;
    let entry_offset = (MESSAGE_WINDOW_OPCODE - COMMAND_DISPATCH_FIRST_OPCODE) * 4;
    let case_address = u32::from_le_bytes(
        dispatch[entry_offset..entry_offset + 4]
            .try_into()
            .expect("fixed dispatch entry width"),
    );
    ensure!(
        u64::from(case_address) == MESSAGE_WINDOW_CASE_ADDRESS,
        "message-window opcode dispatch target changed"
    );
    validate_loaded_address(
        &source,
        address,
        0x1554,
        0x1560,
        GeneralRegister::A0,
        symbol_address(file, MESSAGE_WINDOW_CONSUMER_SYMBOL)?,
        "message-window actor callback",
    )?;

    validate_high_byte_field(
        &source,
        address,
        0x1564,
        0x1568,
        0x156c,
        0x1570,
        0x1574,
        2,
        264,
        "message-window frame texture index",
    )?;
    validate_low_byte_field(
        &source,
        address,
        0x1578,
        0x157c,
        0x1580,
        2,
        268,
        "message-window dialogue texture index",
    )?;

    validate_low_byte_field(
        &source,
        address,
        0x15b0,
        0x15b4,
        0x15b8,
        8,
        288,
        "message-window source row",
    )?;
    validate_high_byte_field(
        &source,
        address,
        0x15dc,
        0x15e0,
        0x15e4,
        0x15e8,
        0x15ec,
        12,
        208,
        "message-window width cells",
    )?;
    validate_low_byte_field(
        &source,
        address,
        0x15f0,
        0x15f4,
        0x15f8,
        12,
        212,
        "message-window height cells",
    )?;
    validate_loaded_address(
        &source,
        address,
        0x161c,
        0x1620,
        GeneralRegister::V0,
        symbol_address(file, SEQUENCE_COMMAND_SIZE_SYMBOL)?,
        "sequence command size table",
    )?;
    require_memory(
        &source,
        address,
        0x1624,
        InstructionForm::Lhu,
        GeneralRegister::V1,
        GeneralRegister::A0,
        0,
        "sequence command size opcode",
    )?;
    require_shift(
        &source,
        address,
        0x1628,
        InstructionForm::Sll,
        GeneralRegister::V1,
        GeneralRegister::V1,
        1,
        "sequence command size index",
    )?;
    require_memory(
        &source,
        address,
        0x1630,
        InstructionForm::Lh,
        GeneralRegister::V0,
        GeneralRegister::V0,
        0,
        "sequence command word length",
    )?;
    require_shift(
        &source,
        address,
        0x1634,
        InstructionForm::Sll,
        GeneralRegister::V0,
        GeneralRegister::V0,
        1,
        "sequence command byte length",
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_high_byte_field(
    source: &[u8],
    address: u64,
    pointer_offset: usize,
    load_offset: usize,
    shift_offset: usize,
    mask_offset: usize,
    store_offset: usize,
    command_displacement: i16,
    actor_displacement: i16,
    role: &str,
) -> Result<()> {
    require_memory(
        source,
        address,
        pointer_offset,
        InstructionForm::Lw,
        GeneralRegister::V1,
        GeneralRegister::S1,
        556,
        role,
    )?;
    require_memory(
        source,
        address,
        load_offset,
        InstructionForm::Lhu,
        GeneralRegister::V1,
        GeneralRegister::V1,
        command_displacement,
        role,
    )?;
    require_shift(
        source,
        address,
        shift_offset,
        InstructionForm::Sra,
        GeneralRegister::V1,
        GeneralRegister::V1,
        8,
        role,
    )?;
    require_unsigned_immediate(
        source,
        address,
        mask_offset,
        InstructionForm::Andi,
        GeneralRegister::V1,
        GeneralRegister::V1,
        255,
        role,
    )?;
    require_memory(
        source,
        address,
        store_offset,
        InstructionForm::Sw,
        GeneralRegister::V1,
        GeneralRegister::V0,
        actor_displacement,
        role,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_low_byte_field(
    source: &[u8],
    address: u64,
    pointer_offset: usize,
    load_offset: usize,
    store_offset: usize,
    command_displacement: i16,
    actor_displacement: i16,
    role: &str,
) -> Result<()> {
    require_memory(
        source,
        address,
        pointer_offset,
        InstructionForm::Lw,
        GeneralRegister::V1,
        GeneralRegister::S1,
        556,
        role,
    )?;
    require_memory(
        source,
        address,
        load_offset,
        InstructionForm::Lbu,
        GeneralRegister::V1,
        GeneralRegister::V1,
        command_displacement,
        role,
    )?;
    require_memory(
        source,
        address,
        store_offset,
        InstructionForm::Sw,
        GeneralRegister::V1,
        GeneralRegister::V0,
        actor_displacement,
        role,
    )?;
    Ok(())
}
