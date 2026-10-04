use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use super::font::{
    MemoryCardGlyphAssignment, decode_hex, encode_translation, validate_protected_tokens,
};
use crate::executable::read_executable_symbol;
use crate::indexed_texture::sha256;
use crate::memory_card_translation::MemoryCardTranslationBundle;

const FONT_TABLE_SYMBOL: &str = "FontList_Zen";
const IMAGE_FILE_RECORD_BYTES: usize = 20;
const REQUIRED_FONT_FILE_RECORDS: [(&str, usize); 2] =
    [("FileList_Image", 21), ("FileList_Image2", 122)];

#[derive(Clone, Copy)]
pub(super) struct FontFileRecordTarget {
    pub(super) role: &'static str,
    pub(super) file_list_symbol: &'static str,
    pub(super) image_file_record: usize,
    pub(super) expected_stored_length: u32,
}

pub(super) struct EncodedMemoryCardMessage<'a> {
    pub(super) message_id: &'a str,
    pub(super) symbol: &'a str,
    pub(super) file_offset: usize,
    pub(super) storage_byte_length: usize,
    pub(super) source_bytes: Vec<u8>,
    pub(super) replacement: Vec<u8>,
}

pub(super) fn encode_memory_card_messages<'a>(
    executable: &[u8],
    bundle: &'a MemoryCardTranslationBundle,
    assignments: &[MemoryCardGlyphAssignment],
) -> Result<Vec<EncodedMemoryCardMessage<'a>>> {
    let mut offsets = BTreeSet::new();
    bundle
        .entries
        .iter()
        .map(|entry| {
            ensure!(
                !entry.translation.text.is_empty(),
                "memory-card development build cannot select an untranslated message: {}",
                entry.source.message_id
            );
            let symbol = read_executable_symbol(executable, &entry.source.symbol)?;
            let source_bytes = decode_hex(&entry.source.source_hex)?;
            ensure!(
                symbol.file_offset == parse_hex_usize(&entry.source.source_file_offset)?
                    && symbol.address
                        == u64::try_from(parse_hex_usize(&entry.source.source_address)?)?
                    && symbol.bytes == source_bytes
                    && symbol.bytes.len() == entry.source.storage_byte_length
                    && sha256(&symbol.bytes) == entry.source.source_sha256,
                "memory-card source symbol differs from the validated translation entry: {}",
                entry.source.message_id
            );
            ensure!(
                offsets.insert(symbol.file_offset),
                "memory-card translation entries share a physical source after deduplication: {}",
                entry.source.message_id
            );
            let replacement = encode_translation(&entry.translation.text, assignments)?;
            ensure!(
                replacement.len() <= symbol.bytes.len(),
                "memory-card translation exceeds fixed storage: {}",
                entry.source.message_id
            );
            validate_protected_tokens(entry, &replacement)?;
            Ok(EncodedMemoryCardMessage {
                message_id: &entry.source.message_id,
                symbol: &entry.source.symbol,
                file_offset: symbol.file_offset,
                storage_byte_length: symbol.bytes.len(),
                source_bytes,
                replacement,
            })
        })
        .collect()
}

