use std::path::PathBuf;

use anyhow::{Context, Result};
use puyo2_ps2_tool::executable_font_consumers::{
    ExecutableFontConsumerInputs, survey_executable_font_consumers,
};
use puyo2_ps2_tool::font_atlas::{FontAtlasInputs, render_font_atlas};
use puyo2_ps2_tool::glyph_allocation::{GlyphAllocationInputs, plan_glyph_allocation};
use puyo2_ps2_tool::memory_card_translation::{
    MemoryCardTranslationBundle, MemoryCardTranslationInputs,
    compose_memory_card_translation_bundle, validate_memory_card_translation_bundle,
    validate_memory_card_translation_capacity,
};
use puyo2_ps2_tool::message_corpus::{
    MessageCorpusInputs, MessageCorpusReport, survey_memory_card_messages,
    survey_memory_card_save_messages, survey_my_nazo_error_messages,
};
use puyo2_ps2_tool::source::{
    DiscInventory, find_inventory_file, read_inventory_file, read_json_file, write_new_json_file,
};

use super::{SourceArgs, inspect};

pub(super) struct MemoryCardTranslationCommand {
    pub source: SourceArgs,
    pub executable_path: String,
    pub translation: PathBuf,
}

struct FreshMemoryCardCorpora {
    inventory: DiscInventory,
    executable: Vec<u8>,
    source_bin_sha256: String,
    executable_iso_path: String,
    executable_sha256: String,
    load: MessageCorpusReport,
    save: MessageCorpusReport,
    my_nazo_error: MessageCorpusReport,
}

impl FreshMemoryCardCorpora {
    fn inputs(&self) -> MemoryCardTranslationInputs<'_> {
        MemoryCardTranslationInputs {
            source_bin_sha256: &self.source_bin_sha256,
            executable_iso_path: &self.executable_iso_path,
            executable_sha256: &self.executable_sha256,
            load: &self.load,
            save: &self.save,
            my_nazo_error: &self.my_nazo_error,
        }
    }
}

pub(super) fn run_memory_card_translation_init(
    command: MemoryCardTranslationCommand,
) -> Result<()> {
    let corpora = load_fresh_corpora(&command)?;
    let bundle = compose_memory_card_translation_bundle(corpora.inputs())?;
    write_new_json_file(&command.translation, &bundle)?;
    println!(
        "initialized {} exact-source memory-card translations ({} shared across consumers) at {}",
        bundle.population.unique_messages,
        bundle.population.shared_messages,
        command.translation.display()
    );
    Ok(())
}

pub(super) fn run_memory_card_translation_check(
    command: MemoryCardTranslationCommand,
    image_path: String,
) -> Result<()> {
    let corpora = load_fresh_corpora(&command)?;
    let bundle: MemoryCardTranslationBundle = read_json_file(&command.translation)?;
    let validation = validate_memory_card_translation_bundle(&bundle, corpora.inputs())?;
    let image_container =
        read_inventory_file(&command.source.bin, &corpora.inventory, &image_path)?;
    let image_container_sha256 = find_inventory_file(&corpora.inventory, &image_path)?
        .sha256
        .clone()
        .with_context(|| format!("image inventory entry has no SHA-256: {image_path}"))?;
    let temporary = tempfile::tempdir().context("failed to create translation check directory")?;
    let font_atlas = render_font_atlas(
        FontAtlasInputs {
            source_bin_sha256: corpora.source_bin_sha256.clone(),
            executable_iso_path: corpora.executable_iso_path.clone(),
            executable_sha256: corpora.executable_sha256.clone(),
            executable: &corpora.executable,
            image_container_iso_path: image_path,
            image_container_sha256,
            image_container: &image_container,
        },
        &temporary.path().join("font-atlas.png"),
    )?;
    let message_corpora = [
        corpora.load.clone(),
        corpora.save.clone(),
        corpora.my_nazo_error.clone(),
    ];
    let font_consumers = survey_executable_font_consumers(ExecutableFontConsumerInputs {
        source_bin_sha256: corpora.source_bin_sha256.clone(),
        executable_iso_path: corpora.executable_iso_path.clone(),
        executable_sha256: corpora.executable_sha256.clone(),
        executable: &corpora.executable,
    })?;
    let allocation = plan_glyph_allocation(GlyphAllocationInputs {
        executable: &corpora.executable,
        font_atlas: &font_atlas,
        message_corpora: &message_corpora,
        font_consumers: &font_consumers,
    })?;
    let capacity = validate_memory_card_translation_capacity(&bundle, &allocation)?;
    let minimum_remaining_storage = capacity
        .minimum_remaining_storage_bytes
        .map_or_else(|| "n/a".to_string(), |bytes| bytes.to_string());
    println!(
        "validated {} memory-card translations: {} untranslated, {} in progress, {} awaiting review, {} awaiting human review, {} complete; {} release-candidate eligible; {} translated messages require {}/{} proven reassignable Hangul slots ({} remain) and pass {} fixed-storage checks (closest tail: {} bytes)",
        validation.unique_messages,
        validation.untranslated,
        validation.in_progress,
        validation.needs_review,
        validation.needs_human_review,
        validation.complete,
        validation.release_candidate_eligible,
        capacity.translated_messages,
        capacity.required_unique_hangul_glyphs,
        capacity.proven_reassignable_slots,
        capacity.remaining_reassignable_slots,
        capacity.fixed_storage_checks,
        minimum_remaining_storage,
    );
    Ok(())
}

fn load_fresh_corpora(command: &MemoryCardTranslationCommand) -> Result<FreshMemoryCardCorpora> {
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
    let load = survey_memory_card_messages(survey())?;
    let save = survey_memory_card_save_messages(survey())?;
    let my_nazo_error = survey_my_nazo_error_messages(survey())?;
    Ok(FreshMemoryCardCorpora {
        inventory,
        executable,
        source_bin_sha256,
        executable_iso_path: command.executable_path.clone(),
        executable_sha256,
        load,
        save,
        my_nazo_error,
    })
}
