use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use crate::memory_card_translation::{KoreanTranslation, TranslationStatus};

use super::{
    ExecutableTextConsumerDemand, ExecutableTextTranslationBundle, ExecutableTextTranslationInputs,
    ExecutableTextTranslationSource, ExecutableTextTranslationValidation,
    compose_executable_text_translation_bundle,
};

pub fn validate_executable_text_translation_bundle(
    bundle: &ExecutableTextTranslationBundle,
    inputs: ExecutableTextTranslationInputs<'_>,
) -> Result<ExecutableTextTranslationValidation> {
    let tutorial_message_bank_byte_length = inputs.tutorial.message_bank.byte_length;
    ensure!(
        inputs.tutorial.message_bank.message_count == inputs.tutorial.messages.len()
            && inputs.tutorial.layout.line_cell_capacity == TUTORIAL_LINE_CELL_CAPACITY
            && inputs.tutorial.layout.line_capacity == TUTORIAL_LINE_CAPACITY,
        "tutorial source report does not carry the verified message-bank and 9x8 layout contract"
    );
    let expected = compose_executable_text_translation_bundle(inputs)?;
    ensure!(
        bundle.schema == 1,
        "unsupported executable text translation schema"
    );
    ensure!(
        bundle.source == expected.source
            && bundle.scope == expected.scope
            && bundle.population == expected.population,
        "executable text translation header differs from the fresh exact-source baseline"
    );
    ensure!(
        bundle.entries.len() == expected.entries.len(),
        "executable text translation bundle does not cover the exact source population"
    );

    let expected_sources = expected
        .entries
        .into_iter()
        .map(|entry| (entry.source.asset_id.clone(), entry.source))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut demand = BTreeMap::<String, (usize, BTreeSet<char>)>::new();
    let mut validation = ExecutableTextTranslationValidation {
        total_entries: bundle.entries.len(),
        untranslated: 0,
        in_progress: 0,
        needs_review: 0,
        needs_human_review: 0,
        complete: 0,
        release_candidate_eligible: 0,
        fixed_storage_checks: 0,
        minimum_remaining_storage_bytes: None,
        tutorial_message_bank_byte_length,
        tutorial_encoded_byte_length: 0,
        tutorial_remaining_message_bank_bytes: tutorial_message_bank_byte_length,
        consumer_demands: Vec::new(),
    };
    let mut previous_id: Option<&str> = None;

    for entry in &bundle.entries {
        let asset_id = entry.source.asset_id.as_str();
        ensure!(
            previous_id.is_none_or(|previous| previous < asset_id),
            "executable text translation entries are not in stable asset-id order"
        );
        previous_id = Some(asset_id);
        ensure!(
            seen.insert(asset_id),
            "duplicate executable translation entry: {asset_id}"
        );
        let expected_source = expected_sources.get(asset_id).ok_or_else(|| {
            anyhow::anyhow!("unexpected executable translation entry: {asset_id}")
        })?;
        ensure!(
            &entry.source == expected_source,
            "protected source fields differ for executable translation entry: {asset_id}"
        );
        validate_translation(&entry.source, &entry.translation)?;
        match entry.translation.status {
            TranslationStatus::Untranslated => validation.untranslated += 1,
            TranslationStatus::InProgress => validation.in_progress += 1,
            TranslationStatus::NeedsReview => validation.needs_review += 1,
            TranslationStatus::NeedsHumanReview => validation.needs_human_review += 1,
            TranslationStatus::Complete => validation.complete += 1,
        }
        if !entry.translation.text.is_empty() {
            let encoded_length =
                encoded_length(&entry.source.consumer_family, &entry.translation.text)?;
            if entry.source.consumer_family == "tutorial" {
                validation.tutorial_encoded_byte_length += encoded_length;
            } else {
                ensure!(
                    encoded_length <= entry.source.storage_byte_length,
                    "translation exceeds fixed storage by {} bytes: {asset_id}",
                    encoded_length - entry.source.storage_byte_length
                );
                validation.fixed_storage_checks += 1;
                let remaining = entry.source.storage_byte_length - encoded_length;
                validation.minimum_remaining_storage_bytes = Some(
                    validation
                        .minimum_remaining_storage_bytes
                        .map_or(remaining, |current| current.min(remaining)),
                );
            }
            let family = demand
                .entry(entry.source.consumer_family.clone())
                .or_default();
            family.0 += 1;
            family.1.extend(
                entry
                    .translation
                    .text
                    .chars()
                    .filter(|character| is_hangul_syllable(*character)),
            );
        }
    }
    ensure!(
        seen.len() == expected_sources.len(),
        "executable text translation bundle is missing an exact-source entry"
    );
    ensure!(
        validation.tutorial_encoded_byte_length <= tutorial_message_bank_byte_length,
        "tutorial translations exceed the shared message bank by {} bytes",
        validation.tutorial_encoded_byte_length - tutorial_message_bank_byte_length
    );
    validation.tutorial_remaining_message_bank_bytes =
        tutorial_message_bank_byte_length - validation.tutorial_encoded_byte_length;
    validation.consumer_demands = demand
        .into_iter()
        .map(
            |(consumer_family, (translated_entries, glyphs))| ExecutableTextConsumerDemand {
                consumer_family,
                translated_entries,
                required_unique_hangul_glyphs: glyphs.len(),
            },
        )
        .collect();
    Ok(validation)
}

