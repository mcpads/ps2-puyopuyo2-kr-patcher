use std::collections::BTreeMap;

use anyhow::{Result, ensure};

use crate::message_corpus::{MessageCorpusReport, MessageEntry};

mod capacity;
mod model;
mod validation;

pub use capacity::validate_memory_card_translation_capacity;
pub use model::*;
pub use validation::validate_memory_card_translation_bundle;

pub struct MemoryCardTranslationInputs<'a> {
    pub source_bin_sha256: &'a str,
    pub executable_iso_path: &'a str,
    pub executable_sha256: &'a str,
    pub load: &'a MessageCorpusReport,
    pub save: &'a MessageCorpusReport,
    pub my_nazo_error: &'a MessageCorpusReport,
}

pub fn compose_memory_card_translation_bundle(
    inputs: MemoryCardTranslationInputs<'_>,
) -> Result<MemoryCardTranslationBundle> {
    let corpora = [
        ("memory_card_load", inputs.load),
        ("memory_card_save", inputs.save),
        ("my_nazo_error", inputs.my_nazo_error),
    ];
    let expected_identity = (
        inputs.source_bin_sha256,
        inputs.executable_iso_path,
        inputs.executable_sha256,
    );
    let mut entries = BTreeMap::<String, MemoryCardTranslationSource>::new();
    let mut corpus_entry_occurrences = 0_usize;

    for (corpus_id, report) in corpora {
        ensure!(
            (
                report.source_bin_sha256.as_str(),
                report.executable_iso_path.as_str(),
                report.executable_sha256.as_str(),
            ) == expected_identity,
            "memory-card translation corpora do not share one exact source identity"
        );
        ensure!(
            report.population.unresolved_sites == 0
                && report.population.unique_messages == report.messages.len(),
            "memory-card corpus {corpus_id} is not a closed message population"
        );
        corpus_entry_occurrences += report.messages.len();
        for message in &report.messages {
            merge_message_source(&mut entries, corpus_id, message)?;
        }
    }

    let shared_messages = entries
        .values()
        .filter(|source| source.consumer_corpora.len() > 1)
        .count();
    let entries = entries
        .into_values()
        .map(|source| MemoryCardTranslationEntry {
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

    Ok(MemoryCardTranslationBundle {
        schema: 1,
        source: TranslationBundleSource {
            source_bin_sha256: inputs.source_bin_sha256.to_string(),
            executable_iso_path: inputs.executable_iso_path.to_string(),
            executable_sha256: inputs.executable_sha256.to_string(),
        },
        scope: MemoryCardTranslationScope {
            consumer_corpora: vec![
                "memory_card_load".to_string(),
                "memory_card_save".to_string(),
                "my_nazo_error".to_string(),
            ],
            target: "Japanese Shift-JIS text in the three declared memory-card message consumers"
                .to_string(),
            preserve: "ASCII/English runs and runtime controls; line breaks may move only within a later validated layout"
                .to_string(),
            build_policy: "review state controls release-candidate eligibility, not explicit non-distribution development/PoC selection; every build must revalidate protected source fields, glyph coverage, storage, layout, and human decisions"
                .to_string(),
            release_candidate: false,
        },
        population: MemoryCardTranslationPopulation {
            corpus_entry_occurrences,
            unique_messages: entries.len(),
            shared_messages,
        },
        entries,
    })
}

fn merge_message_source(
    entries: &mut BTreeMap<String, MemoryCardTranslationSource>,
    corpus_id: &str,
    message: &MessageEntry,
) -> Result<()> {
    let incoming = source_from_message(corpus_id, message)?;
    if let Some(existing) = entries.get_mut(&message.message_id) {
        ensure!(
            existing.same_storage_as(&incoming),
            "shared memory-card message identity differs across corpora: {}",
            message.message_id
        );
        existing.consumer_corpora.push(corpus_id.to_string());
        existing.consumer_corpora.sort();
        existing.consumer_corpora.dedup();
        existing.reference_ids.extend(incoming.reference_ids);
        existing.reference_ids.sort();
        existing.reference_ids.dedup();
    } else {
        entries.insert(message.message_id.clone(), incoming);
    }
    Ok(())
}

fn source_from_message(
    corpus_id: &str,
    message: &MessageEntry,
) -> Result<MemoryCardTranslationSource> {
    let mut source_text = String::new();
    let mut protected_runtime_controls = Vec::new();
    for token in &message.tokens {
        match token.kind {
            "ascii-text" | "shift-jis-text" => source_text.push_str(&token.value),
            "line-break" => source_text.push('\n'),
            "button-glyph" => {
                let control = format!("@{}", token.value);
                source_text.push_str(&control);
                protected_runtime_controls.push(control);
            }
            "dynamic-port" => {
                source_text.push_str("@P");
                protected_runtime_controls.push("@P".to_string());
            }
            "color-control" => {
                source_text.push_str("@0");
                protected_runtime_controls.push("@0".to_string());
            }
            kind => anyhow::bail!("unsupported memory-card source token kind: {kind}"),
        }
    }
    ensure!(
        message
            .tokens
            .iter()
            .any(|token| token.kind == "shift-jis-text"),
        "memory-card translation entry has no Japanese Shift-JIS token: {}",
        message.message_id
    );
    let protected_ascii = message
        .protected_english_runs
        .iter()
        .map(|run| run.text.clone())
        .collect();
    Ok(MemoryCardTranslationSource {
        message_id: message.message_id.clone(),
        symbol: message.symbol.clone(),
        source_address: message.source_address.clone(),
        source_file_offset: message.source_file_offset.clone(),
        storage_byte_length: message.storage_byte_length,
        payload_byte_length: message.payload_byte_length,
        source_sha256: message.source_sha256.clone(),
        source_hex: message.source_hex.clone(),
        terminator: message.terminator.to_string(),
        source_text,
        protected_ascii,
        protected_runtime_controls,
        consumer_corpora: vec![corpus_id.to_string()],
        reference_ids: message.reference_ids.clone(),
    })
}

impl MemoryCardTranslationSource {
    fn same_storage_as(&self, other: &Self) -> bool {
        self.message_id == other.message_id
            && self.symbol == other.symbol
            && self.source_address == other.source_address
            && self.source_file_offset == other.source_file_offset
            && self.storage_byte_length == other.storage_byte_length
            && self.payload_byte_length == other.payload_byte_length
            && self.source_sha256 == other.source_sha256
            && self.source_hex == other.source_hex
            && self.terminator == other.terminator
            && self.source_text == other.source_text
            && self.protected_ascii == other.protected_ascii
            && self.protected_runtime_controls == other.protected_runtime_controls
    }
}

pub fn translation_entry<'a>(
    bundle: &'a MemoryCardTranslationBundle,
    message_id: &str,
) -> Result<&'a MemoryCardTranslationEntry> {
    bundle
        .entries
        .iter()
        .find(|entry| entry.source.message_id == message_id)
        .ok_or_else(|| anyhow::anyhow!("translation bundle has no entry: {message_id}"))
}

#[cfg(test)]
#[path = "memory_card_translation/memory_card_translation_tests.rs"]
mod memory_card_translation_tests;
