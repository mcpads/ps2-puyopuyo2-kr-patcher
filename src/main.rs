use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use puyo2_ps2_tool::cpx::{
    CpxExecutableIdentity, assemble_cpx_corpus_survey, link_image_file_lists, survey_cpx_container,
};
use puyo2_ps2_tool::executable::{ExecutableSurvey, disassemble_symbol};
use puyo2_ps2_tool::executable_font_consumers::{
    ExecutableFontConsumerInputs, survey_executable_font_consumers,
};
use puyo2_ps2_tool::font_atlas::{FontAtlasInputs, render_font_atlas};
use puyo2_ps2_tool::glyph_allocation::{GlyphAllocationInputs, plan_glyph_allocation};
use puyo2_ps2_tool::memory_card_poc::{MemoryCardPocInputs, build_memory_card_poc};
use puyo2_ps2_tool::message_corpus::{
    MessageCorpusInputs, survey_memory_card_messages, survey_memory_card_save_messages,
    survey_my_nazo_error_messages, survey_nazo_message_producers, survey_nazo_messages,
    survey_tutorial_messages,
};
use puyo2_ps2_tool::source::{
    extract_files, find_inventory_file, inspect_source, read_inventory_file, write_inventory,
    write_json_file,
};

#[path = "main/cli.rs"]
mod cli;
#[path = "main/dispatch.rs"]
mod dispatch;

#[path = "main/render_commands.rs"]
mod render_commands;

#[path = "main/render_source.rs"]
mod render_source;

#[path = "main/memory_card_translation_command.rs"]
mod memory_card_translation_command;

#[path = "main/memory_card_translation_build_command.rs"]
mod memory_card_translation_build_command;

#[path = "main/executable_text_translation_command.rs"]
mod executable_text_translation_command;

#[path = "main/font_atlas_draft_command.rs"]
mod font_atlas_draft_command;

#[path = "main/graphics_translation_command.rs"]
mod graphics_translation_command;

#[path = "main/title_translation_draft_command.rs"]
mod title_translation_draft_command;

#[path = "main/title_translation_build_command.rs"]
mod title_translation_build_command;

#[path = "main/ranking_label_command.rs"]
mod ranking_label_command;

#[path = "main/ranking_translation_build_command.rs"]
mod ranking_translation_build_command;

#[path = "main/tutorial_translation_build_command.rs"]
mod tutorial_translation_build_command;

#[path = "main/puzzle_puyo_translation_build_command.rs"]
mod puzzle_puyo_translation_build_command;

#[path = "main/survey_commands.rs"]
mod survey_commands;

#[path = "main/translation_target_command.rs"]
mod translation_target_command;

#[path = "main/story_dialogue_translation_command.rs"]
mod story_dialogue_translation_command;

#[path = "main/story_dialogue_region_command.rs"]
mod story_dialogue_region_command;

#[path = "main/story_dialogue_replacement_command.rs"]
mod story_dialogue_replacement_command;

#[path = "main/story_dialogue_translation_build_command.rs"]
mod story_dialogue_translation_build_command;

#[path = "main/static_region_command.rs"]
mod static_region_command;

#[path = "main/static_ui_command.rs"]
mod static_ui_command;

#[path = "main/deterministic_label_command.rs"]
mod deterministic_label_command;

#[path = "main/stylized_artwork_command.rs"]
mod stylized_artwork_command;

#[path = "main/static_ui_translation_build_command.rs"]
mod static_ui_translation_build_command;

#[path = "main/technical_translation_build_command.rs"]
mod technical_translation_build_command;

