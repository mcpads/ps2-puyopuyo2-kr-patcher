use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use super::*;
use crate::cpx::read_symbol_bytes;
use crate::indexed_texture::{TextureRegistrationPair, parse_texture_registration_pairs};
use crate::texture_registration::validate_registration_buffer_layout;
use crate::typed_evidence::{require_three_general, validate_jump_target, validate_loaded_address};

pub(super) fn validate_registration_path(
    file: &object::File<'_>,
    executable: &[u8],
) -> Result<TutorialFontRegistrationEvidence> {
    let (_, set_bytes) = read_symbol_bytes(file, REGISTRATION_SET_SYMBOL)?;
    let pairs = parse_texture_registration_pairs(REGISTRATION_SET_SYMBOL, &set_bytes)?;
    let expected_pairs = (0..REGISTRATION_PAIR_COUNT)
        .map(|index| TextureRegistrationPair {
            image_load_index: index,
            palette_slot: index,
        })
        .collect::<Vec<_>>();
    ensure!(
        pairs == expected_pairs,
        "tutorial texture registration set changed"
    );

    let (owner_address, owner_bytes) = read_symbol_bytes(file, REGISTRATION_OWNER_SYMBOL)?;
    let (load_table_address, _) = read_symbol_bytes(file, LOAD_TABLE_SYMBOL)?;
    let (set_address, _) = read_symbol_bytes(file, REGISTRATION_SET_SYMBOL)?;
    let (registrar_address, _) = read_symbol_bytes(file, REGISTRAR_SYMBOL)?;
    let mut call_instructions = validate_loaded_address(
        &owner_bytes,
        owner_address,
        0x80,
        0x8c,
        GeneralRegister::A1,
        load_table_address,
        "tutorial texture load table registration",
    )?;
    call_instructions.extend(validate_loaded_address(
        &owner_bytes,
        owner_address,
        0x84,
        0x94,
        GeneralRegister::A2,
        set_address,
        "tutorial texture registration set",
    )?);
    call_instructions.extend([
        require_three_general(
            &owner_bytes,
            owner_address,
            0x88,
            InstructionForm::Daddu,
            GeneralRegister::A0,
            GeneralRegister::ZERO,
            GeneralRegister::ZERO,
            "tutorial registration file id zero",
        )?,
        validate_jump_target(
            &owner_bytes,
            owner_address,
            0x90,
            registrar_address,
            "tutorial texture registration",
        )?,
    ]);
    call_instructions.sort_by(|left, right| left.address.cmp(&right.address));
    let (_, buffer_layout_instructions) = validate_registration_buffer_layout(file, executable)?;

    Ok(TutorialFontRegistrationEvidence {
        registrar_symbol: REGISTRAR_SYMBOL.to_string(),
        registration_owner_symbol: REGISTRATION_OWNER_SYMBOL.to_string(),
        registration_set_symbol: REGISTRATION_SET_SYMBOL.to_string(),
        registration_buffer_symbol: REGISTRATION_BUFFER_SYMBOL.to_string(),
        record_byte_length: REGISTRATION_RECORD_BYTES,
        font_registration_slot: FONT_REGISTRATION_SLOT,
        font_image_load_index: FONT_IMAGE_LOAD_INDEX,
        font_palette_slot: FONT_PALETTE_SLOT,
        pairs: pairs
            .into_iter()
            .map(|pair| TutorialTextureRegistrationPair {
                image_load_index: pair.image_load_index,
                palette_slot: pair.palette_slot,
            })
            .collect(),
        call_instructions,
        buffer_layout_instructions,
    })
}
