use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use encoding_rs::SHIFT_JIS;
use r5900::{GeneralRegister, InstructionForm, Operands};
use serde::Serialize;

use crate::cpx::read_symbol_bytes;
use crate::executable::{SymbolDisassembly, disassemble_symbol, validate_r5900_instruction};
use crate::executable_font_consumers::ExecutableFontConsumerReport;
use crate::font_atlas::{FontAtlasReport, FontGlyph, GlyphRectangle};
use crate::indexed_texture::sha256;
use crate::message_corpus::{MessageCorpusReport, MessageEntry};

const LOOKUP_SYMBOL: &str = "GetFontParam";
const FULLWIDTH_TABLE_SYMBOL: &str = "FontList_Zen";

#[derive(Clone, Debug, Serialize)]
pub struct GlyphAllocationReport {
    pub schema: u32,
    pub source_bin_sha256: String,
    pub executable_iso_path: String,
    pub executable_sha256: String,
    pub font_atlas_png_sha256: String,
    pub scope: GlyphAllocationScope,
    pub lookup: FullwidthLookupEvidence,
    pub population: GlyphAllocationPopulation,
    pub slots: Vec<GlyphSlotPlan>,
    pub hangul: HangulAllocationStatus,
}

#[derive(Clone, Debug, Serialize)]
pub struct GlyphAllocationScope {
    pub consumer: String,
    pub target: &'static str,
    pub preserve: &'static str,
    pub candidate_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct FullwidthLookupEvidence {
    pub symbol: &'static str,
    pub table_symbol: &'static str,
    pub symbol_sha256: String,
    pub table_sha256: String,
    pub behavior: &'static str,
    pub disassembly: SymbolDisassembly,
}

#[derive(Clone, Debug, Serialize)]
pub struct GlyphAllocationPopulation {
    pub fullwidth_slots: usize,
    pub unique_source_codes: usize,
    pub declared_corpus_character_occurrences: usize,
    pub declared_corpus_unique_characters: usize,
    pub declared_corpus_required_slots: usize,
    pub reachable_slots_unused_by_declared_corpus: usize,
    pub shadowed_duplicate_slots: usize,
    pub proven_reassignable_slots: usize,
    pub unresolved_declared_corpus_characters: usize,
    pub preserved_halfwidth_slots: usize,
    pub protected_english_runs: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct GlyphSlotPlan {
    pub index: usize,
    pub source_hex: String,
    pub source_character: String,
    pub rectangle: GlyphRectangle,
    pub first_match_index: usize,
    pub lookup_reachable: bool,
    pub declared_corpus_occurrences: usize,
    pub affected_message_ids: Vec<String>,
    pub classification: &'static str,
    pub reassignment_policy: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct HangulAllocationStatus {
    pub status: &'static str,
    pub approved_translation_asset: bool,
    pub required_unique_hangul_glyphs: Option<usize>,
    pub assignments: Vec<HangulGlyphAssignment>,
    pub next_gate: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct HangulGlyphAssignment {
    pub character: String,
    pub slot_index: usize,
}

pub struct GlyphAllocationInputs<'a> {
    pub executable: &'a [u8],
    pub font_atlas: &'a FontAtlasReport,
    pub message_corpora: &'a [MessageCorpusReport],
    pub font_consumers: &'a ExecutableFontConsumerReport,
}

#[derive(Clone, Debug, Default)]
struct CorpusGlyphUsage {
    occurrences: usize,
    message_ids: BTreeSet<String>,
}

pub fn plan_glyph_allocation(inputs: GlyphAllocationInputs<'_>) -> Result<GlyphAllocationReport> {
    ensure!(
        !inputs.message_corpora.is_empty(),
        "glyph allocation requires at least one declared message corpus"
    );
    ensure!(
        inputs.font_atlas.translation_scope.target == "Japanese text only",
        "font atlas translation scope no longer targets Japanese text only"
    );
    let mut consumers = BTreeSet::new();
    for corpus in inputs.message_corpora {
        ensure!(
            inputs.font_atlas.source_bin_sha256 == corpus.source_bin_sha256,
            "font atlas and message corpus use different source BINs"
        );
        ensure!(
            inputs.font_atlas.executable_iso_path == corpus.executable_iso_path
                && inputs.font_atlas.executable_sha256 == corpus.executable_sha256,
            "font atlas and message corpus use different executables"
        );
        ensure!(
            corpus
                .translation_scope
                .target
                .contains("non-ASCII Shift-JIS"),
            "message corpus translation scope no longer identifies Japanese Shift-JIS runs"
        );
        validate_corpus_population(corpus)?;
        ensure!(
            consumers.insert(corpus.scope.consumer_symbol),
            "duplicate message consumer corpus: {}",
            corpus.scope.consumer_symbol
        );
    }
    validate_main_font_consumer_coverage(inputs.font_consumers, inputs.font_atlas, &consumers)?;

    let lookup = validate_fullwidth_lookup(inputs.executable)?;
    let declared_messages = unique_corpus_messages(inputs.message_corpora)?;
    let usage = collect_corpus_usage(&declared_messages)?;
    let slots = classify_fullwidth_slots(&inputs.font_atlas.fullwidth.glyphs, &usage)?;
    let declared_corpus_character_occurrences =
        usage.values().map(|entry| entry.occurrences).sum::<usize>();
    let protected_english_runs = declared_messages
        .iter()
        .map(|message| message.protected_english_runs.len())
        .sum();

    let unique_source_codes = inputs
        .font_atlas
        .fullwidth
        .glyphs
        .iter()
        .map(|glyph| glyph.source_hex.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let declared_corpus_required_slots = slots
        .iter()
        .filter(|slot| slot.classification == "declared-corpus-source-required")
        .count();
    let reachable_slots_unused_by_declared_corpus = slots
        .iter()
        .filter(|slot| slot.lookup_reachable && slot.declared_corpus_occurrences == 0)
        .count();
    let shadowed_duplicate_slots = slots
        .iter()
        .filter(|slot| slot.classification == "shadowed-duplicate-candidate")
        .count();
    let consumer_census_unused_slots = slots
        .iter()
        .filter(|slot| slot.classification == "consumer-census-unused-candidate")
        .count();
    let proven_reassignable_slots = shadowed_duplicate_slots
        .checked_add(consumer_census_unused_slots)
        .context("glyph candidate slot count overflow")?;

    Ok(GlyphAllocationReport {
        schema: 2,
        source_bin_sha256: inputs.font_atlas.source_bin_sha256.clone(),
        executable_iso_path: inputs.font_atlas.executable_iso_path.clone(),
        executable_sha256: inputs.font_atlas.executable_sha256.clone(),
        font_atlas_png_sha256: inputs.font_atlas.atlas.png_sha256.clone(),
        scope: GlyphAllocationScope {
            consumer: consumers.into_iter().collect::<Vec<_>>().join(" + "),
            target: "fullwidth slots reached by Japanese Shift-JIS tokens in the declared memory-card message corpora",
            preserve: "all halfwidth ASCII slots, protected English runs, and runtime controls",
            candidate_boundary: "shadowed duplicate slots do not displace a reachable source code; reachable slots unused by the declared memory-card consumers are admitted only after the exact executable font-consumer census closes every MsgPrint_Act producer",
        },
        lookup,
        population: GlyphAllocationPopulation {
            fullwidth_slots: slots.len(),
            unique_source_codes,
            declared_corpus_character_occurrences,
            declared_corpus_unique_characters: usage.len(),
            declared_corpus_required_slots,
            reachable_slots_unused_by_declared_corpus,
            shadowed_duplicate_slots,
            proven_reassignable_slots,
            unresolved_declared_corpus_characters: 0,
            preserved_halfwidth_slots: inputs.font_atlas.halfwidth.glyphs.len(),
            protected_english_runs,
        },
        slots,
        hangul: HangulAllocationStatus {
            status: "capacity-only; no automatic character assignment",
            approved_translation_asset: false,
            required_unique_hangul_glyphs: None,
            assignments: Vec::new(),
            next_gate: "approve a bounded Korean translation set, derive its unique Hangul glyph requirement, and admit assignments only within proven capacity",
        },
    })
}

fn validate_main_font_consumer_coverage(
    report: &ExecutableFontConsumerReport,
    font_atlas: &FontAtlasReport,
    declared_consumers: &BTreeSet<&str>,
) -> Result<()> {
    ensure!(
        report.source_bin_sha256 == font_atlas.source_bin_sha256
            && report.executable_iso_path == font_atlas.executable_iso_path
            && report.executable_sha256 == font_atlas.executable_sha256,
        "font atlas and executable font-consumer census use different exact sources"
    );
    let main_families = report
        .families
        .iter()
        .filter(|family| family.family == "main_message")
        .collect::<Vec<_>>();
    ensure!(
        main_families.len() == 1,
        "executable font-consumer census must contain exactly one main_message family"
    );
    let main = main_families[0];
    let covered_consumers = main
        .renderer_producer_counts
        .iter()
        .filter(|producer| producer.declared_actor_creations > 0)
        .map(|producer| producer.producer_symbol.as_str())
        .collect::<BTreeSet<_>>();
    require_closed_main_font_population(
        report.summary.unresolved_font_consumer_paths,
        main.renderer.symbol,
        main.lookup.symbol,
        main.font_table.symbol,
        &covered_consumers,
        declared_consumers,
    )
}

fn require_closed_main_font_population(
    unresolved_paths: usize,
    renderer_symbol: &str,
    lookup_symbol: &str,
    font_table_symbol: &str,
    covered_consumers: &BTreeSet<&str>,
    declared_consumers: &BTreeSet<&str>,
) -> Result<()> {
    ensure!(
        unresolved_paths == 0,
        "executable font-consumer census has unresolved paths"
    );
    ensure!(
        renderer_symbol == "MsgPrint_Act"
            && lookup_symbol == LOOKUP_SYMBOL
            && font_table_symbol == FULLWIDTH_TABLE_SYMBOL,
        "main message font-consumer symbols changed"
    );
    ensure!(
        covered_consumers == declared_consumers,
        "executable font-consumer census does not cover the declared memory-card consumers"
    );
    Ok(())
}

fn validate_fullwidth_lookup(executable: &[u8]) -> Result<FullwidthLookupEvidence> {
    let file = object::File::parse(executable).context("failed to parse executable")?;
    let (lookup_address, lookup_bytes) = read_symbol_bytes(&file, LOOKUP_SYMBOL)?;
    let (table_address, table_bytes) = read_symbol_bytes(&file, FULLWIDTH_TABLE_SYMBOL)?;
    let disassembly = disassemble_symbol(executable, LOOKUP_SYMBOL)?;

    let upper = decode_at(&lookup_bytes, lookup_address, 0xa0)?;
    let lower = decode_at(&lookup_bytes, lookup_address, 0xa4)?;
    let upper_immediate = match upper.operands() {
        Operands::LoadUpper {
            destination: GeneralRegister::A2,
            immediate,
        } => *immediate,
        _ => anyhow::bail!("fullwidth table upper address instruction changed"),
    };
    let lower_immediate = match lower.operands() {
        Operands::SignedImmediate {
            destination: GeneralRegister::A2,
            source: GeneralRegister::A2,
            immediate,
        } => *immediate,
        _ => anyhow::bail!("fullwidth table lower address instruction changed"),
    };
    ensure!(
        upper.form() == InstructionForm::Lui && lower.form() == InstructionForm::Addiu,
        "fullwidth table address is not materialized with lui/addiu"
    );
    let resolved_table =
        (u32::from(upper_immediate) << 16).wrapping_add_signed(i32::from(lower_immediate));
    ensure!(
        u64::from(resolved_table) == table_address,
        "GetFontParam no longer searches FontList_Zen"
    );

    require_memory_instruction(
        &lookup_bytes,
        lookup_address,
        0xa8,
        InstructionForm::Lb,
        GeneralRegister::V0,
        GeneralRegister::A2,
        0,
        "fullwidth first-byte load",
    )?;
    require_branch(
        &lookup_bytes,
        lookup_address,
        0xac,
        GeneralRegister::V0,
        GeneralRegister::A3,
        lookup_address + 0xd0,
        "fullwidth first-byte mismatch",
    )?;
    require_memory_instruction(
        &lookup_bytes,
        lookup_address,
        0xb4,
        InstructionForm::Lb,
        GeneralRegister::V1,
        GeneralRegister::A2,
        1,
        "fullwidth second-byte table load",
    )?;
    require_memory_instruction(
        &lookup_bytes,
        lookup_address,
        0xb8,
        InstructionForm::Lb,
        GeneralRegister::V0,
        GeneralRegister::A0,
        1,
        "fullwidth second-byte input load",
    )?;
    require_branch(
        &lookup_bytes,
        lookup_address,
        0xbc,
        GeneralRegister::V1,
        GeneralRegister::V0,
        lookup_address + 0xd0,
        "fullwidth second-byte mismatch",
    )?;
    let index = decode_at(&lookup_bytes, lookup_address, 0xc0)?;
    ensure!(
        matches!(
            index.operands(),
            Operands::ShiftImmediate {
                destination: GeneralRegister::V1,
                source: GeneralRegister::T0,
                amount,
            } if amount.value() == 1
        ) && index.form() == InstructionForm::Srl,
        "fullwidth match no longer converts byte offset to slot index"
    );
    require_memory_instruction(
        &lookup_bytes,
        lookup_address,
        0xcc,
        InstructionForm::Sw,
        GeneralRegister::V1,
        GeneralRegister::A1,
        0,
        "fullwidth matched-index store",
    )?;
    let advance = decode_at(&lookup_bytes, lookup_address, 0xd0)?;
    ensure!(
        matches!(
            advance.operands(),
            Operands::SignedImmediate {
                destination: GeneralRegister::A2,
                source: GeneralRegister::A2,
                immediate: 2,
            }
        ) && advance.form() == InstructionForm::Addiu,
        "fullwidth lookup no longer advances by one two-byte entry"
    );
    require_memory_instruction(
        &lookup_bytes,
        lookup_address,
        0xd4,
        InstructionForm::Lb,
        GeneralRegister::V0,
        GeneralRegister::A2,
        0,
        "fullwidth terminator load",
    )?;
    require_branch(
        &lookup_bytes,
        lookup_address,
        0xd8,
        GeneralRegister::V0,
        GeneralRegister::ZERO,
        lookup_address + 0xa8,
        "fullwidth lookup loop",
    )?;
    let byte_offset = decode_at(&lookup_bytes, lookup_address, 0xdc)?;
    ensure!(
        matches!(
            byte_offset.operands(),
            Operands::SignedImmediate {
                destination: GeneralRegister::T0,
                source: GeneralRegister::T0,
                immediate: 2,
            }
        ) && byte_offset.form() == InstructionForm::Addiu,
        "fullwidth lookup offset no longer advances by two bytes"
    );

    Ok(FullwidthLookupEvidence {
        symbol: LOOKUP_SYMBOL,
        table_symbol: FULLWIDTH_TABLE_SYMBOL,
        symbol_sha256: sha256(&lookup_bytes),
        table_sha256: sha256(&table_bytes),
        behavior: "linear two-byte scan; the first exact match returns its slot index and a zero lead byte terminates the table",
        disassembly,
    })
}

fn decode_at(source: &[u8], start_address: u64, offset: usize) -> Result<r5900::Instruction> {
    let bytes = source
        .get(offset..offset + 4)
        .context("GetFontParam instruction offset is out of range")?;
    let address = start_address + u64::try_from(offset)?;
    validate_r5900_instruction(bytes, address)
        .with_context(|| format!("failed to validate GetFontParam at {address:#x}"))
}

#[allow(clippy::too_many_arguments)]
fn require_memory_instruction(
    source: &[u8],
    start_address: u64,
    offset: usize,
    form: InstructionForm,
    register: GeneralRegister,
    base: GeneralRegister,
    memory_offset: i16,
    role: &str,
) -> Result<()> {
    let instruction = decode_at(source, start_address, offset)?;
    ensure!(instruction.form() == form, "{role} instruction changed");
    ensure!(
        matches!(
            instruction.operands(),
            Operands::MemoryGeneral {
                register: actual_register,
                base: actual_base,
                offset: actual_offset,
            } if *actual_register == register && *actual_base == base && *actual_offset == memory_offset
        ),
        "{role} operands changed"
    );
    Ok(())
}

fn require_branch(
    source: &[u8],
    start_address: u64,
    offset: usize,
    left: GeneralRegister,
    right: GeneralRegister,
    target: u64,
    role: &str,
) -> Result<()> {
    let instruction = decode_at(source, start_address, offset)?;
    ensure!(
        instruction.form() == InstructionForm::Bne,
        "{role} is no longer bne"
    );
    ensure!(
        matches!(
            instruction.operands(),
            Operands::BranchTwo {
                left: actual_left,
                right: actual_right,
                target: actual_target,
            } if *actual_left == left && *actual_right == right && u64::from(*actual_target) == target
        ),
        "{role} operands changed"
    );
    Ok(())
}

fn validate_corpus_population(corpus: &MessageCorpusReport) -> Result<()> {
    let japanese_characters = corpus
        .messages
        .iter()
        .flat_map(|message| &message.tokens)
        .filter(|token| token.kind == "shift-jis-text")
        .map(|token| token.value.chars().count())
        .sum::<usize>();
    let protected_english_runs = corpus
        .messages
        .iter()
        .map(|message| message.protected_english_runs.len())
        .sum::<usize>();
    ensure!(
        japanese_characters == corpus.population.japanese_characters,
        "{} Japanese character population changed",
        corpus.scope.consumer_symbol
    );
    ensure!(
        protected_english_runs == corpus.population.protected_english_runs,
        "{} protected English population changed",
        corpus.scope.consumer_symbol
    );
    Ok(())
}

fn unique_corpus_messages(corpora: &[MessageCorpusReport]) -> Result<Vec<&MessageEntry>> {
    unique_messages_by_source(corpora.iter().flat_map(|corpus| &corpus.messages))
}

fn unique_messages_by_source<'a>(
    entries: impl IntoIterator<Item = &'a MessageEntry>,
) -> Result<Vec<&'a MessageEntry>> {
    let mut messages_by_id = BTreeMap::<&str, &MessageEntry>::new();
    let mut messages_by_address = BTreeMap::<&str, &MessageEntry>::new();
    for message in entries {
        if let Some(existing) = messages_by_id.get(message.message_id.as_str()) {
            ensure!(
                existing.symbol == message.symbol
                    && existing.source_address == message.source_address
                    && existing.source_sha256 == message.source_sha256
                    && existing.source_hex == message.source_hex,
                "shared message {} differs between consumer corpora",
                message.message_id
            );
        } else {
            messages_by_id.insert(&message.message_id, message);
        }
        if let Some(existing) = messages_by_address.get(message.source_address.as_str()) {
            ensure!(
                existing.message_id == message.message_id
                    && existing.symbol == message.symbol
                    && existing.source_sha256 == message.source_sha256
                    && existing.source_hex == message.source_hex,
                "shared source {} has inconsistent message identity",
                message.source_address
            );
        } else {
            messages_by_address.insert(&message.source_address, message);
        }
    }
    Ok(messages_by_address.into_values().collect())
}

fn collect_corpus_usage(messages: &[&MessageEntry]) -> Result<BTreeMap<String, CorpusGlyphUsage>> {
    let mut usage = BTreeMap::<String, CorpusGlyphUsage>::new();
    for message in messages {
        for token in &message.tokens {
            if token.kind != "shift-jis-text" {
                continue;
            }
            ensure!(
                token.translation_policy == "translate-japanese",
                "Shift-JIS token is not classified as Japanese translation scope"
            );
            for character in token.value.chars() {
                let source_hex = encode_fullwidth_character(character)?;
                let entry = usage.entry(source_hex).or_default();
                entry.occurrences += 1;
                entry.message_ids.insert(message.message_id.clone());
            }
        }
    }
    Ok(usage)
}

fn encode_fullwidth_character(character: char) -> Result<String> {
    let source = character.to_string();
    let (encoded, _, had_errors) = SHIFT_JIS.encode(&source);
    ensure!(
        !had_errors,
        "Japanese corpus character is not Shift-JIS: {character}"
    );
    ensure!(
        encoded.len() == 2,
        "Japanese corpus character is not a fullwidth two-byte code: {character}"
    );
    Ok(encoded.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn classify_fullwidth_slots(
    glyphs: &[FontGlyph],
    usage: &BTreeMap<String, CorpusGlyphUsage>,
) -> Result<Vec<GlyphSlotPlan>> {
    let mut first_match = BTreeMap::<&str, usize>::new();
    for glyph in glyphs {
        first_match
            .entry(glyph.source_hex.as_str())
            .or_insert(glyph.index);
    }
    for source_hex in usage.keys() {
        ensure!(
            first_match.contains_key(source_hex.as_str()),
            "declared Japanese corpus character is absent from FontList_Zen: {source_hex}"
        );
    }

    glyphs
        .iter()
        .map(|glyph| {
            let first_match_index = *first_match
                .get(glyph.source_hex.as_str())
                .context("font glyph has no first-match entry")?;
            let lookup_reachable = glyph.index == first_match_index;
            let glyph_usage = if lookup_reachable {
                usage.get(&glyph.source_hex)
            } else {
                None
            };
            let (classification, reassignment_policy) = if !lookup_reachable {
                (
                    "shadowed-duplicate-candidate",
                    "candidate for a new fullwidth code and glyph without displacing any source-code first match; assignment still requires an approved Korean glyph set and Expected Write",
                )
            } else if glyph_usage.is_some_and(|entry| entry.occurrences > 0) {
                (
                    "declared-corpus-source-required",
                    "preserve until every affected Japanese message has an approved replacement and its Korean glyph demand is admitted",
                )
            } else {
                (
                    "consumer-census-unused-candidate",
                    "candidate for a new fullwidth code and glyph because the exact executable census closes every MsgPrint_Act producer and no declared memory-card source reaches this slot; assignment still requires an approved Korean glyph set and Expected Write",
                )
            };
            Ok(GlyphSlotPlan {
                index: glyph.index,
                source_hex: glyph.source_hex.clone(),
                source_character: glyph.character.clone(),
                rectangle: glyph.rectangle,
                first_match_index,
                lookup_reachable,
                declared_corpus_occurrences: glyph_usage.map_or(0, |entry| entry.occurrences),
                affected_message_ids: glyph_usage
                    .map(|entry| entry.message_ids.iter().cloned().collect())
                    .unwrap_or_default(),
                classification,
                reassignment_policy,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_corpus::{MessageToken, ProtectedEnglishRun};

    #[test]
    fn duplicate_code_is_shadowed_by_the_first_matching_slot() {
        let glyphs = vec![
            glyph(0, "8140", "　"),
            glyph(1, "82a0", "あ"),
            glyph(2, "8140", "　"),
        ];
        let mut usage = BTreeMap::new();
        usage.insert(
            "8140".to_string(),
            CorpusGlyphUsage {
                occurrences: 1,
                message_ids: BTreeSet::from(["message/space".to_string()]),
            },
        );

        let slots = classify_fullwidth_slots(&glyphs, &usage).unwrap();

        assert_eq!(slots[0].classification, "declared-corpus-source-required");
        assert_eq!(slots[0].declared_corpus_occurrences, 1);
        assert_eq!(slots[1].classification, "consumer-census-unused-candidate");
        assert!(slots[1].lookup_reachable);
        assert_eq!(slots[2].classification, "shadowed-duplicate-candidate");
        assert!(!slots[2].lookup_reachable);
        assert_eq!(slots[2].first_match_index, 0);
        assert_eq!(slots[2].declared_corpus_occurrences, 0);
        assert!(slots[2].affected_message_ids.is_empty());
    }

    #[test]
    fn corpus_usage_excludes_ascii_and_runtime_controls() {
        let message = message(vec![
            token("82a0", "shift-jis-text", "あ", "translate-japanese"),
            token(
                "4d454d4f5259",
                "ascii-text",
                "MEMORY",
                "preserve-ascii-in-japanese-only-scope",
            ),
            token(
                "4050",
                "dynamic-port",
                "current memory-card port glyph",
                "preserve-runtime-control",
            ),
        ]);

        let usage = collect_corpus_usage(&[&message]).unwrap();

        assert_eq!(usage.len(), 1);
        assert_eq!(usage["82a0"].occurrences, 1);
    }

    #[test]
    fn shared_messages_are_counted_once_and_must_match() {
        let first = message(vec![token(
            "82a0",
            "shift-jis-text",
            "あ",
            "translate-japanese",
        )]);
        let duplicate = first.clone();

        let messages = unique_messages_by_source([&first, &duplicate]).unwrap();

        assert_eq!(messages.len(), 1);

        let mut changed = duplicate;
        changed.source_hex = "82a200".to_string();
        let error = unique_messages_by_source([&first, &changed])
            .unwrap_err()
            .to_string();
        assert!(error.contains("differs between consumer corpora"));

        let mut renamed = first.clone();
        renamed.message_id = "message/renamed".to_string();
        let error = unique_messages_by_source([&first, &renamed])
            .unwrap_err()
            .to_string();
        assert!(error.contains("inconsistent message identity"));
    }

    #[test]
    fn missing_declared_character_fails_closed() {
        let glyphs = vec![glyph(0, "8140", "　")];
        let usage = BTreeMap::from([(
            "82a0".to_string(),
            CorpusGlyphUsage {
                occurrences: 1,
                message_ids: BTreeSet::from(["message/a".to_string()]),
            },
        )]);

        let error = classify_fullwidth_slots(&glyphs, &usage)
            .unwrap_err()
            .to_string();

        assert!(error.contains("absent from FontList_Zen"));
    }

    #[test]
    fn closed_main_font_population_covers_all_declared_consumers() {
        let consumers = BTreeSet::from(["MemLoad_Act", "MemSave_Act", "MyNazo_ErrMsg"]);

        require_closed_main_font_population(
            0,
            "MsgPrint_Act",
            "GetFontParam",
            "FontList_Zen",
            &consumers,
            &consumers,
        )
        .unwrap();
    }

    #[test]
    fn open_or_incomplete_main_font_population_is_rejected() {
        let declared = BTreeSet::from(["MemLoad_Act", "MemSave_Act", "MyNazo_ErrMsg"]);
        let incomplete = BTreeSet::from(["MemLoad_Act", "MemSave_Act"]);

        assert!(
            require_closed_main_font_population(
                1,
                "MsgPrint_Act",
                "GetFontParam",
                "FontList_Zen",
                &declared,
                &declared,
            )
            .is_err()
        );
        assert!(
            require_closed_main_font_population(
                0,
                "MsgPrint_Act",
                "GetFontParam",
                "FontList_Zen",
                &incomplete,
                &declared,
            )
            .is_err()
        );
    }

    fn glyph(index: usize, source_hex: &str, character: &str) -> FontGlyph {
        FontGlyph {
            index,
            source_hex: source_hex.to_string(),
            character: character.to_string(),
            rectangle: GlyphRectangle {
                x: 0,
                y: 0,
                width: 24,
                height: 24,
            },
        }
    }

    fn message(tokens: Vec<MessageToken>) -> MessageEntry {
        MessageEntry {
            message_id: "message/test".to_string(),
            symbol: "MessageTest".to_string(),
            source_address: "0x00000000".to_string(),
            source_file_offset: "0x00000000".to_string(),
            storage_byte_length: 1,
            payload_byte_length: 0,
            source_sha256: String::new(),
            source_hex: "00".to_string(),
            terminator: "00",
            display_text: String::new(),
            tokens,
            protected_english_runs: Vec::<ProtectedEnglishRun>::new(),
            reference_ids: Vec::new(),
            roundtrip: "exact source bytes, including final NUL",
        }
    }

    fn token(
        raw_hex: &str,
        kind: &'static str,
        value: &str,
        translation_policy: &'static str,
    ) -> MessageToken {
        MessageToken {
            byte_offset: 0,
            byte_length: raw_hex.len() / 2,
            raw_hex: raw_hex.to_string(),
            kind,
            value: value.to_string(),
            translation_policy,
        }
    }
}
