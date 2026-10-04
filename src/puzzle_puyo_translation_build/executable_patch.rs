use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::executable::read_executable_symbol;
use crate::executable_text_translation::{
    ExecutableTextGlyphAssignment, ExecutableTextTranslationBundle,
};
use crate::indexed_texture::sha256;

const FONT_TABLE_SYMBOL: &str = "FontList_Nazo";
const HALFWIDTH_FONT_TABLE_SYMBOL: &str = "FontList_Nazo_Han";
const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image2";
const IMAGE_FILE_RECORD_BYTES: usize = 20;

pub(super) struct EncodedPuzzlePuyoMessage<'a> {
    pub(super) asset_id: &'a str,
    pub(super) symbol: &'a str,
    pub(super) file_offset: usize,
    pub(super) storage_byte_length: usize,
    pub(super) source_bytes: Vec<u8>,
    pub(super) replacement: Vec<u8>,
}

pub(super) fn encode_puzzle_puyo_messages<'a>(
    executable: &[u8],
    bundle: &'a ExecutableTextTranslationBundle,
    assignments: &[ExecutableTextGlyphAssignment],
) -> Result<Vec<EncodedPuzzlePuyoMessage<'a>>> {
    let codes = assignment_codes(assignments)?;
    bundle
        .entries
        .iter()
        .filter(|entry| entry.source.consumer_family == "puzzle_puyo")
        .map(|entry| {
            let symbol = read_executable_symbol(executable, &entry.source.symbol)?;
            let source_bytes = decode_hex(&entry.source.source_hex)?;
            ensure!(
                symbol.file_offset == parse_hex_usize(&entry.source.source_file_offset)?
                    && symbol.address
                        == u64::try_from(parse_hex_usize(&entry.source.source_address)?)?
                    && symbol.bytes == source_bytes
                    && symbol.bytes.len() == entry.source.storage_byte_length
                    && sha256(&symbol.bytes) == entry.source.source_sha256,
                "puzzle-puyo source symbol differs from the validated translation entry: {}",
                entry.source.asset_id
            );
            // The all-clear condition has no dynamic number or color icon. Its
            // single leading source cell is cosmetic and need not be reserved.
            let text = if entry.source.asset_id.starts_with("nazo/question/")
                && entry.source.asset_id != "nazo/question/04"
            {
                preserve_question_indentation(&entry.source.source_text, &entry.translation.text)?
            } else {
                entry.translation.text.clone()
            };
            let replacement = encode_translation(&text, &codes)?;
            ensure!(
                replacement.len() <= symbol.bytes.len(),
                "puzzle-puyo translation exceeds fixed storage: {}",
                entry.source.asset_id
            );
            Ok(EncodedPuzzlePuyoMessage {
                asset_id: &entry.source.asset_id,
                symbol: &entry.source.symbol,
                file_offset: symbol.file_offset,
                storage_byte_length: symbol.bytes.len(),
                source_bytes,
                replacement,
            })
        })
        .collect()
}

