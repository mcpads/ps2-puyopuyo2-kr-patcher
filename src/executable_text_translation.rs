use std::collections::BTreeMap;

use anyhow::{Result, ensure};

use crate::memory_card_metadata::MemoryCardMetadataReport;
use crate::memory_card_translation::{
    KoreanTranslation, TranslationBundleSource, TranslationStatus,
};
use crate::message_corpus::{MessageEntry, NazoMessageCorpusReport, TutorialMessageCorpusReport};

mod glyph_plan;
mod model;
mod validation;

pub use glyph_plan::{
    ExecutableTextGlyphPlanningInputs, plan_executable_text_glyphs,
    plan_puzzle_puyo_halfwidth_glyphs, plan_puzzle_puyo_text_glyphs, plan_tutorial_text_glyphs,
};
pub use model::*;
pub use validation::validate_executable_text_translation_bundle;

pub struct ExecutableTextTranslationInputs<'a> {
    pub source_bin_sha256: &'a str,
    pub executable_iso_path: &'a str,
    pub executable_sha256: &'a str,
    pub tutorial: &'a TutorialMessageCorpusReport,
    pub puzzle_puyo: &'a NazoMessageCorpusReport,
    pub memory_card_metadata: &'a MemoryCardMetadataReport,
}

pub fn compose_executable_text_translation_bundle(
    inputs: ExecutableTextTranslationInputs<'_>,
) -> Result<ExecutableTextTranslationBundle> {
    validate_source_identity(&inputs)?;
    ensure!(
        inputs.tutorial.population.unresolved_sites == 0
            && inputs.tutorial.population.unique_messages == inputs.tutorial.messages.len(),
        "tutorial corpus is not a closed message population"
    );
    ensure!(
        inputs.puzzle_puyo.population.unresolved_sites == 0
            && inputs.puzzle_puyo.population.unique_messages == inputs.puzzle_puyo.messages.len(),
        "Puzzle Puyo corpus is not a closed message population"
    );
    ensure!(
        inputs.memory_card_metadata.population.unresolved_entries == 0
            && inputs
                .memory_card_metadata
                .population
                .japanese_title_entries
                == 1,
        "memory-card metadata title population is not closed"
    );

    let mut entries = BTreeMap::<String, ExecutableTextTranslationSource>::new();
    for message in &inputs.tutorial.messages {
        insert_message_source(&mut entries, "tutorial", message)?;
    }
    for message in &inputs.puzzle_puyo.messages {
        insert_message_source(&mut entries, "puzzle_puyo", message)?;
    }
    insert_metadata_source(&mut entries, inputs.memory_card_metadata)?;

    let entries = entries
        .into_values()
        .map(|source| ExecutableTextTranslationEntry {
            source,
            translation: KoreanTranslation {
                status: TranslationStatus::Untranslated,
                text: String::new(),
                translator_notes: Vec::new(),
                human_approval: None,
                independent_review: None,
            },
        })
        .collect::<Vec<_>>();

    Ok(ExecutableTextTranslationBundle {
        schema: 1,
        source: TranslationBundleSource {
            source_bin_sha256: inputs.source_bin_sha256.to_string(),
            executable_iso_path: inputs.executable_iso_path.to_string(),
            executable_sha256: inputs.executable_sha256.to_string(),
        },
        scope: ExecutableTextTranslationScope {
            consumer_families: vec![
                "memory_card_metadata".to_string(),
                "puzzle_puyo".to_string(),
                "tutorial".to_string(),
            ],
            target: "Japanese Shift-JIS text in the closed tutorial and Puzzle Puyo corpora, plus the Japanese memory-card icon title".to_string(),
            preserve: "all non-whitespace ASCII tokens in original order; English is never a translation target".to_string(),
            build_policy: "review state controls release-candidate eligibility, not explicit non-distribution development/PoC selection; tutorial text is repacked in its verified shared bank and composed only against the 9-cell by 8-line consumer limit, while every consumer family still requires its own glyph allocation, reinsertion, layout, and runtime proof".to_string(),
            release_candidate: false,
        },
        population: ExecutableTextTranslationPopulation {
            tutorial_messages: inputs.tutorial.messages.len(),
            puzzle_puyo_messages: inputs.puzzle_puyo.messages.len(),
            memory_card_metadata_titles: 1,
            total_entries: entries.len(),
        },
        entries,
    })
}

