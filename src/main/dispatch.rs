use super::cli::Command;
use super::*;

pub(super) fn run(command: Command) -> Result<()> {
    match command {
        Command::CreatePatchPackage {
            source,
            target_bin,
            build_report,
            output_directory,
            xdelta,
        } => {
            puyo2_ps2_tool::patch_package::create(
                &source.manifest,
                &source.cue,
                &source.bin,
                &target_bin,
                &build_report,
                &output_directory,
                &xdelta,
            )?;
            println!(
                "Verified local patch package: {}",
                output_directory.display()
            );
        }
        Command::ApplyPatch {
            package,
            cue,
            bin,
            output_directory,
            xdelta,
        } => {
            puyo2_ps2_tool::patch_package::apply(&package, &cue, &bin, &output_directory, &xdelta)?;
            println!("Verified patched BIN/CUE: {}", output_directory.display());
        }
        Command::VerifySource(source) => {
            let inventory = inspect(&source)?;
            println!("{}", serde_json::to_string_pretty(&inventory.source)?);
        }
        Command::SurveyDisc { source, output } => {
            let inventory = inspect(&source)?;
            write_inventory(&output, &inventory)?;
            println!(
                "wrote {} files and {} directories to {}",
                inventory.summary.files,
                inventory.summary.directories,
                output.display()
            );
        }
        Command::ExtractFiles {
            source,
            paths,
            output_directory,
        } => {
            let inventory = inspect(&source)?;
            let extracted = extract_files(&source.bin, &inventory, &paths, &output_directory)?;
            println!("{}", serde_json::to_string_pretty(&extracted)?);
        }
        Command::SurveyExecutable {
            source,
            executable_path,
            symbol,
            output,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, &executable_path)?;
            let executable_entry = find_inventory_file(&inventory, &executable_path)?;
            let report = ExecutableSurvey {
                schema: 1,
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path,
                executable_sha256: executable_entry
                    .sha256
                    .clone()
                    .with_context(|| "executable inventory entry has no SHA-256")?,
                disassembly: disassemble_symbol(&executable, &symbol)?,
            };
            write_json_file(&output, &report)?;
            println!(
                "decoded {} instructions from {} to {}",
                report.disassembly.instructions.len(),
                report.disassembly.symbol.name,
                output.display()
            );
        }
        Command::SurveyExecutableStrings {
            source,
            executable_path,
            catalog,
            output,
        } => run_executable_string_census(source, executable_path, catalog, output)?,
        Command::SurveyExecutableStorageGaps {
            source,
            executable_path,
            catalog,
            output,
        } => run_executable_storage_gap_survey(source, executable_path, catalog, output)?,
        Command::SurveyExecutableGpReferences {
            source,
            executable_path,
            catalog,
            output,
        } => run_executable_gp_reference_survey(source, executable_path, catalog, output)?,
        Command::SurveyExecutableComputedReferences {
            source,
            executable_path,
            catalog,
            code_data_review,
            output,
        } => run_executable_computed_reference_survey(
            source,
            executable_path,
            catalog,
            code_data_review,
            output,
        )?,
        Command::SurveyExecutableFontConsumers {
            source,
            executable_path,
            output,
        } => run_executable_font_consumer_survey(source, executable_path, output)?,
        Command::SurveyTranslationTargets {
            source,
            executable_path,
            executable_string_catalog,
            storage_gap_catalog,
            code_data_review,
            texture_preview_report,
            texture_catalog,
            unlinked_texture_preview_report,
            unlinked_texture_catalog,
            movie_catalog,
            output,
        } => run_translation_target_survey(TranslationTargetCommand {
            source,
            executable_path,
            executable_string_catalog,
            storage_gap_catalog,
            code_data_review,
            texture_preview_report,
            texture_catalog,
            unlinked_texture_preview_report,
            unlinked_texture_catalog,
            movie_catalog,
            output,
        })?,
        Command::SurveyMemoryCardMetadata {
            source,
            executable_path,
            output,
        } => run_memory_card_metadata_survey(source, executable_path, output)?,
        Command::SurveyCpx {
            source,
            executable_path,
            paths,
            output,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, &executable_path)?;
            let executable_sha256 = find_inventory_file(&inventory, &executable_path)?
                .sha256
                .clone()
                .with_context(|| {
                    format!("executable inventory entry has no SHA-256: {executable_path}")
                })?;
            let mut containers = Vec::with_capacity(paths.len());
            for path in paths {
                let bytes = read_inventory_file(&source.bin, &inventory, &path)?;
                let expected_sha256 = find_inventory_file(&inventory, &path)?
                    .sha256
                    .as_deref()
                    .with_context(|| format!("CPX inventory entry has no SHA-256: {path}"))?;
                let container = survey_cpx_container(&path, &bytes)?;
                anyhow::ensure!(
                    container.sha256 == expected_sha256,
                    "CPX container hash differs from the verified inventory: {path}"
                );
                containers.push(container);
            }
            let file_lists = link_image_file_lists(&executable, &mut containers)?;
            let report = assemble_cpx_corpus_survey(
                inventory.source.bin.sha256.clone(),
                CpxExecutableIdentity {
                    iso_path: executable_path,
                    sha256: executable_sha256,
                },
                file_lists,
                containers,
            )?;
            write_json_file(&output, &report)?;
            println!(
                "decoded {} CPX blocks from {} containers to {}",
                report.summary.blocks,
                report.summary.containers,
                output.display()
            );
        }
        Command::RenderTitleTextures {
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        } => run_title_texture_render(
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        )?,
        Command::RenderFontAtlas {
            source,
            executable_path,
            image_path,
            output,
            report,
        } => run_font_atlas_render(source, executable_path, image_path, output, report)?,
        Command::RenderNazoFontAtlas {
            source,
            executable_path,
            image_path,
            output,
            report,
        } => run_nazo_font_atlas_render(source, executable_path, image_path, output, report)?,
        Command::RenderTutorialFontAtlas {
            source,
            executable_path,
            image_path,
            output,
            report,
        } => run_tutorial_font_atlas_render(source, executable_path, image_path, output, report)?,
        Command::RenderRankTextures {
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        } => run_rank_texture_render(
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        )?,
        Command::RenderContinueTextures {
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        } => run_continue_texture_render(
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        )?,
        Command::RenderOmakeTextures {
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        } => run_omake_texture_render(
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        )?,
        Command::RenderRuleTextures {
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        } => run_rule_texture_render(
            source,
            executable_path,
            image_path,
            output_directory,
            report,
        )?,
        Command::SurveyTextureLoadTables {
            source,
            executable_path,
            output,
        } => run_texture_load_census(source, executable_path, output)?,
        Command::SurveyTextureLoadCalls {
            source,
            executable_path,
            output,
        } => run_texture_load_call_survey(source, executable_path, output)?,
        Command::RenderTexturePreviews {
            source,
            executable_path,
            image_d_path,
            image_g_path,
            image_md_path,
            output_directory,
            report,
        } => run_texture_preview_corpus(
            source,
            executable_path,
            [image_d_path, image_g_path, image_md_path],
            output_directory,
            report,
        )?,
        Command::RenderUnlinkedTextures {
            source,
            executable_path,
            container_path,
            image_d_path,
            image_g_path,
            image_md_path,
            output_directory,
            report,
        } => run_unlinked_texture_corpus(
            source,
            executable_path,
            container_path,
            [image_d_path, image_g_path, image_md_path],
            output_directory,
            report,
        )?,
        Command::SurveyTextureLanguages {
            source,
            preview_report,
            catalog,
            output,
        } => run_texture_language_survey(source, preview_report, catalog, output)?,
        Command::SurveyUnlinkedTextureLanguages {
            source,
            preview_report,
            catalog,
            output,
        } => run_unlinked_texture_language_survey(source, preview_report, catalog, output)?,
        Command::SurveyMovies {
            source,
            executable_path,
            catalog,
            output,
        } => run_movie_corpus_survey(source, executable_path, catalog, output)?,
        Command::SurveyMemoryCardMessages {
            source,
            executable_path,
            output,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, &executable_path)?;
            let executable_sha256 = find_inventory_file(&inventory, &executable_path)?
                .sha256
                .clone()
                .with_context(|| {
                    format!("executable inventory entry has no SHA-256: {executable_path}")
                })?;
            let report = survey_memory_card_messages(MessageCorpusInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path,
                executable_sha256,
                executable: &executable,
            })?;
            write_json_file(&output, &report)?;
            println!(
                "surveyed {} MemLoad_Act message sites and {} unique messages to {}",
                report.population.consumer_message_sites,
                report.population.unique_messages,
                output.display()
            );
        }
        Command::SurveyMemoryCardSaveMessages {
            source,
            executable_path,
            output,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, &executable_path)?;
            let executable_sha256 = find_inventory_file(&inventory, &executable_path)?
                .sha256
                .clone()
                .with_context(|| {
                    format!("executable inventory entry has no SHA-256: {executable_path}")
                })?;
            let report = survey_memory_card_save_messages(MessageCorpusInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path,
                executable_sha256,
                executable: &executable,
            })?;
            write_json_file(&output, &report)?;
            println!(
                "surveyed {} MemSave_Act message sites and {} unique messages to {}",
                report.population.consumer_message_sites,
                report.population.unique_messages,
                output.display()
            );
        }
        Command::SurveyMyNazoErrorMessages {
            source,
            executable_path,
            output,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, &executable_path)?;
            let executable_sha256 = find_inventory_file(&inventory, &executable_path)?
                .sha256
                .clone()
                .with_context(|| {
                    format!("executable inventory entry has no SHA-256: {executable_path}")
                })?;
            let report = survey_my_nazo_error_messages(MessageCorpusInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path,
                executable_sha256,
                executable: &executable,
            })?;
            write_json_file(&output, &report)?;
            println!(
                "surveyed {} MyNazo_ErrMsg message sites and {} unique messages to {}",
                report.population.consumer_message_sites,
                report.population.unique_messages,
                output.display()
            );
        }
        Command::InitMemoryCardTranslations {
            source,
            executable_path,
            output,
        } => run_memory_card_translation_init(MemoryCardTranslationCommand {
            source,
            executable_path,
            translation: output,
        })?,
        Command::CheckMemoryCardTranslations {
            source,
            executable_path,
            image_path,
            translation,
        } => run_memory_card_translation_check(
            MemoryCardTranslationCommand {
                source,
                executable_path,
                translation,
            },
            image_path,
        )?,
        Command::InitExecutableTextTranslations {
            source,
            executable_path,
            output,
        } => run_executable_text_translation_init(ExecutableTextTranslationCommand {
            source,
            executable_path,
            translation: output,
        })?,
        Command::CheckExecutableTextTranslations {
            source,
            executable_path,
            translation,
        } => run_executable_text_translation_check(ExecutableTextTranslationCommand {
            source,
            executable_path,
            translation,
        })?,
        Command::InitStoryDialogueTranslations {
            source,
            target_map,
            graphics_translation,
            output,
        } => run_story_dialogue_translation_init(StoryDialogueTranslationCommand {
            source,
            target_map,
            graphics_translation,
            story_translation: output,
            font_config: None,
            font: None,
        })?,
        Command::CheckStoryDialogueTranslations {
            source,
            target_map,
            graphics_translation,
            translation,
            font_config,
            font,
        } => run_story_dialogue_translation_check(StoryDialogueTranslationCommand {
            source,
            target_map,
            graphics_translation,
            story_translation: translation,
            font_config: Some(font_config),
            font: Some(font),
        })?,
        Command::RenderStoryDialogueReplacements {
            source,
            graphics_translation,
            story_translation,
            font_config,
            font,
            output_directory,
            report,
        } => run_story_dialogue_replacement_render(StoryDialogueReplacementCommand {
            source,
            graphics_translation,
            story_translation,
            font_config,
            font,
            output_directory,
            report,
        })?,
        Command::BuildStoryDialogueTranslationDevelopment {
            source,
            graphics_translation,
            story_translation,
            font_config,
            font,
            output_bin,
            output_cue,
            preview_directory,
            report,
        } => run_story_dialogue_translation_build(StoryDialogueTranslationBuildCommand {
            source,
            graphics_translation,
            story_translation,
            font_config,
            font,
            output_bin,
            output_cue,
            preview_directory,
            report,
        })?,
        Command::InventoryStoryDialogueGraphicsRegions {
            source,
            graphics_translation,
            story_translation,
            texture_previews,
            output_directory,
            report,
        } => run_story_dialogue_region_inventory(StoryDialogueRegionCommand {
            source,
            graphics_translation,
            story_translation,
            texture_previews,
            output_directory,
            report,
        })?,
        Command::InventoryStaticGraphicsRegions {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            output_directory,
            report,
            refresh_existing,
        } => run_static_region_inventory(StaticRegionCommand {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            output_directory,
            report,
            refresh_existing,
        })?,
        Command::RenderProductStaticTextDrafts {
            surface,
            source,
            graphics_translation,
            catalog,
            texture_previews,
            font_config,
            font,
            overlay_directory,
            preview_directory,
            report,
            refresh_existing,
        } => run_static_text_draft_render(StaticTextDraftCommand {
            surfaces: surface,
            source,
            graphics_translation,
            catalog,
            texture_previews,
            font_config,
            font,
            overlay_directory,
            preview_directory,
            report,
            refresh_existing,
            scope: StaticTextDraftScope::DeterministicText,
        })?,
        Command::RenderDeterministicLabelMaster {
            configuration,
            font_configuration,
            font,
            output,
            report,
            refresh_existing,
        } => run_deterministic_label_master_render(DeterministicLabelMasterCommand {
            configuration,
            font_configuration,
            font,
            output,
            report,
            refresh_existing,
        })?,
        Command::RenderStylizedArtworkDraft {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            asset,
            native_preview,
            surface_preview,
            report,
            refresh_existing,
        } => run_stylized_artwork_render(StylizedArtworkCommand {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            asset,
            native_preview,
            surface_preview,
            report,
            refresh_existing,
        })?,
        Command::RenderStylizedArtworkSetDraft {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            asset,
            native_preview_directory,
            surface_preview,
            report,
            refresh_existing,
        } => run_stylized_artwork_set_render(StylizedArtworkSetCommand {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            asset,
            native_preview_directory,
            surface_preview,
            report,
            refresh_existing,
        })?,
        Command::RenderStaticFontAtlasDrafts {
            source,
            graphics_translation,
            catalog,
            layout,
            texture_previews,
            font_config,
            font,
            overlay_directory,
            preview_directory,
            report,
            refresh_existing,
        } => run_font_atlas_draft_render(FontAtlasDraftCommand {
            source,
            graphics_translation,
            catalog,
            layout,
            texture_previews,
            font_config,
            font,
            overlay_directory,
            preview_directory,
            report,
            refresh_existing,
        })?,
        Command::BuildStaticUiTranslationDevelopment {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            output_bin,
            output_cue,
            preview_directory,
            report,
        } => run_static_ui_translation_build(StaticUiTranslationBuildCommand {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            output_bin,
            output_cue,
            preview_directory,
            report,
            scope: StaticGraphicsOverlayScope::FunctionalUi,
        })?,
        Command::BuildStaticGraphicsTranslationDevelopment {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            output_bin,
            output_cue,
            preview_directory,
            report,
        } => run_static_ui_translation_build(StaticUiTranslationBuildCommand {
            source,
            graphics_translation,
            catalog,
            texture_previews,
            output_bin,
            output_cue,
            preview_directory,
            report,
            scope: StaticGraphicsOverlayScope::BuildableStaticGraphics,
        })?,
        Command::SurveyStaticRegionPaletteIndices {
            source,
            graphics_translation,
            texture_previews,
            source_surface_id,
            preview_png_sha256,
            rectangles,
            output,
        } => run_static_region_palette_census(StaticRegionPaletteCensusCommand {
            source,
            graphics_translation,
            texture_previews,
            source_surface_id,
            preview_png_sha256,
            rectangles,
            output,
        })?,
        Command::InitGraphicsTranslations {
            source,
            target_map,
            output,
        } => run_graphics_translation_init(GraphicsTranslationCommand {
            source,
            target_map,
            translation: output,
        })?,
        Command::CheckGraphicsTranslations {
            source,
            target_map,
            translation,
        } => run_graphics_translation_check(GraphicsTranslationCommand {
            source,
            target_map,
            translation,
        })?,
        Command::RenderTitleTranslationDraft {
            source,
            executable_path,
            image_path,
            translation,
            asset,
            output,
            japanese_source_mask,
            protected_nontext_mask,
            protected_english_mask,
            report,
        } => run_title_translation_draft(TitleTranslationDraftCommand {
            source,
            executable_path,
            image_path,
            translation,
            asset,
            output,
            japanese_source_mask,
            protected_nontext_mask,
            protected_english_mask,
            report,
        })?,
        Command::BuildTitleTranslationDevelopment {
            source,
            translation,
            asset,
            output_bin,
            output_cue,
            preview,
            report,
        } => run_title_translation_build(TitleTranslationBuildCommand {
            source,
            translation,
            asset,
            output_bin,
            output_cue,
            preview,
            report,
        })?,
        Command::RenderRankingLabelRegionMasks {
            source,
            executable_path,
            image_path,
            translation,
            solo_label_mask,
            endless_label_mask,
            protected_mask,
            report,
        } => run_ranking_label_region_render(RankingLabelRegionCommand {
            source,
            executable_path,
            image_path,
            translation,
            solo_label_mask,
            endless_label_mask,
            protected_mask,
            report,
        })?,
        Command::RenderRankingLabelDraft {
            source,
            executable_path,
            image_path,
            translation,
            font_config,
            font,
            output,
            report,
        } => run_ranking_label_draft(RankingLabelDraftCommand {
            source,
            executable_path,
            image_path,
            translation,
            font_config,
            font,
            output,
            report,
        })?,
        Command::BuildRankingTranslationDevelopment {
            source,
            translation,
            asset,
            output_bin,
            output_cue,
            preview,
            report,
        } => run_ranking_translation_build(RankingTranslationBuildCommand {
            source,
            translation,
            asset,
            output_bin,
            output_cue,
            preview,
            report,
        })?,
        Command::BuildTutorialTranslationDevelopment {
            source,
            translation,
            font_config,
            font,
            output_bin,
            output_cue,
            preview,
            report,
        } => run_tutorial_translation_build(TutorialTranslationBuildCommand {
            source,
            translation,
            font_config,
            font,
            output_bin,
            output_cue,
            preview,
            report,
        })?,
        Command::BuildPuzzlePuyoTranslationDevelopment {
            source,
            translation,
            font_config,
            font,
            output_bin,
            output_cue,
            preview,
            report,
        } => run_puzzle_puyo_translation_build(PuzzlePuyoTranslationBuildCommand {
            source,
            translation,
            font_config,
            font,
            output_bin,
            output_cue,
            preview,
            report,
        })?,
        Command::SurveyTutorialMessages {
            source,
            executable_path,
            output,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, &executable_path)?;
            let executable_sha256 = find_inventory_file(&inventory, &executable_path)?
                .sha256
                .clone()
                .with_context(|| {
                    format!("executable inventory entry has no SHA-256: {executable_path}")
                })?;
            let report = survey_tutorial_messages(MessageCorpusInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path,
                executable_sha256,
                executable: &executable,
            })?;
            write_json_file(&output, &report)?;
            println!(
                "surveyed {} ExpMsgPrint_Act producer and {} tutorial messages to {}",
                report.producer.renderer_actor_creations,
                report.population.unique_messages,
                output.display()
            );
        }
        Command::SurveyNazoMessageProducers {
            source,
            executable_path,
            output,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, &executable_path)?;
            let executable_sha256 = find_inventory_file(&inventory, &executable_path)?
                .sha256
                .clone()
                .with_context(|| {
                    format!("executable inventory entry has no SHA-256: {executable_path}")
                })?;
            let report = survey_nazo_message_producers(MessageCorpusInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path,
                executable_sha256,
                executable: &executable,
            })?;
            write_json_file(&output, &report)?;
            println!(
                "surveyed {} NazoMsgPrint_Act producer functions and {} actor creations to {}",
                report.population.producer_functions,
                report.population.renderer_actor_creations,
                output.display()
            );
        }
        Command::SurveyNazoMessages {
            source,
            executable_path,
            output,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, &executable_path)?;
            let executable_sha256 = find_inventory_file(&inventory, &executable_path)?
                .sha256
                .clone()
                .with_context(|| {
                    format!("executable inventory entry has no SHA-256: {executable_path}")
                })?;
            let report = survey_nazo_messages(MessageCorpusInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path,
                executable_sha256,
                executable: &executable,
            })?;
            write_json_file(&output, &report)?;
            println!(
                "surveyed {} NazoMsgPrint_Act message sites and {} unique messages to {}",
                report.population.consumer_message_sites,
                report.population.unique_messages,
                output.display()
            );
        }
        Command::PlanFontAllocation {
            source,
            executable_path,
            image_path,
            atlas_output,
            output,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, &executable_path)?;
            let executable_sha256 = find_inventory_file(&inventory, &executable_path)?
                .sha256
                .clone()
                .with_context(|| {
                    format!("executable inventory entry has no SHA-256: {executable_path}")
                })?;
            let image_container = read_inventory_file(&source.bin, &inventory, &image_path)?;
            let image_container_sha256 = find_inventory_file(&inventory, &image_path)?
                .sha256
                .clone()
                .with_context(|| format!("image inventory entry has no SHA-256: {image_path}"))?;
            let font_atlas = render_font_atlas(
                FontAtlasInputs {
                    source_bin_sha256: inventory.source.bin.sha256.clone(),
                    executable_iso_path: executable_path.clone(),
                    executable_sha256: executable_sha256.clone(),
                    executable: &executable,
                    image_container_iso_path: image_path,
                    image_container_sha256,
                    image_container: &image_container,
                },
                &atlas_output,
            )?;
            let load_message_corpus = survey_memory_card_messages(MessageCorpusInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path.clone(),
                executable_sha256: executable_sha256.clone(),
                executable: &executable,
            })?;
            let save_message_corpus = survey_memory_card_save_messages(MessageCorpusInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path.clone(),
                executable_sha256: executable_sha256.clone(),
                executable: &executable,
            })?;
            let my_nazo_error_corpus = survey_my_nazo_error_messages(MessageCorpusInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path.clone(),
                executable_sha256: executable_sha256.clone(),
                executable: &executable,
            })?;
            let message_corpora = [
                load_message_corpus,
                save_message_corpus,
                my_nazo_error_corpus,
            ];
            let font_consumers = survey_executable_font_consumers(ExecutableFontConsumerInputs {
                source_bin_sha256: inventory.source.bin.sha256.clone(),
                executable_iso_path: executable_path.clone(),
                executable_sha256: executable_sha256.clone(),
                executable: &executable,
            })?;
            let report = plan_glyph_allocation(GlyphAllocationInputs {
                executable: &executable,
                font_atlas: &font_atlas,
                message_corpora: &message_corpora,
                font_consumers: &font_consumers,
            })?;
            write_json_file(&output, &report)?;
            println!(
                "classified {} fullwidth slots: {} declared-corpus required, {} executable-census unused and {} shadowed duplicate candidates, {} proven reassignable total; wrote {}",
                report.population.fullwidth_slots,
                report.population.declared_corpus_required_slots,
                report.population.reachable_slots_unused_by_declared_corpus,
                report.population.shadowed_duplicate_slots,
                report.population.proven_reassignable_slots,
                output.display()
            );
        }
        Command::BuildMemoryCardTranslationDevelopment {
            source,
            translation,
            font_config,
            font,
            output_bin,
            output_cue,
            preview,
            report,
        } => run_memory_card_translation_build(MemoryCardTranslationBuildCommand {
            source,
            translation,
            font_config,
            font,
            output_bin,
            output_cue,
            preview,
            report,
        })?,
        Command::BuildTechnicalTranslationDevelopment {
            source,
            memory_card_translation,
            executable_text_translation,
            approved_terminology,
            graphics_translation,
            story_translation,
            shared_font_config,
            story_font_config,
            shared_font,
            story_font,
            ranking_asset,
            static_catalog,
            texture_previews,
            output_bin,
            output_cue,
            report,
        } => run_technical_translation_build(TechnicalTranslationBuildCommand {
            source,
            memory_card_translation,
            executable_text_translation,
            approved_terminology,
            graphics_translation,
            story_translation,
            shared_font_config,
            story_font_config,
            shared_font,
            story_font,
            ranking_asset,
            static_catalog,
            texture_previews,
            output_bin,
            output_cue,
            report,
        })?,
        Command::BuildMemoryCardPoc {
            source,
            translation,
            font_config,
            font,
            output_bin,
            output_cue,
            preview,
            report,
        } => {
            let inventory = inspect(&source)?;
            let executable = read_inventory_file(&source.bin, &inventory, "/SLPM_624.00;1")?;
            let image_container = read_inventory_file(&source.bin, &inventory, "/IMAGE_D.BIN;1")?;
            let poc = build_memory_card_poc(MemoryCardPocInputs {
                inventory: &inventory,
                source_bin_path: &source.bin,
                executable: &executable,
                image_container: &image_container,
                translation_path: &translation,
                font_config_path: &font_config,
                font_path: &font,
                output_bin_path: &output_bin,
                output_cue_path: &output_cue,
                preview_path: &preview,
            })?;
            write_json_file(&report, &poc)?;
            println!(
                "built {} changed raw sectors to {} ({})",
                poc.expected_writes.raw_sector_plans,
                output_bin.display(),
                poc.output.bin_sha256
            );
        }
    }
    Ok(())
}
