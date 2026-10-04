use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::executable::read_executable_symbol;
use crate::executable_text_translation::{
    ExecutableTextGlyphAssignment, ExecutableTextTranslationBundle,
};
use crate::indexed_texture::sha256;
use crate::message_corpus::TutorialMessageCorpusReport;

const FONT_TABLE_SYMBOL: &str = "FontList_Exp";
const IMAGE_FILE_LIST_SYMBOL: &str = "FileList_Image";
const IMAGE_FILE_RECORD_BYTES: usize = 20;
const MESSAGE_BANK_START_SYMBOL: &str = "ExpMsg_Data_0";
const MESSAGE_POINTER_TABLE_SYMBOL: &str = "ExpMsg_Data";

pub(super) struct EncodedTutorialMessage<'a> {
    pub(super) asset_id: &'a str,
    pub(super) packed_address: u64,
    pub(super) packed_file_offset: usize,
    pub(super) replacement: Vec<u8>,
}

pub(super) struct EncodedTutorialMessageBank<'a> {
    pub(super) address: u64,
    pub(super) file_offset: usize,
    pub(super) byte_length: usize,
    pub(super) source_bytes: Vec<u8>,
    pub(super) replacement: Vec<u8>,
    pub(super) used_byte_length: usize,
    pub(super) pointer_table_address: u64,
    pub(super) pointer_table_file_offset: usize,
    pub(super) pointer_table_source_bytes: Vec<u8>,
    pub(super) pointer_table_replacement: Vec<u8>,
    pub(super) messages: Vec<EncodedTutorialMessage<'a>>,
}