fn validate_translation(
    source: &ExecutableTextTranslationSource,
    translation: &KoreanTranslation,
) -> Result<()> {
    match translation.status {
        TranslationStatus::Untranslated => ensure!(
            translation.text.is_empty()
                && translation.human_approval.is_none()
                && translation.independent_review.is_none(),
            "untranslated entry contains translation or approval data: {}",
            source.asset_id
        ),
        TranslationStatus::InProgress
        | TranslationStatus::NeedsReview
        | TranslationStatus::NeedsHumanReview => ensure!(
            !translation.text.is_empty(),
            "translated entry has no translation: {}",
            source.asset_id
        ),
        TranslationStatus::Complete => anyhow::bail!(
            "complete status is unavailable until consumer-specific glyph, reinsertion, layout, runtime, and release-candidate eligibility are represented: {}",
            source.asset_id
        ),
    }
    if translation.text.is_empty() {
        return Ok(());
    }
    ensure!(
        !translation.text.contains('\0'),
        "translation contains an interior NUL: {}",
        source.asset_id
    );
    validate_protected_ascii(source, &translation.text)?;
    if source.consumer_family == "tutorial" {
        validate_tutorial_layout(source, &translation.text)?;
    }
    if source.consumer_family == "puzzle_puyo" {
        validate_puzzle_puyo_layout(source, &translation.text)?;
    }
    if matches!(
        translation.status,
        TranslationStatus::NeedsReview
            | TranslationStatus::NeedsHumanReview
            | TranslationStatus::Complete
    ) {
        ensure!(
            translation
                .text
                .chars()
                .all(|character| character.is_ascii() || is_hangul_syllable(character)),
            "review-ready translation contains a character outside ASCII and Hangul syllables: {}",
            source.asset_id
        );
    }
    Ok(())
}

const TUTORIAL_LINE_CELL_CAPACITY: usize = 9;
const TUTORIAL_LINE_CAPACITY: usize = 8;

fn validate_tutorial_layout(source: &ExecutableTextTranslationSource, text: &str) -> Result<()> {
    for (line_index, line) in text.lines().enumerate() {
        let cell_count = line.chars().count();
        ensure!(
            cell_count <= TUTORIAL_LINE_CELL_CAPACITY,
            "tutorial translation line {} uses {cell_count} cells but the window holds {TUTORIAL_LINE_CELL_CAPACITY}: {}",
            line_index + 1,
            source.asset_id
        );
    }
    let line_count = text.lines().count();
    ensure!(
        line_count <= TUTORIAL_LINE_CAPACITY,
        "tutorial translation uses {line_count} lines but the consumer holds {TUTORIAL_LINE_CAPACITY}: {}",
        source.asset_id
    );
    Ok(())
}

fn validate_puzzle_puyo_layout(source: &ExecutableTextTranslationSource, text: &str) -> Result<()> {
    let source_line_capacity = source
        .source_text
        .lines()
        .map(puzzle_puyo_halfwidth_cells)
        .max()
        .unwrap_or(0);
    for (line_index, line) in text.lines().enumerate() {
        let halfwidth_cells = puzzle_puyo_halfwidth_cells(line);
        ensure!(
            halfwidth_cells <= source_line_capacity,
            "Puzzle Puyo translation line {} uses {halfwidth_cells} halfwidth cells but the source-backed window allocation holds {source_line_capacity}: {}",
            line_index + 1,
            source.asset_id
        );
    }
    let line_count = text.lines().count();
    let source_line_count = source.source_text.lines().count();
    ensure!(
        line_count <= source_line_count,
        "Puzzle Puyo translation uses {line_count} lines but the source window allocation has {source_line_count}: {}",
        source.asset_id
    );
    Ok(())
}

fn puzzle_puyo_halfwidth_cells(line: &str) -> usize {
    line.chars()
        .map(|character| if character.is_ascii() { 1 } else { 2 })
        .sum()
}

fn validate_protected_ascii(source: &ExecutableTextTranslationSource, text: &str) -> Result<()> {
    let mut cursor = 0_usize;
    for run in &source.protected_ascii {
        let relative = text[cursor..].find(run).ok_or_else(|| {
            anyhow::anyhow!(
                "protected ASCII run is missing or reordered in {}: {}",
                source.asset_id,
                run
            )
        })?;
        cursor += relative + run.len();
    }
    Ok(())
}

fn encoded_length(consumer_family: &str, text: &str) -> Result<usize> {
    let mut bytes = 1_usize;
    for character in text.chars() {
        if character == '\n' {
            bytes += 1;
        } else if consumer_family == "tutorial" && character == ' ' {
            bytes += 2;
        } else if consumer_family == "tutorial" && character.is_ascii() {
            anyhow::bail!(
                "tutorial translation character has no two-byte renderer encoding: {character}"
            );
        } else if character.is_ascii() {
            bytes += 1;
        } else if is_hangul_syllable(character) {
            bytes += 2;
        } else {
            anyhow::bail!("translation cannot be encoded by the development codec: {character}");
        }
    }
    Ok(bytes)
}

fn is_hangul_syllable(character: char) -> bool {
    matches!(character, '\u{ac00}'..='\u{d7a3}')
}

#[cfg(test)]
pub(super) fn validate_translation_for_test(
    source: &ExecutableTextTranslationSource,
    translation: &KoreanTranslation,
) -> Result<()> {
    validate_translation(source, translation)
}

#[cfg(test)]
pub(super) fn encoded_length_for_test(consumer_family: &str, text: &str) -> Result<usize> {
    encoded_length(consumer_family, text)
}