fn validate_source_identity(inputs: &ExecutableTextTranslationInputs<'_>) -> Result<()> {
    let expected = (
        inputs.source_bin_sha256,
        inputs.executable_iso_path,
        inputs.executable_sha256,
    );
    for (name, actual) in [
        (
            "tutorial",
            (
                inputs.tutorial.source_bin_sha256.as_str(),
                inputs.tutorial.executable_iso_path.as_str(),
                inputs.tutorial.executable_sha256.as_str(),
            ),
        ),
        (
            "Puzzle Puyo",
            (
                inputs.puzzle_puyo.source_bin_sha256.as_str(),
                inputs.puzzle_puyo.executable_iso_path.as_str(),
                inputs.puzzle_puyo.executable_sha256.as_str(),
            ),
        ),
        (
            "memory-card metadata",
            (
                inputs.memory_card_metadata.source_bin_sha256.as_str(),
                inputs.memory_card_metadata.executable_iso_path.as_str(),
                inputs.memory_card_metadata.executable_sha256.as_str(),
            ),
        ),
    ] {
        ensure!(
            actual == expected,
            "{name} does not share the exact source identity"
        );
    }
    Ok(())
}

fn insert_message_source(
    entries: &mut BTreeMap<String, ExecutableTextTranslationSource>,
    consumer_family: &str,
    message: &MessageEntry,
) -> Result<()> {
    ensure!(
        message
            .tokens
            .iter()
            .any(|token| token.kind == "shift-jis-text"),
        "executable translation entry has no Japanese Shift-JIS token: {}",
        message.message_id
    );
    ensure!(
        message
            .tokens
            .iter()
            .all(|token| matches!(token.kind, "ascii-text" | "shift-jis-text" | "line-break")),
        "executable translation entry has an unsupported token: {}",
        message.message_id
    );
    let protected_ascii = message
        .tokens
        .iter()
        .filter(|token| {
            token.kind == "ascii-text"
                && token
                    .value
                    .chars()
                    .any(|character| !character.is_ascii_whitespace())
        })
        .map(|token| token.value.clone())
        .collect();
    let source = ExecutableTextTranslationSource {
        asset_id: message.message_id.clone(),
        consumer_family: consumer_family.to_string(),
        symbol: message.symbol.clone(),
        source_address: message.source_address.clone(),
        source_file_offset: message.source_file_offset.clone(),
        storage_byte_length: message.storage_byte_length,
        payload_byte_length: message.payload_byte_length,
        source_sha256: message.source_sha256.clone(),
        source_hex: message.source_hex.clone(),
        terminator: message.terminator.to_string(),
        source_text: message.display_text.clone(),
        protected_ascii,
        reference_ids: message.reference_ids.clone(),
    };
    ensure!(
        entries.insert(message.message_id.clone(), source).is_none(),
        "duplicate executable translation asset: {}",
        message.message_id
    );
    Ok(())
}

fn insert_metadata_source(
    entries: &mut BTreeMap<String, ExecutableTextTranslationSource>,
    report: &MemoryCardMetadataReport,
) -> Result<()> {
    let title = &report.title;
    let asset_id = title.metadata_id.to_string();
    let source = ExecutableTextTranslationSource {
        asset_id: asset_id.clone(),
        consumer_family: "memory_card_metadata".to_string(),
        symbol: title.symbol.to_string(),
        source_address: title.address.clone(),
        source_file_offset: title.file_offset.clone(),
        storage_byte_length: title.storage_byte_length,
        payload_byte_length: title.storage_byte_length - 1,
        source_sha256: title.source_sha256.clone(),
        source_hex: title.source_hex.clone(),
        terminator: title.terminator.to_string(),
        source_text: title.display_text.clone(),
        protected_ascii: Vec::new(),
        reference_ids: vec!["memory-card/icon-system-write-pipeline".to_string()],
    };
    ensure!(
        entries.insert(asset_id.clone(), source).is_none(),
        "duplicate executable translation asset: {asset_id}"
    );
    Ok(())
}

#[cfg(test)]
#[path = "executable_text_translation/executable_text_translation_tests.rs"]
mod executable_text_translation_tests;
