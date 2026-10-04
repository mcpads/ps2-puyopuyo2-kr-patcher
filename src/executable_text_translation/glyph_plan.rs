use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::font_atlas::FontGlyphSet;
use crate::font_atlas::nazo::NazoFontAtlasReport;
use crate::font_atlas::tutorial::TutorialFontAtlasReport;

use super::{
    ExecutableTextGlyphAssignment, ExecutableTextGlyphPlan, ExecutableTextHalfwidthGlyphPlan,
    ExecutableTextTranslationBundle, ExecutableTextTranslationValidation,
};

const CUSTOM_CODE_LEAD: u8 = 0xf0;
const CUSTOM_CODE_START: u8 = 0x40;

pub struct ExecutableTextGlyphPlanningInputs<'a> {
    pub bundle: &'a ExecutableTextTranslationBundle,
    pub validation: &'a ExecutableTextTranslationValidation,
    pub tutorial_font: &'a TutorialFontAtlasReport,
    pub puzzle_puyo_font: &'a NazoFontAtlasReport,
}

pub fn plan_executable_text_glyphs(
    inputs: ExecutableTextGlyphPlanningInputs<'_>,
) -> Result<Vec<ExecutableTextGlyphPlan>> {
    validate_report_source(
        inputs.bundle,
        &inputs.tutorial_font.source_bin_sha256,
        &inputs.tutorial_font.executable_iso_path,
        &inputs.tutorial_font.executable_sha256,
        "tutorial font",
    )?;
    validate_report_source(
        inputs.bundle,
        &inputs.puzzle_puyo_font.source_bin_sha256,
        &inputs.puzzle_puyo_font.executable_iso_path,
        &inputs.puzzle_puyo_font.executable_sha256,
        "Puzzle Puyo font",
    )?;
    ensure!(
        inputs.tutorial_font.fullwidth.glyphs.len() == inputs.tutorial_font.lookup.slots.slots,
        "tutorial font glyph population differs from its lookup table"
    );
    ensure!(
        inputs.puzzle_puyo_font.fullwidth.glyphs.len()
            == inputs.puzzle_puyo_font.lookup.fullwidth_slots.slots,
        "Puzzle Puyo font glyph population differs from its lookup table"
    );

    Ok(vec![
        plan_puzzle_puyo_text_glyphs(inputs.bundle, inputs.validation, inputs.puzzle_puyo_font)?,
        plan_tutorial_text_glyphs(inputs.bundle, inputs.validation, inputs.tutorial_font)?,
    ])
}

pub fn plan_puzzle_puyo_text_glyphs(
    bundle: &ExecutableTextTranslationBundle,
    validation: &ExecutableTextTranslationValidation,
    puzzle_puyo_font: &NazoFontAtlasReport,
) -> Result<ExecutableTextGlyphPlan> {
    validate_report_source(
        bundle,
        &puzzle_puyo_font.source_bin_sha256,
        &puzzle_puyo_font.executable_iso_path,
        &puzzle_puyo_font.executable_sha256,
        "Puzzle Puyo font",
    )?;
    ensure!(
        puzzle_puyo_font.fullwidth.glyphs.len() == puzzle_puyo_font.lookup.fullwidth_slots.slots,
        "Puzzle Puyo font glyph population differs from its lookup table"
    );
    let demand = validation
        .consumer_demands
        .iter()
        .find(|item| item.consumer_family == "puzzle_puyo")
        .context("validated translation demand has no Puzzle Puyo family")?;
    let hangul = unique_family_hangul(bundle, "puzzle_puyo");
    ensure!(
        hangul.len() == demand.required_unique_hangul_glyphs,
        "Puzzle Puyo glyph demand differs between validation and allocation"
    );
    plan_family_glyphs(
        "puzzle_puyo",
        &puzzle_puyo_font.fullwidth,
        puzzle_puyo_font.lookup.missing_fullwidth_fallback_index,
        &hangul,
    )
}

