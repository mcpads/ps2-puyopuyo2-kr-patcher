use std::path::PathBuf;

use anyhow::{Context, Result};
use puyo2_ps2_tool::executable_text_translation::{
    ExecutableTextGlyphPlanningInputs, ExecutableTextTranslationBundle,
    ExecutableTextTranslationInputs, compose_executable_text_translation_bundle,
    plan_executable_text_glyphs, plan_puzzle_puyo_halfwidth_glyphs,
    validate_executable_text_translation_bundle,
};
use puyo2_ps2_tool::font_atlas::FontAtlasInputs;
use puyo2_ps2_tool::font_atlas::nazo::render_nazo_font_atlas;
use puyo2_ps2_tool::font_atlas::tutorial::render_tutorial_font_atlas;
use puyo2_ps2_tool::memory_card_metadata::{
    MemoryCardMetadataInputs, MemoryCardMetadataReport, survey_memory_card_metadata,
};
use puyo2_ps2_tool::message_corpus::{
    MessageCorpusInputs, NazoMessageCorpusReport, TutorialMessageCorpusReport,
    survey_nazo_messages, survey_tutorial_messages,
};
use puyo2_ps2_tool::source::{
    DiscInventory, find_inventory_file, read_inventory_file, read_json_file, write_new_json_file,
};

use super::{SourceArgs, inspect};

pub(super) struct ExecutableTextTranslationCommand {
    pub source: SourceArgs,
    pub executable_path: String,
    pub translation: PathBuf,
}

struct FreshExecutableTextCorpora {
    inventory: DiscInventory,
    executable: Vec<u8>,
    source_bin_sha256: String,
    executable_iso_path: String,
    executable_sha256: String,
    tutorial: TutorialMessageCorpusReport,
    puzzle_puyo: NazoMessageCorpusReport,
    memory_card_metadata: MemoryCardMetadataReport,
}

impl FreshExecutableTextCorpora {
    fn inputs(&self) -> ExecutableTextTranslationInputs<'_> {
        ExecutableTextTranslationInputs {
            source_bin_sha256: &self.source_bin_sha256,
            executable_iso_path: &self.executable_iso_path,
            executable_sha256: &self.executable_sha256,
            tutorial: &self.tutorial,
            puzzle_puyo: &self.puzzle_puyo,
            memory_card_metadata: &self.memory_card_metadata,
        }
    }
}

pub(super) fn run_executable_text_translation_init(
    command: ExecutableTextTranslationCommand,
) -> Result<()> {
    let corpora = load_fresh_corpora(&command)?;
    let bundle = compose_executable_text_translation_bundle(corpora.inputs())?;
    write_new_json_file(&command.translation, &bundle)?;
    println!(
        "initialized {} exact-source executable text translations ({} tutorial, {} Puzzle Puyo, {} memory-card metadata) at {}",
        bundle.population.total_entries,
        bundle.population.tutorial_messages,
        bundle.population.puzzle_puyo_messages,
        bundle.population.memory_card_metadata_titles,
        command.translation.display()
    );
    Ok(())
}

