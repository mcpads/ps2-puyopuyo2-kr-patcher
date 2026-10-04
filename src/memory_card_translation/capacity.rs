use std::collections::BTreeSet;

use anyhow::{Result, ensure};

use super::{MemoryCardTranslationBundle, MemoryCardTranslationCapacity};
use crate::glyph_allocation::GlyphAllocationReport;

pub fn validate_memory_card_translation_capacity(
    bundle: &MemoryCardTranslationBundle,
    allocation: &GlyphAllocationReport,
) -> Result<MemoryCardTranslationCapacity> {
    ensure!(
        bundle.source.source_bin_sha256 == allocation.source_bin_sha256
            && bundle.source.executable_iso_path == allocation.executable_iso_path
            && bundle.source.executable_sha256 == allocation.executable_sha256,
        "memory-card translation bundle and glyph allocation use different exact sources"
    );
    let translated_entries = bundle
        .entries
        .iter()
        .filter(|entry| !entry.translation.text.is_empty())
        .collect::<Vec<_>>();
    let texts = translated_entries
        .iter()
        .map(|entry| entry.translation.text.as_str())
        .collect::<Vec<_>>();
    let mut capacity =
        measure_translation_capacity(&texts, allocation.population.proven_reassignable_slots)?;
    let mut minimum_remaining_storage_bytes = None;
    for entry in &translated_entries {
        let encoded_byte_length = encoded_translation_byte_length(&entry.translation.text)?;
        ensure!(
            encoded_byte_length <= entry.source.storage_byte_length,
            "translation {} needs {encoded_byte_length} bytes including NUL but its fixed source storage is only {} bytes",
            entry.source.message_id,
            entry.source.storage_byte_length
        );
        let remaining = entry.source.storage_byte_length - encoded_byte_length;
        minimum_remaining_storage_bytes = Some(
            minimum_remaining_storage_bytes
                .map_or(remaining, |minimum: usize| minimum.min(remaining)),
        );
    }
    capacity.fixed_storage_checks = translated_entries.len();
    capacity.minimum_remaining_storage_bytes = minimum_remaining_storage_bytes;
    Ok(capacity)
}

fn measure_translation_capacity(
    texts: &[&str],
    proven_reassignable_slots: usize,
) -> Result<MemoryCardTranslationCapacity> {
    let mut hangul = BTreeSet::new();
    for text in texts {
        for character in text.chars() {
            if ('\u{ac00}'..='\u{d7a3}').contains(&character) {
                hangul.insert(character);
            } else {
                ensure!(
                    character.is_ascii(),
                    "memory-card translation contains a non-ASCII character outside the supported Hangul syllable range: {character}"
                );
            }
        }
    }
    ensure!(
        hangul.len() <= proven_reassignable_slots,
        "active memory-card translations require {} unique Hangul glyphs but only {proven_reassignable_slots} reassignable slots are proven",
        hangul.len()
    );
    Ok(MemoryCardTranslationCapacity {
        translated_messages: texts.len(),
        required_unique_hangul_glyphs: hangul.len(),
        proven_reassignable_slots,
        remaining_reassignable_slots: proven_reassignable_slots - hangul.len(),
        fixed_storage_checks: 0,
        minimum_remaining_storage_bytes: None,
        status: "active translation glyph demand and fixed source storage fit; this is not layout, reinsertion, runtime, or release-candidate eligibility",
    })
}

fn encoded_translation_byte_length(text: &str) -> Result<usize> {
    let mut byte_length = 1_usize;
    for character in text.chars() {
        let width = if ('\u{ac00}'..='\u{d7a3}').contains(&character) {
            2
        } else {
            ensure!(
                character.is_ascii(),
                "memory-card translation contains a character without a current encoding: {character}"
            );
            1
        };
        byte_length = byte_length
            .checked_add(width)
            .ok_or_else(|| anyhow::anyhow!("encoded translation byte length overflow"))?;
    }
    Ok(byte_length)
}

#[cfg(test)]
pub(super) fn measure_translation_capacity_for_test(
    texts: &[&str],
    proven_reassignable_slots: usize,
) -> Result<MemoryCardTranslationCapacity> {
    measure_translation_capacity(texts, proven_reassignable_slots)
}

#[cfg(test)]
pub(super) fn encoded_translation_byte_length_for_test(text: &str) -> Result<usize> {
    encoded_translation_byte_length(text)
}