pub fn plan_puzzle_puyo_halfwidth_glyphs(
    bundle: &ExecutableTextTranslationBundle,
    puzzle_puyo_font: &NazoFontAtlasReport,
) -> Result<ExecutableTextHalfwidthGlyphPlan> {
    validate_report_source(
        bundle,
        &puzzle_puyo_font.source_bin_sha256,
        &puzzle_puyo_font.executable_iso_path,
        &puzzle_puyo_font.executable_sha256,
        "Puzzle Puyo font",
    )?;
    ensure!(
        puzzle_puyo_font.halfwidth.glyphs.len() == puzzle_puyo_font.lookup.halfwidth_slots.slots,
        "Puzzle Puyo halfwidth glyph population differs from its lookup table"
    );
    let demand = unique_family_ascii(bundle, "puzzle_puyo");
    let shadowed_space_slots = puzzle_puyo_font
        .lookup
        .halfwidth_slots
        .duplicates
        .iter()
        .filter(|duplicate| duplicate.source_hex == "20")
        .flat_map(|duplicate| duplicate.shadowed_slots.iter().copied())
        .collect::<Vec<_>>();
    plan_missing_halfwidth_glyphs(
        "puzzle_puyo",
        &puzzle_puyo_font.halfwidth,
        &shadowed_space_slots,
        &demand,
    )
}

pub fn plan_tutorial_text_glyphs(
    bundle: &ExecutableTextTranslationBundle,
    validation: &ExecutableTextTranslationValidation,
    tutorial_font: &TutorialFontAtlasReport,
) -> Result<ExecutableTextGlyphPlan> {
    validate_report_source(
        bundle,
        &tutorial_font.source_bin_sha256,
        &tutorial_font.executable_iso_path,
        &tutorial_font.executable_sha256,
        "tutorial font",
    )?;
    ensure!(
        tutorial_font.fullwidth.glyphs.len() == tutorial_font.lookup.slots.slots,
        "tutorial font glyph population differs from its lookup table"
    );
    let demand = validation
        .consumer_demands
        .iter()
        .find(|item| item.consumer_family == "tutorial")
        .context("validated translation demand has no tutorial family")?;
    let hangul = unique_family_hangul(bundle, "tutorial");
    ensure!(
        hangul.len() == demand.required_unique_hangul_glyphs,
        "tutorial glyph demand differs between validation and allocation"
    );
    plan_family_glyphs(
        "tutorial",
        &tutorial_font.fullwidth,
        tutorial_font.lookup.missing_fallback_index,
        &hangul,
    )
}

fn validate_report_source(
    bundle: &ExecutableTextTranslationBundle,
    source_bin_sha256: &str,
    executable_iso_path: &str,
    executable_sha256: &str,
    label: &str,
) -> Result<()> {
    ensure!(
        bundle.source.source_bin_sha256 == source_bin_sha256
            && bundle.source.executable_iso_path == executable_iso_path
            && bundle.source.executable_sha256 == executable_sha256,
        "{label} does not share the translation bundle source identity"
    );
    Ok(())
}

fn unique_family_hangul(
    bundle: &ExecutableTextTranslationBundle,
    consumer_family: &str,
) -> Vec<char> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for character in bundle
        .entries
        .iter()
        .filter(|entry| entry.source.consumer_family == consumer_family)
        .flat_map(|entry| entry.translation.text.chars())
    {
        if matches!(character, '\u{ac00}'..='\u{d7a3}') && seen.insert(character) {
            output.push(character);
        }
    }
    output
}

fn unique_family_ascii(
    bundle: &ExecutableTextTranslationBundle,
    consumer_family: &str,
) -> Vec<char> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for character in bundle
        .entries
        .iter()
        .filter(|entry| entry.source.consumer_family == consumer_family)
        .flat_map(|entry| entry.translation.text.chars())
    {
        if character.is_ascii() && character != '\n' && seen.insert(character) {
            output.push(character);
        }
    }
    output
}