pub(super) fn encode_tutorial_messages<'a>(
    executable: &[u8],
    bundle: &'a ExecutableTextTranslationBundle,
    assignments: &[ExecutableTextGlyphAssignment],
    corpus: &TutorialMessageCorpusReport,
) -> Result<EncodedTutorialMessageBank<'a>> {
    let codes = assignment_codes(assignments)?;
    let entries = bundle
        .entries
        .iter()
        .filter(|entry| entry.source.consumer_family == "tutorial")
        .collect::<Vec<_>>();
    ensure!(
        entries.len() == corpus.message_bank.message_count
            && corpus.message_table.pointer_count == entries.len(),
        "tutorial translation population differs from the verified message bank"
    );

    let bank_start = read_executable_symbol(executable, MESSAGE_BANK_START_SYMBOL)?;
    let pointer_table = read_executable_symbol(executable, MESSAGE_POINTER_TABLE_SYMBOL)?;
    let byte_length = pointer_table
        .file_offset
        .checked_sub(bank_start.file_offset)
        .context("tutorial pointer table precedes the message bank")?;
    ensure!(
        pointer_table.address.checked_sub(bank_start.address) == Some(u64::try_from(byte_length)?)
            && bank_start.address == parse_hex_u64(&corpus.message_bank.address)?
            && bank_start.file_offset == parse_hex_usize(&corpus.message_bank.file_offset)?
            && byte_length == corpus.message_bank.byte_length
            && sha256(&executable[bank_start.file_offset..pointer_table.file_offset])
                == corpus.message_bank.source_sha256
            && pointer_table.address == parse_hex_u64(&corpus.message_table.address)?
            && pointer_table.file_offset == parse_hex_usize(&corpus.message_table.file_offset)?
            && pointer_table.bytes.len() == corpus.message_table.byte_length
            && sha256(&pointer_table.bytes) == corpus.message_table.sha256,
        "tutorial message bank or pointer-table evidence differs from the exact source"
    );
    ensure!(
        pointer_table.bytes.len() == entries.len() * 4,
        "tutorial pointer-table length changed"
    );

    let source_bytes = executable[bank_start.file_offset..pointer_table.file_offset].to_vec();
    let mut declared_source_bytes = vec![false; byte_length];
    let mut replacement = Vec::new();
    let mut pointer_table_replacement = Vec::with_capacity(pointer_table.bytes.len());
    let mut messages = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        ensure!(
            !entry.translation.text.is_empty(),
            "tutorial build requires a translation for {}",
            entry.source.asset_id
        );
        let symbol = read_executable_symbol(executable, &entry.source.symbol)?;
        let declared_source = decode_hex(&entry.source.source_hex)?;
        ensure!(
            symbol.file_offset == parse_hex_usize(&entry.source.source_file_offset)?
                && symbol.address == parse_hex_u64(&entry.source.source_address)?
                && symbol.bytes == declared_source
                && symbol.bytes.len() == entry.source.storage_byte_length
                && sha256(&symbol.bytes) == entry.source.source_sha256,
            "tutorial source symbol differs from the validated translation entry: {}",
            entry.source.asset_id
        );
        let source_relative = symbol
            .file_offset
            .checked_sub(bank_start.file_offset)
            .with_context(|| {
                format!(
                    "tutorial source symbol precedes the shared bank: {}",
                    entry.source.asset_id
                )
            })?;
        ensure!(
            source_relative + symbol.bytes.len() <= byte_length,
            "tutorial source symbol exceeds the shared bank: {}",
            entry.source.asset_id
        );
        for covered in
            &mut declared_source_bytes[source_relative..source_relative + symbol.bytes.len()]
        {
            ensure!(!*covered, "tutorial source symbols overlap");
            *covered = true;
        }
        let original_pointer = u32::from_le_bytes(
            pointer_table.bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("fixed tutorial pointer width"),
        );
        ensure!(
            u64::from(original_pointer) == symbol.address,
            "tutorial source pointer differs for {}",
            entry.source.asset_id
        );

        let encoded = encode_translation(&entry.translation.text, &codes)?;
        let packed_file_offset = bank_start.file_offset + replacement.len();
        let packed_address = bank_start.address + u64::try_from(replacement.len())?;
        replacement.extend_from_slice(&encoded);
        pointer_table_replacement.extend_from_slice(&u32::try_from(packed_address)?.to_le_bytes());
        messages.push(EncodedTutorialMessage {
            asset_id: &entry.source.asset_id,
            packed_address,
            packed_file_offset,
            replacement: encoded,
        });
    }
    ensure!(
        declared_source_bytes
            .iter()
            .enumerate()
            .all(|(offset, covered)| *covered || source_bytes[offset] == 0),
        "tutorial source message-bank padding is not zero"
    );
    ensure!(
        replacement.len() <= byte_length,
        "tutorial translations exceed the shared message bank by {} bytes",
        replacement.len() - byte_length
    );
    let used_byte_length = replacement.len();
    replacement.resize(byte_length, 0);
    Ok(EncodedTutorialMessageBank {
        address: bank_start.address,
        file_offset: bank_start.file_offset,
        byte_length,
        source_bytes,
        replacement,
        used_byte_length,
        pointer_table_address: pointer_table.address,
        pointer_table_file_offset: pointer_table.file_offset,
        pointer_table_source_bytes: pointer_table.bytes,
        pointer_table_replacement,
        messages,
    })
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
                .context("tutorial assignment has no character")?;
            let bytes = decode_hex(&assignment.code_hex)?;
            ensure!(bytes.len() == 2, "tutorial custom code is not two bytes");
            Ok((character, [bytes[0], bytes[1]]))
        })
        .collect()
}

fn encode_translation(text: &str, codes: &BTreeMap<char, [u8; 2]>) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    for character in text.chars() {
        if character == '\n' {
            output.push(b'\n');
        } else if character == ' ' {
            output.extend_from_slice(&[0x81, 0x40]);
        } else if character.is_ascii() {
            anyhow::bail!("tutorial text character has no two-byte renderer encoding: {character}");
        } else {
            output.extend_from_slice(
                codes.get(&character).with_context(|| {
                    format!("tutorial text has an unassigned glyph: {character}")
                })?,
            );
        }
    }
    output.push(0);
    ensure!(
        !output[..output.len() - 1].contains(&0),
        "tutorial encoding contains an interior NUL"
    );
    Ok(output)
}

