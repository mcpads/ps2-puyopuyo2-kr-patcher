use anyhow::Result;
use r5900::{GeneralRegister, InstructionForm};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::typed_evidence::{
    require_shift, require_signed_immediate, require_three_general, require_unsigned_immediate,
    symbol_address, validate_jump_target, validate_loaded_address,
};

pub(super) fn validate_renderer_path(file: &object::File<'_>) -> Result<NazoFontRendererEvidence> {
    let (renderer_address, renderer_bytes) = read_symbol_bytes(file, RENDERER_SYMBOL)?;
    let (lookup_address, _) = read_symbol_bytes(file, LOOKUP_SYMBOL)?;
    let (sprite_packet_address, _) = read_symbol_bytes(file, SPRITE_PACKET_SYMBOL)?;
    let buffer_address = symbol_address(file, REGISTRATION_BUFFER_SYMBOL)?;
    let selected_record_address =
        buffer_address + u64::try_from(FONT_REGISTRATION_SLOT)? * REGISTRATION_RECORD_BYTES;
    let mut instructions = validate_loaded_address(
        &renderer_bytes,
        renderer_address,
        0x58,
        0x5c,
        GeneralRegister::V0,
        selected_record_address,
        "Nazo font registration record",
    )?;
    instructions.extend([
        validate_jump_target(
            &renderer_bytes,
            renderer_address,
            0xb0,
            lookup_address,
            "Nazo font lookup",
        )?,
        require_unsigned_immediate(
            &renderer_bytes,
            renderer_address,
            0x104,
            InstructionForm::Andi,
            GeneralRegister::V1,
            GeneralRegister::V1,
            0x0f,
            "Nazo fullwidth atlas column",
        )?,
        require_shift(
            &renderer_bytes,
            renderer_address,
            0x108,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            5,
            "Nazo fullwidth x coordinate",
        )?,
        require_shift(
            &renderer_bytes,
            renderer_address,
            0x114,
            InstructionForm::Srl,
            GeneralRegister::V1,
            GeneralRegister::V1,
            4,
            "Nazo fullwidth atlas row",
        )?,
        require_shift(
            &renderer_bytes,
            renderer_address,
            0x118,
            InstructionForm::Sll,
            GeneralRegister::V1,
            GeneralRegister::V1,
            5,
            "Nazo fullwidth y coordinate",
        )?,
        require_unsigned_immediate(
            &renderer_bytes,
            renderer_address,
            0x184,
            InstructionForm::Andi,
            GeneralRegister::A1,
            GeneralRegister::A1,
            0x1f,
            "Nazo halfwidth atlas column",
        )?,
        require_shift(
            &renderer_bytes,
            renderer_address,
            0x188,
            InstructionForm::Sll,
            GeneralRegister::A1,
            GeneralRegister::A1,
            4,
            "Nazo halfwidth x coordinate",
        )?,
        require_three_general(
            &renderer_bytes,
            renderer_address,
            0x194,
            InstructionForm::And,
            GeneralRegister::A0,
            GeneralRegister::A1,
            GeneralRegister::A0,
            "Nazo halfwidth atlas row",
        )?,
        require_signed_immediate(
            &renderer_bytes,
            renderer_address,
            0x198,
            InstructionForm::Addiu,
            GeneralRegister::A0,
            GeneralRegister::A0,
            i16::try_from(HALFWIDTH_Y)?,
            "Nazo halfwidth y base",
        )?,
        validate_jump_target(
            &renderer_bytes,
            renderer_address,
            0x1b0,
            sprite_packet_address,
            "Nazo sprite packet",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(NazoFontRendererEvidence {
        symbol: RENDERER_SYMBOL.to_string(),
        sprite_packet_symbol: SPRITE_PACKET_SYMBOL.to_string(),
        registration_buffer_address: format!("{buffer_address:#010x}"),
        selected_record_address: format!("{selected_record_address:#010x}"),
        fullwidth_cell: "16 columns of 32x32 cells from y=0".to_string(),
        halfwidth_cell: "32 columns of 16x32 cells from y=224".to_string(),
        instructions,
    })
}
