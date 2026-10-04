use anyhow::Result;
use r5900::{GeneralRegister, InstructionForm};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::typed_evidence::{
    require_shift, require_signed_immediate, require_unsigned_immediate, symbol_address,
    validate_jump_target, validate_loaded_address,
};

pub(super) fn validate_renderer_path(
    file: &object::File<'_>,
) -> Result<TutorialFontRendererEvidence> {
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
        "tutorial font registration record",
    )?;
    instructions.extend([
        require_signed_immediate(
            &renderer_bytes,
            renderer_address,
            0x6c,
            InstructionForm::Addiu,
            GeneralRegister::V1,
            GeneralRegister::ZERO,
            32,
            "tutorial glyph cell size",
        )?,
        validate_jump_target(
            &renderer_bytes,
            renderer_address,
            0xe0,
            lookup_address,
            "tutorial font lookup",
        )?,
        require_unsigned_immediate(
            &renderer_bytes,
            renderer_address,
            0x128,
            InstructionForm::Andi,
            GeneralRegister::V0,
            GeneralRegister::V0,
            0x0f,
            "tutorial atlas column",
        )?,
        require_shift(
            &renderer_bytes,
            renderer_address,
            0x12c,
            InstructionForm::Sll,
            GeneralRegister::V0,
            GeneralRegister::V0,
            5,
            "tutorial atlas x coordinate",
        )?,
        require_shift(
            &renderer_bytes,
            renderer_address,
            0x138,
            InstructionForm::Srl,
            GeneralRegister::V0,
            GeneralRegister::V0,
            4,
            "tutorial atlas row",
        )?,
        require_shift(
            &renderer_bytes,
            renderer_address,
            0x13c,
            InstructionForm::Sll,
            GeneralRegister::V0,
            GeneralRegister::V0,
            5,
            "tutorial atlas y coordinate",
        )?,
        validate_jump_target(
            &renderer_bytes,
            renderer_address,
            0x15c,
            sprite_packet_address,
            "tutorial sprite packet",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(TutorialFontRendererEvidence {
        symbol: RENDERER_SYMBOL.to_string(),
        sprite_packet_symbol: SPRITE_PACKET_SYMBOL.to_string(),
        registration_buffer_address: format!("{buffer_address:#010x}"),
        selected_record_address: format!("{selected_record_address:#010x}"),
        fullwidth_cell: "16 columns of 32x32 cells from y=0".to_string(),
        instructions,
    })
}