pub(super) fn patch_executable(
    source: &[u8],
    assignments: &[ExecutableTextGlyphAssignment],
    message_bank: &EncodedTutorialMessageBank<'_>,
    image_file_record: usize,
    expected_stored_length: u32,
    replacement_cpx_length: usize,
) -> Result<(Vec<u8>, usize)> {
    let font_table = read_executable_symbol(source, FONT_TABLE_SYMBOL)?;
    let file_list = read_executable_symbol(source, IMAGE_FILE_LIST_SYMBOL)?;
    let stored_length_offset = file_list
        .file_offset
        .checked_add(image_file_record * IMAGE_FILE_RECORD_BYTES + 8)
        .context("tutorial file-record stored-length offset overflow")?;
    let replacement_stored_length = u32::try_from(
        replacement_cpx_length
            .checked_sub(1)
            .context("tutorial CPX has no terminator byte")?,
    )?
    .to_le_bytes();
    ensure!(
        source[stored_length_offset..stored_length_offset + 4]
            == expected_stored_length.to_le_bytes(),
        "tutorial source CPX stored length changed"
    );
    let mut plan = WritePlan::new()
        .region(ImageRegion {
            id: "tutorial-font-list".to_string(),
            range: font_table.file_offset..font_table.file_offset + font_table.bytes.len(),
            kind: RegionKind::Metadata,
            reason: "FontList_Exp fullwidth lookup table".to_string(),
        })
        .region(ImageRegion {
            id: "tutorial-cpx-stored-length".to_string(),
            range: stored_length_offset..stored_length_offset + 4,
            kind: RegionKind::Metadata,
            reason: "FileList_Image D 22 stored-length field".to_string(),
        })
        .region(ImageRegion {
            id: "tutorial-message-bank".to_string(),
            range: message_bank.file_offset..message_bank.file_offset + message_bank.byte_length,
            kind: RegionKind::Data,
            reason: "repacked Korean tutorial message bank".to_string(),
        })
        .region(ImageRegion {
            id: "tutorial-message-pointer-table".to_string(),
            range: message_bank.pointer_table_file_offset
                ..message_bank.pointer_table_file_offset
                    + message_bank.pointer_table_replacement.len(),
            kind: RegionKind::Metadata,
            reason: "ExpMsg_Data pointers for the repacked Korean messages".to_string(),
        });
    for assignment in assignments {
        let offset = font_table.file_offset + assignment.slot_index * 2;
        plan = plan.write(ExpectedWrite {
            id: format!("assign-tutorial-font-slot-{}", assignment.slot_index),
            owner: "tutorial-translation-encoding".to_string(),
            purpose: format!(
                "assign {} to its deterministic tutorial slot",
                assignment.character
            ),
            offset,
            expected_original: source[offset..offset + 2].to_vec(),
            replacement: decode_hex(&assignment.code_hex)?,
            intent: WriteIntent::Metadata,
        });
    }
    plan = plan.write(ExpectedWrite {
        id: "update-tutorial-font-cpx-stored-length".to_string(),
        owner: "tutorial-translation-compressor".to_string(),
        purpose: "make the source loader consume the complete tutorial font CPX".to_string(),
        offset: stored_length_offset,
        expected_original: expected_stored_length.to_le_bytes().to_vec(),
        replacement: replacement_stored_length.to_vec(),
        intent: WriteIntent::Metadata,
    });
    plan = plan
        .write(ExpectedWrite {
            id: "replace-tutorial-message-bank".to_string(),
            owner: "tutorial-translation-development".to_string(),
            purpose: "replace the complete Japanese bank with tightly packed Korean messages"
                .to_string(),
            offset: message_bank.file_offset,
            expected_original: message_bank.source_bytes.clone(),
            replacement: message_bank.replacement.clone(),
            intent: WriteIntent::Data,
        })
        .write(ExpectedWrite {
            id: "repoint-tutorial-message-table".to_string(),
            owner: "tutorial-translation-development".to_string(),
            purpose: "point all twelve ExpMsg_Data entries at the repacked Korean messages"
                .to_string(),
            offset: message_bank.pointer_table_file_offset,
            expected_original: message_bank.pointer_table_source_bytes.clone(),
            replacement: message_bank.pointer_table_replacement.clone(),
            intent: WriteIntent::Metadata,
        });
    let write_count = plan.writes.len();
    Ok((plan.apply(source, None)?, write_count))
}