pub(super) fn patch_executable(
    source: &[u8],
    assignments: &[MemoryCardGlyphAssignment],
    messages: &[EncodedMemoryCardMessage<'_>],
    file_record_targets: &[FontFileRecordTarget],
    replacement_cpx_length: usize,
) -> Result<(Vec<u8>, usize)> {
    validate_font_file_record_targets(file_record_targets)?;
    let font_table = read_executable_symbol(source, FONT_TABLE_SYMBOL)?;
    let replacement_stored_length = u32::try_from(
        replacement_cpx_length
            .checked_sub(1)
            .context("memory-card CPX has no terminator byte")?,
    )?
    .to_le_bytes();

    let mut plan = WritePlan::new().region(ImageRegion {
        id: "memory-card-font-list".to_string(),
        range: font_table.file_offset..font_table.file_offset + font_table.bytes.len(),
        kind: RegionKind::Metadata,
        reason: "FontList_Zen fullwidth lookup table".to_string(),
    });
    let mut stored_length_offsets = Vec::with_capacity(file_record_targets.len());
    for target in file_record_targets {
        let stored_length_offset = stored_length_offset(source, *target)?;
        plan = plan.region(ImageRegion {
            id: format!("memory-card-{}-cpx-stored-length", sanitize_id(target.role)),
            range: stored_length_offset..stored_length_offset + 4,
            kind: RegionKind::Metadata,
            reason: format!(
                "{} record {} stored-length field for {}",
                target.file_list_symbol, target.image_file_record, target.role
            ),
        });
        stored_length_offsets.push(stored_length_offset);
    }
    for message in messages {
        plan = plan.region(ImageRegion {
            id: format!("memory-card-message-{}", sanitize_id(message.message_id)),
            range: message.file_offset..message.file_offset + message.replacement.len(),
            kind: RegionKind::Data,
            reason: format!("translated memory-card message {}", message.message_id),
        });
    }
    for assignment in assignments {
        let offset = font_table.file_offset + assignment.slot_index * 2;
        plan = plan.write(ExpectedWrite {
            id: format!("assign-memory-card-font-slot-{}", assignment.slot_index),
            owner: "memory-card-translation-encoding".to_string(),
            purpose: format!(
                "assign {} to its deterministic memory-card slot",
                assignment.character
            ),
            offset,
            expected_original: source[offset..offset + 2].to_vec(),
            replacement: decode_hex(&assignment.code_hex)?,
            intent: WriteIntent::Metadata,
        });
    }
    for (target, stored_length_offset) in file_record_targets.iter().zip(stored_length_offsets) {
        plan = plan.write(ExpectedWrite {
            id: format!(
                "update-memory-card-{}-cpx-stored-length",
                sanitize_id(target.role)
            ),
            owner: "memory-card-translation-compressor".to_string(),
            purpose: format!(
                "make {} record {} consume the complete memory-card font CPX",
                target.file_list_symbol, target.image_file_record
            ),
            offset: stored_length_offset,
            expected_original: target.expected_stored_length.to_le_bytes().to_vec(),
            replacement: replacement_stored_length.to_vec(),
            intent: WriteIntent::Metadata,
        });
    }
    for message in messages {
        plan = plan.write(ExpectedWrite {
            id: format!(
                "replace-memory-card-message-{}",
                sanitize_id(message.message_id)
            ),
            owner: "memory-card-translation-development".to_string(),
            purpose: "replace Japanese memory-card text with its direct Korean draft".to_string(),
            offset: message.file_offset,
            expected_original: message.source_bytes[..message.replacement.len()].to_vec(),
            replacement: message.replacement.clone(),
            intent: WriteIntent::Data,
        });
    }
    let write_count = plan.writes.len();
    Ok((plan.apply(source, None)?, write_count))
}

pub(super) fn verify_patched_executable(
    source: &[u8],
    patched: &[u8],
    assignments: &[MemoryCardGlyphAssignment],
    messages: &[EncodedMemoryCardMessage<'_>],
    file_record_targets: &[FontFileRecordTarget],
    replacement_cpx_length: usize,
) -> Result<()> {
    validate_font_file_record_targets(file_record_targets)?;
    let table = read_executable_symbol(patched, FONT_TABLE_SYMBOL)?;
    ensure!(
        table.bytes.last() == Some(&0),
        "FontList_Zen terminator changed"
    );
    for assignment in assignments {
        let code = decode_hex(&assignment.code_hex)?;
        let first_match = table.bytes[..table.bytes.len() - 1]
            .chunks_exact(2)
            .position(|entry| entry == code)
            .with_context(|| format!("memory-card code disappeared: {}", assignment.code_hex))?;
        ensure!(
            first_match == assignment.slot_index,
            "memory-card custom code first-match differs for {}",
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
            "memory-card message readback differs: {}",
            message.message_id
        );
    }
    for target in file_record_targets {
        let file_list = read_executable_symbol(patched, target.file_list_symbol)?;
        let relative = target
            .image_file_record
            .checked_mul(IMAGE_FILE_RECORD_BYTES)
            .and_then(|offset| offset.checked_add(8))
            .context("memory-card file-record readback offset overflow")?;
        let field = file_list
            .bytes
            .get(relative..relative + 4)
            .with_context(|| {
                format!(
                    "{} record {} stored-length field exceeds its symbol",
                    target.file_list_symbol, target.image_file_record
                )
            })?;
        let stored_length = u32::from_le_bytes(field.try_into()?);
        ensure!(
            stored_length as usize + 1 == replacement_cpx_length,
            "{} record {} CPX stored-length readback differs",
            target.file_list_symbol,
            target.image_file_record
        );
    }
    Ok(())
}

fn stored_length_offset(source: &[u8], target: FontFileRecordTarget) -> Result<usize> {
    let file_list = read_executable_symbol(source, target.file_list_symbol)?;
    let relative = target
        .image_file_record
        .checked_mul(IMAGE_FILE_RECORD_BYTES)
        .and_then(|offset| offset.checked_add(8))
        .context("memory-card file-record stored-length offset overflow")?;
    let offset = file_list
        .file_offset
        .checked_add(relative)
        .context("memory-card file-record absolute offset overflow")?;
    let field = source.get(offset..offset + 4).with_context(|| {
        format!(
            "{} record {} stored-length field exceeds the executable",
            target.file_list_symbol, target.image_file_record
        )
    })?;
    ensure!(
        field == target.expected_stored_length.to_le_bytes(),
        "{} record {} source CPX stored length changed",
        target.file_list_symbol,
        target.image_file_record
    );
    Ok(offset)
}

fn validate_font_file_record_targets(targets: &[FontFileRecordTarget]) -> Result<()> {
    let actual = targets
        .iter()
        .map(|target| (target.file_list_symbol, target.image_file_record))
        .collect::<BTreeSet<_>>();
    ensure!(
        targets.len() == REQUIRED_FONT_FILE_RECORDS.len()
            && actual == REQUIRED_FONT_FILE_RECORDS.into_iter().collect(),
        "memory-card font stored-length targets must be exactly FileList_Image record 21 and FileList_Image2 record 122"
    );
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

#[cfg(test)]
#[path = "executable_patch_tests.rs"]
mod executable_patch_tests;
