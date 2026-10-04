use super::*;

#[derive(Debug, Parser)]
#[command(about = "Verify and survey the supported PS2 Puyo Puyo Tsuu source disc")]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Command,
}

#[derive(Debug, Subcommand)]
pub(super) enum Command {
    /// Create a private, hash-bound delta package and verify a full roundtrip.
    CreatePatchPackage {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long)]
        target_bin: PathBuf,
        #[arg(long)]
        build_report: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long, default_value = "xdelta3")]
        xdelta: PathBuf,
    },
    /// Verify the original disc and apply a local delta into a new directory.
    ApplyPatch {
        #[arg(long)]
        package: PathBuf,
        #[arg(long)]
        cue: PathBuf,
        #[arg(long)]
        bin: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long, default_value = "xdelta3")]
        xdelta: PathBuf,
    },
    /// Verify source identity, disc framing, ISO9660, and SYSTEM.CNF.
    VerifySource(SourceArgs),
    /// Verify the source and write a deterministic ISO9660 inventory.
    SurveyDisc {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the source and extract selected ISO9660 files without overwriting differences.
    ExtractFiles {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long = "path", required = true)]
        paths: Vec<String>,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Verify the source and typed-decode one R5900 ELF symbol.
    SurveyExecutable {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        symbol: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Classify Japanese-script Shift-JIS symbol candidates and static references.
    SurveyExecutableStrings {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "config/executable-string-review.json")]
        catalog: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Classify Japanese-looking Shift-JIS runs outside named ELF storage.
    SurveyExecutableStorageGaps {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "config/executable-storage-gap-review.json")]
        catalog: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Find direct and one-pointer GP-relative references to Japanese-script candidates.
    SurveyExecutableGpReferences {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "config/executable-string-review.json")]
        catalog: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Propagate straight-line constants to Japanese-script candidates.
    SurveyExecutableComputedReferences {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "config/executable-string-review.json")]
        catalog: PathBuf,
        #[arg(long, default_value = "config/executable-code-data-review.json")]
        code_data_review: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Census the three Shift-JIS font lookup and renderer families.
    SurveyExecutableFontConsumers {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Re-run every Japanese-bearing source surface and write the admitted target map.
    SurveyTranslationTargets {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "config/executable-string-review.json")]
        executable_string_catalog: PathBuf,
        #[arg(long, default_value = "config/executable-storage-gap-review.json")]
        storage_gap_catalog: PathBuf,
        #[arg(long, default_value = "config/executable-code-data-review.json")]
        code_data_review: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_preview_report: PathBuf,
        #[arg(long, default_value = "config/texture-language-survey.json")]
        texture_catalog: PathBuf,
        #[arg(long, default_value = "out/research/unlinked-textures.json")]
        unlinked_texture_preview_report: PathBuf,
        #[arg(long, default_value = "config/unlinked-texture-language-survey.json")]
        unlinked_texture_catalog: PathBuf,
        #[arg(long, default_value = "config/movie-language-survey.json")]
        movie_catalog: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Trace the Japanese memory-card title through the icon.sys write pipeline.
    SurveyMemoryCardMetadata {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the source and survey aligned CPX blocks in selected ISO9660 files.
    SurveyCpx {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long = "path", required = true)]
        paths: Vec<String>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the source and render title PSMT8 textures through their ELF-owned palettes.
    RenderTitleTextures {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_path: String,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Verify the source and render the consumer-linked message font atlas.
    RenderFontAtlas {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_path: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Verify and render the Nazo message font atlas through every registration set.
    RenderNazoFontAtlas {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_G.BIN;1")]
        image_path: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Verify and render the tutorial message font atlas through its registration path.
    RenderTutorialFontAtlas {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_path: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Verify and render the ranking-screen texture registration set.
    RenderRankTextures {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_G.BIN;1")]
        image_path: String,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Verify and render continue-screen textures through their pattern consumers.
    RenderContinueTextures {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_G.BIN;1")]
        image_path: String,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Verify and render Omake-menu textures through their pattern consumers.
    RenderOmakeTextures {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_path: String,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Verify and render rule-selection textures through their pattern consumers.
    RenderRuleTextures {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_path: String,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Census every structural ELF texture load table, pointer table, and direct reference.
    SurveyTextureLoadTables {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Census every aqCdvd_ReadFile call and resolve its load-table argument.
    SurveyTextureLoadCalls {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Render deduplicated load-order and declared consumer-paired texture previews.
    RenderTexturePreviews {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_d_path: String,
        #[arg(long, default_value = "/IMAGE_G.BIN;1")]
        image_g_path: String,
        #[arg(long, default_value = "/IMAGE_MD.BIN;1")]
        image_md_path: String,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Render structural texture candidates from a CPX container without an ELF file list.
    RenderUnlinkedTextures {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_E.BIN;1")]
        container_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_d_path: String,
        #[arg(long, default_value = "/IMAGE_G.BIN;1")]
        image_g_path: String,
        #[arg(long, default_value = "/IMAGE_MD.BIN;1")]
        image_md_path: String,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    /// Verify and classify every image in a texture preview corpus by visible language.
    SurveyTextureLanguages {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long)]
        preview_report: PathBuf,
        #[arg(long, default_value = "config/texture-language-survey.json")]
        catalog: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify and classify every rendered image from an unlinked CPX texture corpus.
    SurveyUnlinkedTextureLanguages {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long)]
        preview_report: PathBuf,
        #[arg(long, default_value = "config/unlinked-texture-language-survey.json")]
        catalog: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify every PSS frame, movie table entry, and whole-file language classification.
    SurveyMovies {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "config/movie-language-survey.json")]
        catalog: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the source and survey every message consumed by MemLoad_Act.
    SurveyMemoryCardMessages {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the source and survey every message consumed by MemSave_Act.
    SurveyMemoryCardSaveMessages {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the source and survey every message consumed by MyNazo_ErrMsg.
    SurveyMyNazoErrorMessages {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Create a non-overwriting exact-source memory-card translation bundle.
    InitMemoryCardTranslations {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(
            long,
            default_value = "assets/translation/memory-card-messages.ko.json"
        )]
        output: PathBuf,
    },
    /// Revalidate the memory-card translation bundle against the exact source.
    CheckMemoryCardTranslations {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_path: String,
        #[arg(
            long,
            default_value = "assets/translation/memory-card-messages.ko.json"
        )]
        translation: PathBuf,
    },
    /// Create a non-overwriting exact-source tutorial, Puzzle Puyo, and metadata bundle.
    InitExecutableTextTranslations {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "assets/translation/executable-text.ko.json")]
        output: PathBuf,
    },
    /// Revalidate executable text translations against exact-source consumer constraints.
    CheckExecutableTextTranslations {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "assets/translation/executable-text.ko.json")]
        translation: PathBuf,
    },
    /// Create the 74-surface partitioned story dialogue ledger in consumer order.
    InitStoryDialogueTranslations {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "out/research/translation-targets.json")]
        target_map: PathBuf,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/story-dialogue/manifest.json"
        )]
        output: PathBuf,
    },
    /// Revalidate the partitioned story dialogue set against the graphics ledger.
    CheckStoryDialogueTranslations {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "out/research/translation-targets.json")]
        target_map: PathBuf,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/story-dialogue/manifest.json"
        )]
        translation: PathBuf,
        #[arg(long, default_value = "config/story-dialogue-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
    },
    /// Render all 74 Korean story dialogue replacement previews from the exact source.
    RenderStoryDialogueReplacements {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/story-dialogue/manifest.json"
        )]
        story_translation: PathBuf,
        #[arg(long, default_value = "config/story-dialogue-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(long, default_value = "out/development/story-dialogue/replacements")]
        output_directory: PathBuf,
        #[arg(
            long,
            default_value = "out/development/story-dialogue/replacements.json"
        )]
        report: PathBuf,
    },
    /// Insert all 74 Korean story-dialogue replacements into a non-release development BIN/CUE.
    BuildStoryDialogueTranslationDevelopment {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/story-dialogue/manifest.json"
        )]
        story_translation: PathBuf,
        #[arg(long, default_value = "config/story-dialogue-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(
            long,
            default_value = "out/development/puyo2-story-dialogue-ko-dev.bin"
        )]
        output_bin: PathBuf,
        #[arg(
            long,
            default_value = "out/development/puyo2-story-dialogue-ko-dev.cue"
        )]
        output_cue: PathBuf,
        #[arg(long, default_value = "out/development/story-dialogue/replacements")]
        preview_directory: PathBuf,
        #[arg(
            long,
            default_value = "out/development/story-dialogue/translation-build.json"
        )]
        report: PathBuf,
    },
    /// Inventory exact Japanese and protected-English pixels for story dialogue surfaces.
    InventoryStoryDialogueGraphicsRegions {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/story-dialogue/manifest.json"
        )]
        story_translation: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(long, default_value = "assets/translation/graphics/story-dialogue")]
        output_directory: PathBuf,
        #[arg(
            long,
            default_value = "out/research/story-dialogue-graphics-regions.json"
        )]
        report: PathBuf,
    },
    /// Inventory visible Japanese pixels inside reviewed static graphics rectangles.
    InventoryStaticGraphicsRegions {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(long, default_value = "config/static-graphics-regions.json")]
        catalog: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(long, default_value = "assets/translation/graphics/static-regions")]
        output_directory: PathBuf,
        #[arg(long, default_value = "out/research/static-graphics-regions.json")]
        report: PathBuf,
        /// Refresh unapproved generated masks after an intentional catalog change.
        #[arg(long)]
        refresh_existing: bool,
    },
    /// Render deterministic Korean role overlays for functional UI; defer authored artwork.
    RenderProductStaticTextDrafts {
        /// Render only these catalog surface IDs; repeat for multiple surfaces.
        #[arg(long)]
        surface: Vec<String>,
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(long, default_value = "config/static-graphics-regions.json")]
        catalog: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(long, default_value = "config/static-ui-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(long, default_value = "assets/translation/graphics/static-ui")]
        overlay_directory: PathBuf,
        #[arg(long, default_value = "out/development/static-text/previews")]
        preview_directory: PathBuf,
        #[arg(long, default_value = "out/development/static-text/drafts.json")]
        report: PathBuf,
        /// Replace unapproved deterministic overlays after an intentional renderer change.
        #[arg(long)]
        refresh_existing: bool,
    },
    /// Render one fixed Korean label through an exact-hash bitmap-font pipeline.
    RenderDeterministicLabelMaster {
        #[arg(long)]
        configuration: PathBuf,
        #[arg(long, default_value = "config/static-ui-font.json")]
        font_configuration: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        report: PathBuf,
        /// Replace the output after intentionally changing the fixed raster inputs.
        #[arg(long)]
        refresh_existing: bool,
    },
    /// Render one hash-bound generated lettering master into an exact native palette overlay.
    RenderStylizedArtworkDraft {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(long, default_value = "config/static-graphics-regions.json")]
        catalog: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/graphics/tutorial/all-clear-callout.json"
        )]
        asset: PathBuf,
        #[arg(
            long,
            default_value = "out/development/stylized-artwork/tutorial-all-clear.native.png"
        )]
        native_preview: PathBuf,
        #[arg(
            long,
            default_value = "out/development/stylized-artwork/tutorial-all-clear.preview.png"
        )]
        surface_preview: PathBuf,
        #[arg(
            long,
            default_value = "out/development/stylized-artwork/tutorial-all-clear.json"
        )]
        report: PathBuf,
        /// Replace an unapproved overlay after intentionally changing its master or transform.
        #[arg(long)]
        refresh_existing: bool,
    },
    /// Render a coherent set of hash-bound generated lettering masters into one exact overlay.
    RenderStylizedArtworkSetDraft {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(long, default_value = "config/static-graphics-regions.json")]
        catalog: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(long)]
        asset: PathBuf,
        #[arg(long)]
        native_preview_directory: PathBuf,
        #[arg(long)]
        surface_preview: PathBuf,
        #[arg(long)]
        report: PathBuf,
        /// Replace an unapproved set overlay after intentionally changing its masters or transforms.
        #[arg(long)]
        refresh_existing: bool,
    },
    /// Render Korean phrase-atlas overlays through explicit consumer-oriented placements.
    RenderStaticFontAtlasDrafts {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(long, default_value = "config/static-graphics-regions.json")]
        catalog: PathBuf,
        #[arg(long, default_value = "config/font-atlas-layout.json")]
        layout: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(long, default_value = "config/static-ui-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(long, default_value = "assets/translation/graphics/font-atlas")]
        overlay_directory: PathBuf,
        #[arg(long, default_value = "out/development/font-atlas/previews")]
        preview_directory: PathBuf,
        #[arg(long, default_value = "out/development/font-atlas/drafts.json")]
        report: PathBuf,
        /// Replace unapproved deterministic overlays after an intentional layout change.
        #[arg(long)]
        refresh_existing: bool,
    },
    /// Insert all deterministic functional-UI overlays into a non-release development BIN/CUE.
    BuildStaticUiTranslationDevelopment {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(long, default_value = "config/static-graphics-regions.json")]
        catalog: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(
            long,
            default_value = "out/development/functional-ui-layout/puyo2-functional-ui-ko-dev.bin"
        )]
        output_bin: PathBuf,
        #[arg(
            long,
            default_value = "out/development/functional-ui-layout/puyo2-functional-ui-ko-dev.cue"
        )]
        output_cue: PathBuf,
        #[arg(long, default_value = "out/development/static-ui/build-previews")]
        preview_directory: PathBuf,
        #[arg(
            long,
            default_value = "out/development/static-ui/translation-build.json"
        )]
        report: PathBuf,
    },
    /// Insert every currently buildable static-graphics overlay into a development BIN/CUE.
    BuildStaticGraphicsTranslationDevelopment {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(long, default_value = "config/static-graphics-regions.json")]
        catalog: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(
            long,
            default_value = "out/development/static-graphics-layout/puyo2-static-graphics-ko-dev.bin"
        )]
        output_bin: PathBuf,
        #[arg(
            long,
            default_value = "out/development/static-graphics-layout/puyo2-static-graphics-ko-dev.cue"
        )]
        output_cue: PathBuf,
        #[arg(long, default_value = "out/development/static-graphics/build-previews")]
        preview_directory: PathBuf,
        #[arg(
            long,
            default_value = "out/development/static-graphics/translation-build.json"
        )]
        report: PathBuf,
    },
    /// Survey exact source palette indices and rendered colors inside one rectangle.
    SurveyStaticRegionPaletteIndices {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(long)]
        source_surface_id: String,
        #[arg(long)]
        preview_png_sha256: String,
        #[arg(long = "rectangle", required = true)]
        rectangles: Vec<StaticRegionCensusRectangle>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Create a non-overwriting graphics authoring ledger from the exact-source target map.
    InitGraphicsTranslations {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "out/research/translation-targets.json")]
        target_map: PathBuf,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        output: PathBuf,
    },
    /// Revalidate the graphics authoring ledger against its exact source and target map.
    CheckGraphicsTranslations {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "out/research/translation-targets.json")]
        target_map: PathBuf,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        translation: PathBuf,
    },
    /// Render a protected-pixel Korean title-logo development draft without patching the disc.
    RenderTitleTranslationDraft {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_path: String,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/graphics/title/title-logo-lettering-candidate.json"
        )]
        asset: PathBuf,
        #[arg(long, default_value = "out/development/title-logo-draft.png")]
        output: PathBuf,
        #[arg(
            long,
            default_value = "out/development/title-logo-source-japanese.mask.png"
        )]
        japanese_source_mask: PathBuf,
        #[arg(
            long,
            default_value = "out/development/title-logo-protected-nontext.mask.png"
        )]
        protected_nontext_mask: PathBuf,
        #[arg(
            long,
            default_value = "out/development/title-logo-protected-english.mask.png"
        )]
        protected_english_mask: PathBuf,
        #[arg(long, default_value = "out/development/title-logo-draft.json")]
        report: PathBuf,
    },
    /// Insert the protected Korean title draft into a non-release development BIN/CUE.
    BuildTitleTranslationDevelopment {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/graphics/title/title-logo-lettering-candidate.json"
        )]
        asset: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-title-ko-dev.bin")]
        output_bin: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-title-ko-dev.cue")]
        output_cue: PathBuf,
        #[arg(long, default_value = "out/development/title-logo-inserted.png")]
        preview: PathBuf,
        #[arg(long, default_value = "out/development/title-translation-build.json")]
        report: PathBuf,
    },
    /// Render exact Japanese ranking-label regions and protect every other source pixel.
    RenderRankingLabelRegionMasks {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_G.BIN;1")]
        image_path: String,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/graphics/ranking/solo-mode-label-source.mask.png"
        )]
        solo_label_mask: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/graphics/ranking/endless-mode-label-source.mask.png"
        )]
        endless_label_mask: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/graphics/ranking/protected-outside-japanese.mask.png"
        )]
        protected_mask: PathBuf,
        #[arg(long, default_value = "out/research/ranking-label-regions.json")]
        report: PathBuf,
    },
    /// Render deterministic Korean ranking labels without inserting a disc image.
    RenderRankingLabelDraft {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_G.BIN;1")]
        image_path: String,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        translation: PathBuf,
        #[arg(long, default_value = "config/ranking-label-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(long, default_value = "out/development/ranking-label-replacement.png")]
        output: PathBuf,
        #[arg(long, default_value = "out/development/ranking-label-draft.json")]
        report: PathBuf,
    },
    /// Insert the Korean ranking-label draft into a non-release development BIN/CUE.
    BuildRankingTranslationDevelopment {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/graphics/ranking/ranking-label-replacement.png"
        )]
        asset: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-ranking-ko-dev.bin")]
        output_bin: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-ranking-ko-dev.cue")]
        output_cue: PathBuf,
        #[arg(long, default_value = "out/development/ranking-label-inserted.png")]
        preview: PathBuf,
        #[arg(long, default_value = "out/development/ranking-translation-build.json")]
        report: PathBuf,
    },
    /// Insert all twelve Korean tutorial drafts into a non-release development BIN/CUE.
    BuildTutorialTranslationDevelopment {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/executable-text.ko.json")]
        translation: PathBuf,
        #[arg(long, default_value = "config/poc-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-tutorial-ko-dev.bin")]
        output_bin: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-tutorial-ko-dev.cue")]
        output_cue: PathBuf,
        #[arg(long, default_value = "out/development/tutorial-font-inserted.png")]
        preview: PathBuf,
        #[arg(
            long,
            default_value = "out/development/tutorial-translation-build.json"
        )]
        report: PathBuf,
    },
    /// Insert all 36 Korean Puzzle Puyo drafts into a non-release development BIN/CUE.
    BuildPuzzlePuyoTranslationDevelopment {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "assets/translation/executable-text.ko.json")]
        translation: PathBuf,
        #[arg(long, default_value = "config/poc-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-puzzle-puyo-ko-dev.bin")]
        output_bin: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-puzzle-puyo-ko-dev.cue")]
        output_cue: PathBuf,
        #[arg(long, default_value = "out/development/puzzle-puyo-font-inserted.png")]
        preview: PathBuf,
        #[arg(
            long,
            default_value = "out/development/puzzle-puyo-translation-build.json"
        )]
        report: PathBuf,
    },
    /// Verify the source and survey the complete ExpMsgPrint_Act tutorial table.
    SurveyTutorialMessages {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the complete direct NazoMsgPrint_Act producer population.
    SurveyNazoMessageProducers {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify all direct NazoMsgPrint_Act consumers and their four message tables.
    SurveyNazoMessages {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify the source and classify fullwidth slots for Korean allocation planning.
    PlanFontAllocation {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long, default_value = "/SLPM_624.00;1")]
        executable_path: String,
        #[arg(long, default_value = "/IMAGE_D.BIN;1")]
        image_path: String,
        #[arg(long)]
        atlas_output: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Insert all 75 Korean memory-card drafts into a non-release development BIN/CUE.
    BuildMemoryCardTranslationDevelopment {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(
            long,
            default_value = "assets/translation/memory-card-messages.ko.json"
        )]
        translation: PathBuf,
        #[arg(long, default_value = "config/poc-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-memory-card-ko-dev.bin")]
        output_bin: PathBuf,
        #[arg(long, default_value = "out/development/puyo2-memory-card-ko-dev.cue")]
        output_cue: PathBuf,
        #[arg(long, default_value = "out/development/memory-card-font-inserted.png")]
        preview: PathBuf,
        #[arg(
            long,
            default_value = "out/development/memory-card-translation-build.json"
        )]
        report: PathBuf,
    },
    /// Compose all currently buildable translation components from the verified source.
    BuildTechnicalTranslationDevelopment {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(
            long,
            default_value = "assets/translation/memory-card-messages.ko.json"
        )]
        memory_card_translation: PathBuf,
        #[arg(long, default_value = "assets/translation/executable-text.ko.json")]
        executable_text_translation: PathBuf,
        #[arg(long, default_value = "assets/translation/approved-terminology.json")]
        approved_terminology: PathBuf,
        #[arg(long, default_value = "assets/translation/graphics-text/manifest.json")]
        graphics_translation: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/story-dialogue/manifest.json"
        )]
        story_translation: PathBuf,
        #[arg(long, default_value = "config/poc-font.json")]
        shared_font_config: PathBuf,
        #[arg(long, default_value = "config/story-dialogue-font.json")]
        story_font_config: PathBuf,
        #[arg(long)]
        shared_font: PathBuf,
        #[arg(long)]
        story_font: PathBuf,
        #[arg(
            long,
            default_value = "assets/translation/graphics/ranking/ranking-label-replacement.png"
        )]
        ranking_asset: PathBuf,
        #[arg(long, default_value = "config/static-graphics-regions.json")]
        static_catalog: PathBuf,
        #[arg(long, default_value = "out/research/texture-previews.json")]
        texture_previews: PathBuf,
        #[arg(
            long,
            default_value = "out/development/technical-translation/puyo2-ko-technical-dev.bin"
        )]
        output_bin: PathBuf,
        #[arg(
            long,
            default_value = "out/development/technical-translation/puyo2-ko-technical-dev.cue"
        )]
        output_cue: PathBuf,
        #[arg(
            long,
            default_value = "out/development/technical-translation/build.json"
        )]
        report: PathBuf,
    },
    /// Build the bounded Korean memory-card message development PoC.
    BuildMemoryCardPoc {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(
            long,
            default_value = "assets/translation/memory-card-messages.ko.json"
        )]
        translation: PathBuf,
        #[arg(long, default_value = "config/poc-font.json")]
        font_config: PathBuf,
        #[arg(long)]
        font: PathBuf,
        #[arg(long)]
        output_bin: PathBuf,
        #[arg(long)]
        output_cue: PathBuf,
        #[arg(long)]
        preview: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
}

#[derive(Clone, Debug, clap::Args)]
pub(super) struct SourceArgs {
    #[arg(long, default_value = "config/source.json")]
    pub(super) manifest: PathBuf,
    #[arg(long)]
    pub(super) cue: PathBuf,
    #[arg(long)]
    pub(super) bin: PathBuf,
}