pub(super) fn verify_patched_executable(
    source: &[u8],
    patched: &[u8],
    assignments: &[ExecutableTextGlyphAssignment],
    message_bank: &EncodedTutorialMessageBank<'_>,
) -> Result<()> {
    let table = read_executable_symbol(patched, FONT_TABLE_SYMBOL)?;
    ensure!(
        table.bytes.last() == Some(&0),
        "FontList_Exp terminator changed"
    );
    for assignment in assignments {
        let code = decode_hex(&assignment.code_hex)?;
        let first_match = table.bytes[..table.bytes.len() - 1]
            .chunks_exact(2)
            .position(|entry| entry == code)
            .with_context(|| format!("tutorial code disappeared: {}", assignment.code_hex))?;
        ensure!(
            first_match == assignment.slot_index,
            "tutorial custom code first-match differs for {}",
            assignment.character
        );
    }
    ensure!(
        source[message_bank.file_offset..message_bank.file_offset + message_bank.byte_length]
            == message_bank.source_bytes
            && patched
                [message_bank.file_offset..message_bank.file_offset + message_bank.byte_length]
                == message_bank.replacement
            && source[message_bank.pointer_table_file_offset
                ..message_bank.pointer_table_file_offset
                    + message_bank.pointer_table_source_bytes.len()]
                == message_bank.pointer_table_source_bytes
            && patched[message_bank.pointer_table_file_offset
                ..message_bank.pointer_table_file_offset
                    + message_bank.pointer_table_replacement.len()]
                == message_bank.pointer_table_replacement,
        "tutorial message-bank or pointer-table readback differs"
    );
    for (index, message) in message_bank.messages.iter().enumerate() {
        let pointer_offset = message_bank.pointer_table_file_offset + index * 4;
        let pointer = u64::from(u32::from_le_bytes(
            patched[pointer_offset..pointer_offset + 4]
                .try_into()
                .expect("fixed tutorial pointer width"),
        ));
        let relative =
            usize::try_from(pointer.checked_sub(message_bank.address).with_context(|| {
                format!("tutorial pointer precedes the bank: {}", message.asset_id)
            })?)?;
        let start = message_bank.file_offset + relative;
        let terminator_relative = patched
            .get(start..message_bank.file_offset + message_bank.byte_length)
            .with_context(|| format!("tutorial pointer escapes the bank: {}", message.asset_id))?
            .iter()
            .position(|byte| *byte == 0)
            .with_context(|| format!("tutorial message has no NUL: {}", message.asset_id))?;
        let end = start + terminator_relative + 1;
        ensure!(
            pointer == message.packed_address
                && start == message.packed_file_offset
                && patched[start..end] == message.replacement,
            "tutorial message readback differs: {}",
            message.asset_id
        );
    }
    ensure!(
        message_bank.replacement[message_bank.used_byte_length..]
            .iter()
            .all(|byte| *byte == 0),
        "tutorial message-bank tail padding is not zero"
    );
    Ok(())
}

fn parse_hex_usize(source: &str) -> Result<usize> {
    usize::from_str_radix(source.trim_start_matches("0x"), 16)
        .with_context(|| format!("invalid hexadecimal offset: {source}"))
}

fn parse_hex_u64(source: &str) -> Result<u64> {
    u64::from_str_radix(source.trim_start_matches("0x"), 16)
        .with_context(|| format!("invalid hexadecimal address: {source}"))
}

fn decode_hex(source: &str) -> Result<Vec<u8>> {
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