fn plan_missing_halfwidth_glyphs(
    consumer_family: &str,
    halfwidth: &FontGlyphSet,
    shadowed_space_slots: &[usize],
    demand: &[char],
) -> Result<ExecutableTextHalfwidthGlyphPlan> {
    let source_supported = halfwidth
        .glyphs
        .iter()
        .filter_map(|glyph| {
            let mut characters = glyph.character.chars();
            let character = characters.next()?;
            (characters.next().is_none() && character.is_ascii()).then_some(character)
        })
        .collect::<BTreeSet<_>>();
    let missing = demand
        .iter()
        .copied()
        .filter(|character| !source_supported.contains(character))
        .collect::<Vec<_>>();
    ensure!(
        missing.len() <= shadowed_space_slots.len(),
        "{consumer_family} needs {} added halfwidth glyphs but only {} shadowed space slots are assignable",
        missing.len(),
        shadowed_space_slots.len()
    );
    let assignments = missing
        .iter()
        .zip(shadowed_space_slots)
        .map(|(character, slot_index)| {
            ensure!(
                *slot_index < halfwidth.glyphs.len()
                    && halfwidth.glyphs[*slot_index].source_hex == "20",
                "{consumer_family} halfwidth assignment does not name a shadowed space slot"
            );
            Ok(ExecutableTextGlyphAssignment {
                character: character.to_string(),
                code_hex: format!("{:02x}", *character as u8),
                slot_index: *slot_index,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ExecutableTextHalfwidthGlyphPlan {
        consumer_family: consumer_family.to_string(),
        font_table_symbol: halfwidth.symbol.clone(),
        physical_slots: halfwidth.glyphs.len(),
        source_supported_glyphs: source_supported.len(),
        assignable_shadowed_space_slots: shadowed_space_slots.len(),
        required_added_glyphs: assignments.len(),
        remaining_shadowed_space_slots: shadowed_space_slots.len() - assignments.len(),
        assignments,
    })
}

fn plan_family_glyphs(
    consumer_family: &str,
    fullwidth: &FontGlyphSet,
    fallback_slot: usize,
    hangul: &[char],
) -> Result<ExecutableTextGlyphPlan> {
    let physical_slots = fullwidth.glyphs.len();
    ensure!(
        fallback_slot < physical_slots,
        "{consumer_family} fallback slot is outside its fullwidth atlas"
    );
    let candidate_slots = fullwidth
        .glyphs
        .iter()
        .map(|glyph| glyph.index)
        .filter(|slot| *slot != fallback_slot)
        .collect::<Vec<_>>();
    ensure!(
        candidate_slots.len() >= hangul.len(),
        "{consumer_family} needs {} Hangul glyphs but only {} slots remain after reserving fallback slot {fallback_slot}",
        hangul.len(),
        candidate_slots.len()
    );
    ensure!(
        hangul.len() <= usize::from(u8::MAX - CUSTOM_CODE_START) + 1,
        "{consumer_family} exceeds the two-byte development-code range"
    );
    let source_codes = fullwidth
        .glyphs
        .iter()
        .map(|glyph| glyph.source_hex.as_str())
        .collect::<BTreeSet<_>>();
    let assignments = hangul
        .iter()
        .zip(candidate_slots.iter())
        .enumerate()
        .map(|(ordinal, (character, slot_index))| {
            let trail = CUSTOM_CODE_START + u8::try_from(ordinal)?;
            let code_hex = format!("{CUSTOM_CODE_LEAD:02x}{trail:02x}");
            ensure!(
                !source_codes.contains(code_hex.as_str()),
                "{consumer_family} custom code collides with the source font table: {code_hex}"
            );
            Ok(ExecutableTextGlyphAssignment {
                character: character.to_string(),
                code_hex,
                slot_index: *slot_index,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let assignable_slots = candidate_slots.len();
    Ok(ExecutableTextGlyphPlan {
        consumer_family: consumer_family.to_string(),
        font_table_symbol: fullwidth.symbol.clone(),
        physical_slots,
        reserved_fallback_slot: fallback_slot,
        assignable_slots,
        required_unique_hangul_glyphs: assignments.len(),
        remaining_slots: assignable_slots - assignments.len(),
        assignments,
    })
}

#[cfg(test)]
#[path = "glyph_plan_tests.rs"]
mod glyph_plan_tests;
