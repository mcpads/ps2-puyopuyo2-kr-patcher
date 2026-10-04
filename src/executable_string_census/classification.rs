use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use super::{
    ExecutableStringClassification, ExecutableStringReviewCatalog, ReviewedCandidateClassification,
};
use crate::executable_string_census::scanner::ScannedJapaneseString;

#[derive(Debug)]
pub(super) struct CandidateDecision {
    pub classification: ExecutableStringClassification,
    pub translation_target: bool,
    pub declared_message_corpora: Vec<&'static str>,
    pub translation_policy: &'static str,
    pub review_note: String,
}

pub(super) fn classify_candidates(
    candidates: &[ScannedJapaneseString],
    catalog: &ExecutableStringReviewCatalog,
    executable_sha256: &str,
) -> Result<BTreeMap<String, CandidateDecision>> {
    ensure!(
        catalog.schema == 1,
        "unsupported executable string review schema"
    );
    ensure!(
        catalog.executable_sha256 == executable_sha256,
        "executable string review catalog targets a different executable"
    );
    ensure!(
        !catalog.review_basis.trim().is_empty(),
        "executable string review basis is empty"
    );
    let mut reviews = BTreeMap::new();
    for entry in &catalog.entries {
        ensure!(
            reviews.insert(entry.symbol.as_str(), entry).is_none(),
            "duplicate executable string review entry: {}",
            entry.symbol
        );
    }
    let message_memberships = crate::message_corpus::declared_message_corpus_memberships();
    let mut decisions = BTreeMap::new();
    let mut consumed_reviews = BTreeSet::new();
    for candidate in candidates {
        let decision = if let Some(corpora) = message_memberships.get(candidate.symbol.as_str()) {
            ensure!(
                !reviews.contains_key(candidate.symbol.as_str()),
                "declared message candidate must not have a separate review entry: {}",
                candidate.symbol
            );
            CandidateDecision {
                classification: ExecutableStringClassification::DeclaredMessage,
                translation_target: true,
                declared_message_corpora: corpora.clone(),
                translation_policy: "translate Japanese runs only; preserve ASCII and control tokens according to every owning message corpus",
                review_note: "covered by producer-specific message corpus evidence".to_string(),
            }
        } else {
            let entry = reviews
                .get(candidate.symbol.as_str())
                .copied()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Japanese-script candidate has no message membership or review: {}",
                        candidate.symbol
                    )
                })?;
            ensure!(
                entry.address == format!("{:#010x}", candidate.address),
                "review address differs for {}",
                candidate.symbol
            );
            ensure!(
                entry.source_sha256 == crate::indexed_texture::sha256(&candidate.bytes),
                "review source hash differs for {}",
                candidate.symbol
            );
            ensure!(
                !entry.note.trim().is_empty(),
                "review note is empty for {}",
                candidate.symbol
            );
            consumed_reviews.insert(entry.symbol.as_str());
            let (classification, translation_target, translation_policy) = match entry
                .classification
            {
                ReviewedCandidateClassification::DisplayedJapaneseMetadata => (
                    ExecutableStringClassification::DisplayedJapaneseMetadata,
                    true,
                    "translate Japanese metadata text; preserve any ASCII bytes unchanged",
                ),
                ReviewedCandidateClassification::FontRepertoire => (
                    ExecutableStringClassification::FontRepertoire,
                    false,
                    "font lookup repertoire, not displayed prose; change only with its owning font contract",
                ),
                ReviewedCandidateClassification::NonStringBinary => (
                    ExecutableStringClassification::NonStringBinary,
                    false,
                    "binary address data, not translatable text",
                ),
            };
            CandidateDecision {
                classification,
                translation_target,
                declared_message_corpora: Vec::new(),
                translation_policy,
                review_note: entry.note.clone(),
            }
        };
        ensure!(
            decisions
                .insert(candidate.symbol.clone(), decision)
                .is_none(),
            "duplicate scanned candidate symbol: {}",
            candidate.symbol
        );
    }
    ensure!(
        consumed_reviews.len() == reviews.len(),
        "review catalog contains entries outside the scanned candidate population"
    );
    Ok(decisions)
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod classification_tests;
