use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use super::{
    KoreanTranslation, MemoryCardTranslationBundle, MemoryCardTranslationInputs,
    MemoryCardTranslationSource, MemoryCardTranslationValidation, TranslationStatus,
    compose_memory_card_translation_bundle,
};

pub fn validate_memory_card_translation_bundle(
    bundle: &MemoryCardTranslationBundle,
    inputs: MemoryCardTranslationInputs<'_>,
) -> Result<MemoryCardTranslationValidation> {
    let expected = compose_memory_card_translation_bundle(inputs)?;
    ensure!(
        bundle.schema == 1,
        "unsupported memory-card translation schema"
    );
    ensure!(
        bundle.source == expected.source
            && bundle.scope == expected.scope
            && bundle.population == expected.population,
        "memory-card translation bundle header differs from the fresh exact-source baseline"
    );
    ensure!(
        bundle.entries.len() == expected.entries.len(),
        "memory-card translation bundle does not cover the exact source population"
    );

    let expected_sources = expected
        .entries
        .into_iter()
        .map(|entry| (entry.source.message_id.clone(), entry.source))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut validation = MemoryCardTranslationValidation {
        unique_messages: bundle.entries.len(),
        untranslated: 0,
        in_progress: 0,
        needs_review: 0,
        needs_human_review: 0,
        complete: 0,
        release_candidate_eligible: 0,
    };
    let mut previous_id: Option<&str> = None;

    for entry in &bundle.entries {
        let message_id = entry.source.message_id.as_str();
        ensure!(
            previous_id.is_none_or(|previous| previous < message_id),
            "memory-card translation entries are not in stable message-id order"
        );
        previous_id = Some(message_id);
        ensure!(
            seen.insert(message_id),
            "duplicate memory-card translation entry: {message_id}"
        );
        let expected_source = expected_sources
            .get(message_id)
            .ok_or_else(|| anyhow::anyhow!("unexpected translation entry: {message_id}"))?;
        ensure!(
            &entry.source == expected_source,
            "protected source fields differ for translation entry: {message_id}"
        );
        validate_translation(&entry.source, &entry.translation)?;
        match entry.translation.status {
            TranslationStatus::Untranslated => validation.untranslated += 1,
            TranslationStatus::InProgress => validation.in_progress += 1,
            TranslationStatus::NeedsReview => validation.needs_review += 1,
            TranslationStatus::NeedsHumanReview => validation.needs_human_review += 1,
            TranslationStatus::Complete => {
                validation.complete += 1;
            }
        }
    }
    ensure!(
        seen.len() == expected_sources.len(),
        "memory-card translation bundle is missing an exact-source entry"
    );
    Ok(validation)
}

fn validate_translation(
    source: &MemoryCardTranslationSource,
    translation: &KoreanTranslation,
) -> Result<()> {
    match translation.status {
        TranslationStatus::Untranslated => ensure!(
            translation.text.is_empty()
                && translation.human_approval.is_none()
                && translation.independent_review.is_none(),
            "untranslated entry contains translation or approval data: {}",
            source.message_id
        ),
        TranslationStatus::InProgress => ensure!(
            !translation.text.is_empty(),
            "in-progress entry has no translation: {}",
            source.message_id
        ),
        TranslationStatus::NeedsReview | TranslationStatus::NeedsHumanReview => ensure!(
            !translation.text.is_empty(),
            "review entry has no translation: {}",
            source.message_id
        ),
        TranslationStatus::Complete => anyhow::bail!(
            "complete status is unavailable until glyph, storage, layout, runtime, and release-candidate eligibility are represented: {}",
            source.message_id
        ),
    }
    if translation.text.is_empty() {
        return Ok(());
    }
    ensure!(
        !translation.text.contains('\0'),
        "translation contains an interior NUL: {}",
        source.message_id
    );
    validate_runtime_controls(source, &translation.text)?;
    validate_protected_ascii(source, &translation.text)?;
    if matches!(
        translation.status,
        TranslationStatus::NeedsReview
            | TranslationStatus::NeedsHumanReview
            | TranslationStatus::Complete
    ) {
        ensure!(
            !translation.text.chars().any(is_japanese_kana),
            "review-ready translation still contains Japanese kana: {}",
            source.message_id
        );
    }
    Ok(())
}

fn validate_runtime_controls(source: &MemoryCardTranslationSource, text: &str) -> Result<()> {
    let actual = parse_runtime_controls(text)?;
    ensure!(
        actual == source.protected_runtime_controls,
        "runtime controls changed in translation entry: {}",
        source.message_id
    );
    Ok(())
}

fn parse_runtime_controls(text: &str) -> Result<Vec<String>> {
    let bytes = text.as_bytes();
    let mut controls = Vec::new();
    let mut cursor = 0_usize;
    while cursor < bytes.len() {
        if bytes[cursor] != b'@' {
            cursor += 1;
            continue;
        }
        let argument = *bytes
            .get(cursor + 1)
            .ok_or_else(|| anyhow::anyhow!("translation contains a truncated @ control"))?;
        ensure!(
            argument == b'0' || argument.is_ascii_uppercase(),
            "translation contains an unsupported @ control: @{argument}"
        );
        controls.push(format!("@{}", char::from(argument)));
        cursor += 2;
    }
    Ok(controls)
}

fn validate_protected_ascii(source: &MemoryCardTranslationSource, text: &str) -> Result<()> {
    let mut cursor = 0_usize;
    for run in &source.protected_ascii {
        let relative = text[cursor..].find(run).ok_or_else(|| {
            anyhow::anyhow!(
                "protected ASCII run is missing or reordered in {}: {}",
                source.message_id,
                run
            )
        })?;
        cursor += relative + run.len();
    }
    Ok(())
}

fn is_japanese_kana(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}'
            | '\u{31f0}'..='\u{31ff}'
    )
}

#[cfg(test)]
pub(super) fn validate_translation_for_test(
    source: &MemoryCardTranslationSource,
    translation: &KoreanTranslation,
) -> Result<()> {
    validate_translation(source, translation)
}
