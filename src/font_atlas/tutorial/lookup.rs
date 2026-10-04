use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use super::super::{FontGlyph, GlyphRectangle, decode_shift_jis, ensure_rectangle, hex};
use super::*;
use crate::cpx::read_symbol_bytes;
use crate::typed_evidence::{
    require_memory, require_shift, require_signed_immediate, validate_loaded_address,
};

pub(super) fn validate_lookup_path(file: &object::File<'_>) -> Result<TutorialFontLookupEvidence> {
    let (lookup_address, lookup_bytes) = read_symbol_bytes(file, LOOKUP_SYMBOL)?;
    let (font_list_address, font_list_bytes) = read_symbol_bytes(file, FONT_LIST_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &lookup_bytes,
        lookup_address,
        0x98,
        0x9c,
        GeneralRegister::A3,
        font_list_address,
        "tutorial font lookup table",
    )?;
    instructions.extend([
        require_shift(
            &lookup_bytes,
            lookup_address,
            0xb8,
            InstructionForm::Srl,
            GeneralRegister::V1,
            GeneralRegister::T1,
            1,
            "tutorial font byte offset to slot index",
        )?,
        require_signed_immediate(
            &lookup_bytes,
            lookup_address,
            0xc8,
            InstructionForm::Addiu,
            GeneralRegister::A3,
            GeneralRegister::A3,
            2,
            "tutorial font lookup entry advance",
        )?,
        require_memory(
            &lookup_bytes,
            lookup_address,
            0xcc,
            InstructionForm::Lb,
            GeneralRegister::V1,
            GeneralRegister::A3,
            0,
            "tutorial font lookup terminator",
        )?,
        require_signed_immediate(
            &lookup_bytes,
            lookup_address,
            0xd4,
            InstructionForm::Addiu,
            GeneralRegister::T1,
            GeneralRegister::T1,
            2,
            "tutorial font lookup byte index",
        )?,
        require_signed_immediate(
            &lookup_bytes,
            lookup_address,
            0xd8,
            InstructionForm::Addiu,
            GeneralRegister::V1,
            GeneralRegister::ZERO,
            0x50,
            "tutorial missing glyph fallback index",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(TutorialFontLookupEvidence {
        symbol: LOOKUP_SYMBOL.to_string(),
        font_list_symbol: FONT_LIST_SYMBOL.to_string(),
        slots: summarize_tutorial_slots(&font_list_bytes)?,
        missing_fallback_index: 0x50,
        instructions,
    })
}

pub(super) fn summarize_tutorial_slots(source: &[u8]) -> Result<TutorialLookupSlotSummary> {
    ensure!(
        source.last() == Some(&0),
        "tutorial font list has no final terminator"
    );
    let payload = &source[..source.len() - 1];
    ensure!(
        payload.len().is_multiple_of(2),
        "tutorial font list has a partial Shift-JIS pair"
    );
    let mut slots_by_code = BTreeMap::<[u8; 2], Vec<usize>>::new();
    for (slot, code) in payload.chunks_exact(2).enumerate() {
        slots_by_code
            .entry([code[0], code[1]])
            .or_default()
            .push(slot);
    }
    let duplicates = slots_by_code
        .iter()
        .filter(|(_, slots)| slots.len() > 1)
        .map(|(code, slots)| TutorialDuplicateCode {
            source_hex: hex(code),
            first_slot: slots[0],
            shadowed_slots: slots[1..].to_vec(),
        })
        .collect::<Vec<_>>();
    Ok(TutorialLookupSlotSummary {
        slots: payload.len() / 2,
        unique_codes: slots_by_code.len(),
        shadowed_duplicate_slots: duplicates
            .iter()
            .map(|duplicate| duplicate.shadowed_slots.len())
            .sum(),
        duplicates,
    })
}

pub(super) fn parse_tutorial_glyphs(
    source: &[u8],
    atlas_width: u32,
    atlas_height: u32,
) -> Result<Vec<FontGlyph>> {
    ensure!(
        source.last() == Some(&0),
        "tutorial font list has no final terminator"
    );
    ensure!(
        source[..source.len() - 1].len().is_multiple_of(2),
        "tutorial font list has a partial Shift-JIS pair"
    );
    source[..source.len() - 1]
        .chunks_exact(2)
        .enumerate()
        .map(|(index, encoded)| {
            let rectangle = GlyphRectangle {
                x: u32::try_from(index)? % FULLWIDTH_COLUMNS * 32,
                y: u32::try_from(index)? / FULLWIDTH_COLUMNS * 32,
                width: 32,
                height: 32,
            };
            ensure_rectangle(rectangle, atlas_width, atlas_height)?;
            Ok(FontGlyph {
                index,
                source_hex: hex(encoded),
                character: decode_shift_jis(encoded)?,
                rectangle,
            })
        })
        .collect()
}