use cli::{Cli, SourceArgs};
use deterministic_label_command::{
    DeterministicLabelMasterCommand, run_deterministic_label_master_render,
};
use executable_text_translation_command::{
    ExecutableTextTranslationCommand, run_executable_text_translation_check,
    run_executable_text_translation_init,
};
use font_atlas_draft_command::{FontAtlasDraftCommand, run_font_atlas_draft_render};
use graphics_translation_command::{
    GraphicsTranslationCommand, run_graphics_translation_check, run_graphics_translation_init,
};
use memory_card_translation_build_command::{
    MemoryCardTranslationBuildCommand, run_memory_card_translation_build,
};
use memory_card_translation_command::{
    MemoryCardTranslationCommand, run_memory_card_translation_check,
    run_memory_card_translation_init,
};
use puyo2_ps2_tool::graphics_translation::static_ui::StaticTextDraftScope;
use puyo2_ps2_tool::static_ui_translation_build::StaticGraphicsOverlayScope;
use puzzle_puyo_translation_build_command::{
    PuzzlePuyoTranslationBuildCommand, run_puzzle_puyo_translation_build,
};
use ranking_label_command::{
    RankingLabelDraftCommand, RankingLabelRegionCommand, run_ranking_label_draft,
    run_ranking_label_region_render,
};
use ranking_translation_build_command::{
    RankingTranslationBuildCommand, run_ranking_translation_build,
};
use render_commands::{
    run_continue_texture_render, run_font_atlas_render, run_nazo_font_atlas_render,
    run_omake_texture_render, run_rank_texture_render, run_rule_texture_render,
    run_texture_preview_corpus, run_title_texture_render, run_tutorial_font_atlas_render,
    run_unlinked_texture_corpus,
};
use static_region_command::{
    StaticRegionCensusRectangle, StaticRegionCommand, StaticRegionPaletteCensusCommand,
    run_static_region_inventory, run_static_region_palette_census,
};
use static_ui_command::{StaticTextDraftCommand, run_static_text_draft_render};
use static_ui_translation_build_command::{
    StaticUiTranslationBuildCommand, run_static_ui_translation_build,
};
use story_dialogue_region_command::{
    StoryDialogueRegionCommand, run_story_dialogue_region_inventory,
};
use story_dialogue_replacement_command::{
    StoryDialogueReplacementCommand, run_story_dialogue_replacement_render,
};
use story_dialogue_translation_build_command::{
    StoryDialogueTranslationBuildCommand, run_story_dialogue_translation_build,
};
use story_dialogue_translation_command::{
    StoryDialogueTranslationCommand, run_story_dialogue_translation_check,
    run_story_dialogue_translation_init,
};
use stylized_artwork_command::{
    StylizedArtworkCommand, StylizedArtworkSetCommand, run_stylized_artwork_render,
    run_stylized_artwork_set_render,
};
use survey_commands::{
    run_executable_computed_reference_survey, run_executable_font_consumer_survey,
    run_executable_gp_reference_survey, run_executable_storage_gap_survey,
    run_executable_string_census, run_memory_card_metadata_survey, run_movie_corpus_survey,
    run_texture_language_survey, run_texture_load_call_survey, run_texture_load_census,
    run_unlinked_texture_language_survey,
};
use technical_translation_build_command::{
    TechnicalTranslationBuildCommand, run_technical_translation_build,
};
use title_translation_build_command::{TitleTranslationBuildCommand, run_title_translation_build};
use title_translation_draft_command::{TitleTranslationDraftCommand, run_title_translation_draft};
use translation_target_command::{TranslationTargetCommand, run_translation_target_survey};
use tutorial_translation_build_command::{
    TutorialTranslationBuildCommand, run_tutorial_translation_build,
};

fn main() -> Result<()> {
    dispatch::run(Cli::parse().command)
}

fn inspect(source: &SourceArgs) -> Result<puyo2_ps2_tool::source::DiscInventory> {
    ensure_regular_file(&source.manifest, "manifest")?;
    ensure_regular_file(&source.cue, "CUE")?;
    ensure_regular_file(&source.bin, "BIN")?;
    inspect_source(&source.manifest, &source.cue, &source.bin)
}

fn ensure_regular_file(path: &Path, label: &str) -> Result<()> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("failed to stat {label} {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "{label} is not a regular file: {}",
        path.display()
    );
    Ok(())
}