pub(super) fn run_executable_text_translation_check(
    command: ExecutableTextTranslationCommand,
) -> Result<()> {
    let corpora = load_fresh_corpora(&command)?;
    let bundle: ExecutableTextTranslationBundle = read_json_file(&command.translation)?;
    let validation = validate_executable_text_translation_bundle(&bundle, corpora.inputs())?;
    let image_d_path = "/IMAGE_D.BIN;1";
    let image_g_path = "/IMAGE_G.BIN;1";
    let image_d = read_inventory_file(&command.source.bin, &corpora.inventory, image_d_path)?;
    let image_g = read_inventory_file(&command.source.bin, &corpora.inventory, image_g_path)?;
    let image_d_sha256 = find_inventory_file(&corpora.inventory, image_d_path)?
        .sha256
        .clone()
        .context("IMAGE_D inventory entry has no SHA-256")?;
    let image_g_sha256 = find_inventory_file(&corpora.inventory, image_g_path)?
        .sha256
        .clone()
        .context("IMAGE_G inventory entry has no SHA-256")?;
    let temporary = tempfile::tempdir().context("failed to create font-planning directory")?;
    let tutorial_font = render_tutorial_font_atlas(
        FontAtlasInputs {
            source_bin_sha256: corpora.source_bin_sha256.clone(),
            executable_iso_path: corpora.executable_iso_path.clone(),
            executable_sha256: corpora.executable_sha256.clone(),
            executable: &corpora.executable,
            image_container_iso_path: image_d_path.to_string(),
            image_container_sha256: image_d_sha256,
            image_container: &image_d,
        },
        &temporary.path().join("tutorial-font.png"),
    )?;
    let puzzle_puyo_font = render_nazo_font_atlas(
        FontAtlasInputs {
            source_bin_sha256: corpora.source_bin_sha256.clone(),
            executable_iso_path: corpora.executable_iso_path.clone(),
            executable_sha256: corpora.executable_sha256.clone(),
            executable: &corpora.executable,
            image_container_iso_path: image_g_path.to_string(),
            image_container_sha256: image_g_sha256,
            image_container: &image_g,
        },
        &temporary.path().join("puzzle-puyo-font.png"),
    )?;
    let glyph_plans = plan_executable_text_glyphs(ExecutableTextGlyphPlanningInputs {
        bundle: &bundle,
        validation: &validation,
        tutorial_font: &tutorial_font,
        puzzle_puyo_font: &puzzle_puyo_font,
    })?;
    let puzzle_puyo_halfwidth = plan_puzzle_puyo_halfwidth_glyphs(&bundle, &puzzle_puyo_font)?;
    let minimum_remaining_storage = validation
        .minimum_remaining_storage_bytes
        .map_or_else(|| "n/a".to_string(), |bytes| bytes.to_string());
    let demand = validation
        .consumer_demands
        .iter()
        .map(|consumer| {
            format!(
                "{}: {} entries/{} Hangul",
                consumer.consumer_family,
                consumer.translated_entries,
                consumer.required_unique_hangul_glyphs
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let capacity = glyph_plans
        .iter()
        .map(|plan| {
            format!(
                "{}: {}/{} assignable, {} remaining (fallback slot {} reserved)",
                plan.consumer_family,
                plan.required_unique_hangul_glyphs,
                plan.assignable_slots,
                plan.remaining_slots,
                plan.reserved_fallback_slot,
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "validated {} executable text translations: {} untranslated, {} in progress, {} awaiting review, {} awaiting human review, {} complete; {} release-candidate eligible; {} fixed-storage checks (closest tail: {} bytes); tutorial shared bank: {}/{} bytes ({} remaining), layout 9x8; consumer demand [{}]; glyph capacity [{}]; Puzzle Puyo halfwidth additions: {}/{} shadowed spaces ({} remaining)",
        validation.total_entries,
        validation.untranslated,
        validation.in_progress,
        validation.needs_review,
        validation.needs_human_review,
        validation.complete,
        validation.release_candidate_eligible,
        validation.fixed_storage_checks,
        minimum_remaining_storage,
        validation.tutorial_encoded_byte_length,
        validation.tutorial_message_bank_byte_length,
        validation.tutorial_remaining_message_bank_bytes,
        demand,
        capacity,
        puzzle_puyo_halfwidth.required_added_glyphs,
        puzzle_puyo_halfwidth.assignable_shadowed_space_slots,
        puzzle_puyo_halfwidth.remaining_shadowed_space_slots,
    );
    Ok(())
}

fn load_fresh_corpora(
    command: &ExecutableTextTranslationCommand,
) -> Result<FreshExecutableTextCorpora> {
    let inventory = inspect(&command.source)?;
    let executable =
        read_inventory_file(&command.source.bin, &inventory, &command.executable_path)?;
    let executable_sha256 = find_inventory_file(&inventory, &command.executable_path)?
        .sha256
        .clone()
        .with_context(|| {
            format!(
                "executable inventory entry has no SHA-256: {}",
                command.executable_path
            )
        })?;
    let source_bin_sha256 = inventory.source.bin.sha256.clone();
    let survey = || MessageCorpusInputs {
        source_bin_sha256: source_bin_sha256.clone(),
        executable_iso_path: command.executable_path.clone(),
        executable_sha256: executable_sha256.clone(),
        executable: &executable,
    };
    let tutorial = survey_tutorial_messages(survey())?;
    let puzzle_puyo = survey_nazo_messages(survey())?;
    let memory_card_metadata = survey_memory_card_metadata(MemoryCardMetadataInputs {
        source_bin_sha256: source_bin_sha256.clone(),
        executable_iso_path: command.executable_path.clone(),
        executable_sha256: executable_sha256.clone(),
        executable: &executable,
    })?;
    Ok(FreshExecutableTextCorpora {
        inventory,
        executable,
        source_bin_sha256,
        executable_iso_path: command.executable_path.clone(),
        executable_sha256,
        tutorial,
        puzzle_puyo,
        memory_card_metadata,
    })
}
