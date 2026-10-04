use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use r5900::{GeneralRegister, InstructionForm};

use super::super::{FontGlyph, GlyphRectangle, decode_shift_jis, ensure_rectangle, hex};
use super::*;
use crate::cpx::read_symbol_bytes;
use crate::typed_evidence::{require_shift, require_signed_immediate, validate_loaded_address};

pub(super) fn validate_lookup_path(file: &object::File<'_>) -> Result<NazoFontLookupEvidence> {
    let (lookup_address, lookup_bytes) = read_symbol_bytes(file, LOOKUP_SYMBOL)?;
    let (fullwidth_address, fullwidth_bytes) = read_symbol_bytes(file, FULLWIDTH_LIST_SYMBOL)?;
    let (halfwidth_address, halfwidth_bytes) = read_symbol_bytes(file, HALFWIDTH_LIST_SYMBOL)?;
    let mut instructions = validate_loaded_address(
        &lookup_bytes,
        lookup_address,
        0xa0,
        0xa4,
        GeneralRegister::A2,
        fullwidth_address,
        "Nazo fullwidth lookup table",
    )?;
    instructions.extend(validate_loaded_address(
        &lookup_bytes,
        lookup_address,
        0x120,
        0x124,
        GeneralRegister::A2,
        halfwidth_address,
        "Nazo halfwidth lookup table",
    )?);
    instructions.extend([
        require_shift(
            &lookup_bytes,
            lookup_address,
            0xc0,
            InstructionForm::Srl,
            GeneralRegister::V1,
            GeneralRegister::T0,
            1,
            "Nazo fullwidth byte offset to slot index",
        )?,
        require_signed_immediate(
            &lookup_bytes,
            lookup_address,
            0xd0,
            InstructionForm::Addiu,
            GeneralRegister::A2,
            GeneralRegister::A2,
            2,
            "Nazo fullwidth lookup entry advance",
        )?,
        require_signed_immediate(
            &lookup_bytes,
            lookup_address,
            0xe0,
            InstructionForm::Addiu,
            GeneralRegister::V0,
            GeneralRegister::ZERO,
            0x50,
            "Nazo missing fullwidth fallback index",
        )?,
        require_signed_immediate(
            &lookup_bytes,
            lookup_address,
            0x140,
            InstructionForm::Addiu,
            GeneralRegister::A2,
            GeneralRegister::A2,
            1,
            "Nazo halfwidth lookup entry advance",
        )?,
    ]);
    instructions.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(NazoFontLookupEvidence {
        symbol: LOOKUP_SYMBOL.to_string(),
        fullwidth_list_symbol: FULLWIDTH_LIST_SYMBOL.to_string(),
        halfwidth_list_symbol: HALFWIDTH_LIST_SYMBOL.to_string(),
        fullwidth_slots: summarize_lookup_slots(&fullwidth_bytes, 2)?,
        halfwidth_slots: summarize_lookup_slots(&halfwidth_bytes, 1)?,
        missing_fullwidth_fallback_index: 0x50,
        instructions,
    })
}

pub(super) fn summarize_lookup_slots(
    source: &[u8],
    code_bytes: usize,
) -> Result<NazoLookupSlotSummary> {
    ensure!(
        source.last() == Some(&0),
        "font lookup table has no terminator"
    );
    let payload = &source[..source.len() - 1];
    ensure!(
        payload.len().is_multiple_of(code_bytes),
        "font lookup table has a partial code"
    );
    let mut slots_by_code = BTreeMap::<Vec<u8>, Vec<usize>>::new();
    for (slot, code) in payload.chunks_exact(code_bytes).enumerate() {
        slots_by_code.entry(code.to_vec()).or_default().push(slot);
    }
    let duplicates = slots_by_code
        .iter()
        .filter(|(_, slots)| slots.len() > 1)
        .map(|(code, slots)| NazoDuplicateCode {
            source_hex: hex(code),
            first_slot: slots[0],
            shadowed_slots: slots[1..].to_vec(),
        })
        .collect::<Vec<_>>();
    Ok(NazoLookupSlotSummary {
        slots: payload.len() / code_bytes,
        unique_codes: slots_by_code.len(),
        shadowed_duplicate_slots: duplicates
            .iter()
            .map(|duplicate| duplicate.shadowed_slots.len())
            .sum(),
        duplicates,
    })
}

pub(super) fn parse_nazo_fullwidth_glyphs(
    source: &[u8],
    atlas_width: u32,
    atlas_height: u32,
) -> Result<Vec<FontGlyph>> {
    ensure!(
        source.last() == Some(&0),
        "Nazo fullwidth font list has no final terminator"
    );
    ensure!(
        source[..source.len() - 1].len().is_multiple_of(2),
        "Nazo fullwidth font list has a partial Shift-JIS pair"
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

pub(super) fn parse_nazo_halfwidth_glyphs(
    source: &[u8],
    atlas_width: u32,
    atlas_height: u32,
) -> Result<Vec<FontGlyph>> {
    ensure!(
        source.last() == Some(&0),
        "Nazo halfwidth font list has no final terminator"
    );
    source[..source.len() - 1]
        .iter()
        .copied()
        .enumerate()
        .map(|(index, encoded)| {
            ensure!(
                encoded.is_ascii(),
                "Nazo halfwidth font list contains a non-ASCII byte"
            );
            let rectangle = GlyphRectangle {
                x: u32::try_from(index)? % HALFWIDTH_COLUMNS * 16,
                y: u32::try_from(index)? / HALFWIDTH_COLUMNS * 32 + HALFWIDTH_Y,
                width: 16,
                height: 32,
            };
            ensure_rectangle(rectangle, atlas_width, atlas_height)?;
            Ok(FontGlyph {
                index,
                source_hex: format!("{encoded:02x}"),
                character: char::from(encoded).to_string(),
                rectangle,
            })
        })
        .collect()
}
