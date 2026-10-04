use std::collections::BTreeSet;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::executable_text_translation::ExecutableTextTranslationBundle;
use crate::graphics_translation::GraphicsTranslationBundle;
use crate::memory_card_translation::MemoryCardTranslationBundle;
use crate::story_dialogue_translation::StoryDialogueTranslationBundle;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedTerminology {
    pub schema: u32,
    pub decisions: Vec<ApprovedTermDecision>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedTermDecision {
    pub decision_id: String,
    pub source_referents: Vec<String>,
    pub approved_korean: String,
    pub applicability: String,
    pub evidence: String,
    pub transfer_limit: String,
    pub state: ApprovedTermState,
    pub forbidden_korean_variants: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovedTermState {
    Approved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApprovedTerminologyValidation {
    pub approved_decisions: usize,
    pub checked_korean_outputs: usize,
    pub approved_term_occurrences: usize,
    pub forbidden_variant_occurrences: usize,
}

pub fn validate_approved_terminology(
    terminology: &ApprovedTerminology,
    memory_card: &MemoryCardTranslationBundle,
    executable_text: &ExecutableTextTranslationBundle,
    graphics: &GraphicsTranslationBundle,
    story: &StoryDialogueTranslationBundle,
) -> Result<ApprovedTerminologyValidation> {
    validate_terminology_decisions(terminology)?;
    let mut validation = ApprovedTerminologyValidation {
        approved_decisions: terminology.decisions.len(),
        checked_korean_outputs: 0,
        approved_term_occurrences: 0,
        forbidden_variant_occurrences: 0,
    };

    for entry in &memory_card.entries {
        validate_korean_output(
            terminology,
            &format!("memory-card/{}", entry.source.message_id),
            &entry.translation.text,
            &mut validation,
        )?;
    }
    for entry in &executable_text.entries {
        validate_korean_output(
            terminology,
            &format!("executable-text/{}", entry.source.asset_id),
            &entry.translation.text,
            &mut validation,
        )?;
    }
    for entry in &graphics.entries {
        for region in &entry.authoring.regions {
            if let Some(korean_text) = &region.korean_text {
                validate_korean_output(
                    terminology,
                    &format!(
                        "graphics/{}/{}",
                        entry.source.source_surface_id, region.region_id
                    ),
                    korean_text,
                    &mut validation,
                )?;
            }
        }
    }
    for entry in &story.entries {
        for segment in &entry.authoring.segments {
            validate_korean_output(
                terminology,
                &format!("story/{}", segment.segment_id),
                &segment.korean_text,
                &mut validation,
            )?;
        }
    }
    Ok(validation)
}

fn validate_terminology_decisions(terminology: &ApprovedTerminology) -> Result<()> {
    ensure!(
        terminology.schema == 1,
        "unsupported approved terminology schema"
    );
    ensure!(
        !terminology.decisions.is_empty(),
        "approved terminology has no decisions"
    );
    let mut decision_ids = BTreeSet::new();
    for decision in &terminology.decisions {
        ensure!(
            decision_ids.insert(decision.decision_id.as_str()),
            "duplicate approved terminology decision: {}",
            decision.decision_id
        );
        ensure!(
            !decision.decision_id.trim().is_empty()
                && !decision.approved_korean.trim().is_empty()
                && !decision.applicability.trim().is_empty()
                && !decision.evidence.trim().is_empty()
                && !decision.transfer_limit.trim().is_empty(),
            "approved terminology decision has an empty required field: {}",
            decision.decision_id
        );
        ensure!(
            !decision.source_referents.is_empty()
                && decision
                    .source_referents
                    .iter()
                    .all(|referent| !referent.trim().is_empty()),
            "approved terminology decision has no source referent: {}",
            decision.decision_id
        );
        let unique_referents = decision
            .source_referents
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        ensure!(
            unique_referents.len() == decision.source_referents.len(),
            "approved terminology decision has duplicate source referents: {}",
            decision.decision_id
        );
        ensure!(
            !decision.forbidden_korean_variants.is_empty(),
            "approved terminology decision has no forbidden variants: {}",
            decision.decision_id
        );
        let unique_variants = decision
            .forbidden_korean_variants
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        ensure!(
            unique_variants.len() == decision.forbidden_korean_variants.len()
                && unique_variants
                    .iter()
                    .all(|variant| !variant.trim().is_empty()),
            "approved terminology decision has empty or duplicate forbidden variants: {}",
            decision.decision_id
        );
        ensure!(
            !unique_variants.contains(decision.approved_korean.as_str()),
            "approved terminology is also forbidden: {}",
            decision.decision_id
        );
    }
    Ok(())
}

fn validate_korean_output(
    terminology: &ApprovedTerminology,
    output_id: &str,
    korean_text: &str,
    validation: &mut ApprovedTerminologyValidation,
) -> Result<()> {
    validation.checked_korean_outputs += 1;
    for decision in &terminology.decisions {
        validation.approved_term_occurrences +=
            korean_text.matches(&decision.approved_korean).count();
        for forbidden in &decision.forbidden_korean_variants {
            let occurrences = korean_text.matches(forbidden).count();
            validation.forbidden_variant_occurrences += occurrences;
            ensure!(
                occurrences == 0,
                "Korean output {output_id} uses forbidden terminology {forbidden:?}; use {:?} from decision {}",
                decision.approved_korean,
                decision.decision_id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::graphics_translation::load_graphics_translation_set;
    use crate::source::read_json_file;
    use crate::story_dialogue_translation::load_story_dialogue_translation_set;

    fn skeleton_t_terminology() -> ApprovedTerminology {
        ApprovedTerminology {
            schema: 1,
            decisions: vec![ApprovedTermDecision {
                decision_id: "character-name-skeleton-t".to_string(),
                source_referents: vec!["スケルトンＴ".to_string()],
                approved_korean: "스켈톤T".to_string(),
                applicability: "character name".to_string(),
                evidence: "approved Korean series term".to_string(),
                transfer_limit: "original English pixels stay unchanged".to_string(),
                state: ApprovedTermState::Approved,
                forbidden_korean_variants: vec!["스켈레톤T".to_string(), "스켈톤 T".to_string()],
            }],
        }
    }

    #[test]
    fn approved_character_name_is_counted() {
        let mut validation = ApprovedTerminologyValidation {
            approved_decisions: 1,
            checked_korean_outputs: 0,
            approved_term_occurrences: 0,
            forbidden_variant_occurrences: 0,
        };
        validate_terminology_decisions(&skeleton_t_terminology()).unwrap();
        validate_korean_output(
            &skeleton_t_terminology(),
            "story/test",
            "스켈톤T가 차를 마신다.",
            &mut validation,
        )
        .unwrap();
        assert_eq!(validation.checked_korean_outputs, 1);
        assert_eq!(validation.approved_term_occurrences, 1);
        assert_eq!(validation.forbidden_variant_occurrences, 0);
    }

    #[test]
    fn forbidden_character_name_variant_fails() {
        let mut validation = ApprovedTerminologyValidation {
            approved_decisions: 1,
            checked_korean_outputs: 0,
            approved_term_occurrences: 0,
            forbidden_variant_occurrences: 0,
        };
        let error = validate_korean_output(
            &skeleton_t_terminology(),
            "story/test",
            "스켈레톤T가 차를 마신다.",
            &mut validation,
        )
        .unwrap_err();
        assert!(error.to_string().contains("use \"스켈톤T\""));
    }

    #[test]
    #[ignore = "requires assets/translation/"]
    fn tracked_translation_outputs_follow_approved_terms() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let terminology: ApprovedTerminology =
            read_json_file(&root.join("assets/translation/approved-terminology.json")).unwrap();
        let memory_card: MemoryCardTranslationBundle =
            read_json_file(&root.join("assets/translation/memory-card-messages.ko.json")).unwrap();
        let executable_text: ExecutableTextTranslationBundle =
            read_json_file(&root.join("assets/translation/executable-text.ko.json")).unwrap();
        let graphics = load_graphics_translation_set(
            &root.join("assets/translation/graphics-text/manifest.json"),
        )
        .unwrap();
        let story = load_story_dialogue_translation_set(
            &root.join("assets/translation/story-dialogue/manifest.json"),
        )
        .unwrap();

        let validation = validate_approved_terminology(
            &terminology,
            &memory_card,
            &executable_text,
            &graphics,
            &story,
        )
        .unwrap();
        assert!(validation.approved_decisions > 0);
        assert!(validation.checked_korean_outputs > 0);
        assert_eq!(validation.forbidden_variant_occurrences, 0);
    }
}