fn preserve_question_indentation(source: &str, translation: &str) -> Result<String> {
    let source_lines: Vec<_> = source.split('\n').collect();
    let translated_lines: Vec<_> = translation.split('\n').collect();
    ensure!(
        source_lines.len() == translated_lines.len(),
        "puzzle condition row count changed"
    );
    // The condition renderer overlays numbers and puyo icons on these blank cells.
    // NazoMsgPrint_Act advances 32 pixels for a fullwidth cell and 16 for ASCII.
    Ok(source_lines
        .iter()
        .zip(translated_lines)
        .map(|(source, translated)| {
            // The fullwidth opening parenthesis occupies one more halfwidth
            // cell than its translated ASCII form, before the fixed color icons.
            let translated = if source.contains("（　　") {
                translated.replacen("(    ", "(     ", 1)
            } else {
                translated.to_string()
            };
            let cells: usize = source
                .chars()
                .take_while(|c| matches!(c, ' ' | '\u{3000}'))
                .map(|c| if c == '\u{3000}' { 2 } else { 1 })
                .sum();
            format!(
                "{}{}",
                " ".repeat(cells),
                translated.trim_start_matches([' ', '\u{3000}'])
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn assignment_codes(
    assignments: &[ExecutableTextGlyphAssignment],
) -> Result<BTreeMap<char, [u8; 2]>> {
    assignments
        .iter()
        .map(|assignment| {
            let character = assignment
                .character
                .chars()
                .next()
                .context("puzzle-puyo assignment has no character")?;
            let bytes = decode_hex(&assignment.code_hex)?;
            ensure!(bytes.len() == 2, "puzzle-puyo custom code is not two bytes");
            Ok((character, [bytes[0], bytes[1]]))
        })
        .collect()
}

fn encode_translation(text: &str, codes: &BTreeMap<char, [u8; 2]>) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    for character in text.chars() {
        if character == '\n' {
            output.push(b'\n');
        } else if character.is_ascii() {
            output.push(character as u8);
        } else {
            output.extend_from_slice(codes.get(&character).with_context(|| {
                format!("puzzle-puyo text has an unassigned glyph: {character}")
            })?);
        }
    }
    output.push(0);
    ensure!(
        !output[..output.len() - 1].contains(&0),
        "puzzle-puyo encoding contains an interior NUL"
    );
    Ok(output)
}

pub(super) fn patch_executable(
    source: &[u8],
    fullwidth_assignments: &[ExecutableTextGlyphAssignment],
    halfwidth_assignments: &[ExecutableTextGlyphAssignment],
    messages: &[EncodedPuzzlePuyoMessage<'_>],
    image_file_record: usize,
    expected_stored_length: u32,
    replacement_cpx_length: usize,
) -> Result<(Vec<u8>, usize)> {
    let font_table = read_executable_symbol(source, FONT_TABLE_SYMBOL)?;
    let halfwidth_font_table = read_executable_symbol(source, HALFWIDTH_FONT_TABLE_SYMBOL)?;
    let file_list = read_executable_symbol(source, IMAGE_FILE_LIST_SYMBOL)?;
    let stored_length_offset = file_list
        .file_offset
        .checked_add(image_file_record * IMAGE_FILE_RECORD_BYTES + 8)
        .context("puzzle-puyo file-record stored-length offset overflow")?;
    let replacement_stored_length = u32::try_from(
        replacement_cpx_length
            .checked_sub(1)
            .context("puzzle-puyo CPX has no terminator byte")?,
    )?
    .to_le_bytes();
    ensure!(
        source[stored_length_offset..stored_length_offset + 4]
            == expected_stored_length.to_le_bytes(),
        "puzzle-puyo source CPX stored length changed"
    );
    let mut plan = WritePlan::new()
        .region(ImageRegion {
            id: "puzzle-puyo-font-list".to_string(),
            range: font_table.file_offset..font_table.file_offset + font_table.bytes.len(),
            kind: RegionKind::Metadata,
            reason: "FontList_Nazo fullwidth lookup table".to_string(),
        })
        .region(ImageRegion {
            id: "puzzle-puyo-halfwidth-font-list".to_string(),
            range: halfwidth_font_table.file_offset
                ..halfwidth_font_table.file_offset + halfwidth_font_table.bytes.len(),
            kind: RegionKind::Metadata,
            reason: "FontList_Nazo_Han halfwidth lookup table".to_string(),
        })
        .region(ImageRegion {
            id: "puzzle-puyo-cpx-stored-length".to_string(),
            range: stored_length_offset..stored_length_offset + 4,
            kind: RegionKind::Metadata,
            reason: "FileList_Image2 G 113 stored-length field".to_string(),
        });
    for message in messages {
        plan = plan.region(ImageRegion {
            id: format!("puzzle-puyo-message-{}", sanitize_id(message.asset_id)),
            range: message.file_offset..message.file_offset + message.replacement.len(),
            kind: RegionKind::Data,
            reason: format!("translated puzzle-puyo message {}", message.asset_id),
        });
    }
    for assignment in fullwidth_assignments {
        let offset = font_table.file_offset + assignment.slot_index * 2;
        plan = plan.write(ExpectedWrite {
            id: format!("assign-puzzle-puyo-font-slot-{}", assignment.slot_index),
            owner: "puzzle-puyo-translation-encoding".to_string(),
            purpose: format!(
                "assign {} to its deterministic puzzle-puyo slot",
                assignment.character
            ),
            offset,
            expected_original: source[offset..offset + 2].to_vec(),
            replacement: decode_hex(&assignment.code_hex)?,
            intent: WriteIntent::Metadata,
        });
    }
    for assignment in halfwidth_assignments {
        let offset = halfwidth_font_table.file_offset + assignment.slot_index;
        ensure!(
            source[offset] == b' ',
            "puzzle-puyo halfwidth assignment does not replace a source space"
        );
        plan = plan.write(ExpectedWrite {
            id: format!(
                "assign-puzzle-puyo-halfwidth-font-slot-{}",
                assignment.slot_index
            ),
            owner: "puzzle-puyo-translation-encoding".to_string(),
            purpose: format!(
                "assign {} to a shadowed puzzle-puyo halfwidth space slot",
                assignment.character
            ),
            offset,
            expected_original: vec![b' '],
            replacement: decode_hex(&assignment.code_hex)?,
            intent: WriteIntent::Metadata,
        });
    }
    plan = plan.write(ExpectedWrite {
        id: "update-puzzle-puyo-font-cpx-stored-length".to_string(),
        owner: "puzzle-puyo-translation-compressor".to_string(),
        purpose: "make the source loader consume the complete puzzle-puyo font CPX".to_string(),
        offset: stored_length_offset,
        expected_original: expected_stored_length.to_le_bytes().to_vec(),
        replacement: replacement_stored_length.to_vec(),
        intent: WriteIntent::Metadata,
    });
    for message in messages {
        plan = plan.write(ExpectedWrite {
            id: format!(
                "replace-puzzle-puyo-message-{}",
                sanitize_id(message.asset_id)
            ),
            owner: "puzzle-puyo-translation-development".to_string(),
            purpose: "replace Japanese puzzle-puyo text with its direct Korean draft".to_string(),
            offset: message.file_offset,
            expected_original: message.source_bytes[..message.replacement.len()].to_vec(),
            replacement: message.replacement.clone(),
            intent: WriteIntent::Data,
        });
    }
    plan = super::menu_layout::add_writes(plan, source)?;
    let write_count = plan.writes.len();
    Ok((plan.apply(source, None)?, write_count))
}

pub(super) fn verify_patched_executable(
    source: &[u8],
    patched: &[u8],
    fullwidth_assignments: &[ExecutableTextGlyphAssignment],
    halfwidth_assignments: &[ExecutableTextGlyphAssignment],
    messages: &[EncodedPuzzlePuyoMessage<'_>],
) -> Result<()> {
    super::menu_layout::verify(source, patched)?;
    let table = read_executable_symbol(patched, FONT_TABLE_SYMBOL)?;
    ensure!(
        table.bytes.last() == Some(&0),
        "FontList_Nazo terminator changed"
    );
    for assignment in fullwidth_assignments {
        let code = decode_hex(&assignment.code_hex)?;
        let first_match = table.bytes[..table.bytes.len() - 1]
            .chunks_exact(2)
            .position(|entry| entry == code)
            .with_context(|| format!("puzzle-puyo code disappeared: {}", assignment.code_hex))?;
        ensure!(
            first_match == assignment.slot_index,
            "puzzle-puyo custom code first-match differs for {}",
            assignment.character
        );
    }
    let halfwidth_table = read_executable_symbol(patched, HALFWIDTH_FONT_TABLE_SYMBOL)?;
    ensure!(
        halfwidth_table.bytes.last() == Some(&0),
        "FontList_Nazo_Han terminator changed"
    );
    for assignment in halfwidth_assignments {
        let code = decode_hex(&assignment.code_hex)?;
        ensure!(
            code.len() == 1,
            "puzzle-puyo halfwidth custom code is not one byte"
        );
        let first_match = halfwidth_table.bytes[..halfwidth_table.bytes.len() - 1]
            .iter()
            .position(|entry| *entry == code[0])
            .with_context(|| {
                format!(
                    "puzzle-puyo halfwidth code disappeared: {}",
                    assignment.code_hex
                )
            })?;
        ensure!(
            first_match == assignment.slot_index,
            "puzzle-puyo halfwidth code first-match differs for {}",
            assignment.character
        );
    }
    for message in messages {
        let source_symbol = read_executable_symbol(source, message.symbol)?;
        let patched_symbol = read_executable_symbol(patched, message.symbol)?;
        ensure!(
            patched_symbol.bytes[..message.replacement.len()] == message.replacement
                && patched_symbol.bytes[message.replacement.len()..]
                    == source_symbol.bytes[message.replacement.len()..]
                && patched_symbol.bytes[message.replacement.len() - 1] == 0
                && patched_symbol.bytes.len() == message.storage_byte_length,
            "puzzle-puyo message readback differs: {}",
            message.asset_id
        );
    }
    Ok(())
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn parse_hex_usize(source: &str) -> Result<usize> {
    usize::from_str_radix(source.trim_start_matches("0x"), 16)
        .with_context(|| format!("invalid hexadecimal offset: {source}"))
}

pub(super) fn decode_hex(source: &str) -> Result<Vec<u8>> {
    ensure!(source.len().is_multiple_of(2), "hex string has odd length");
    (0..source.len())
        .step_by(2)
        .map(|offset| {
            u8::from_str_radix(&source[offset..offset + 2], 16)
                .with_context(|| format!("invalid hex byte at offset {offset}"))
        })
        .collect()
}

#[cfg(test)]
#[path = "executable_patch_tests.rs"]
mod executable_patch_tests;
