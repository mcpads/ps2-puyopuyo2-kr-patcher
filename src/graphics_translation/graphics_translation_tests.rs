use super::*;
use crate::texture_language_survey::TextureLanguageClassification;

fn target(record: usize) -> LinkedTextureTarget {
    LinkedTextureTarget {
        container_iso_path: "/IMAGE_D.BIN;1".to_string(),
        image_file_record: record,
        width: 640,
        height: 512,
        linear_indices_sha256: format!("hash-{record}"),
        classification: TextureLanguageClassification::JapaneseOnly,
        requires_english_pixel_preservation: false,
        owner_tables: vec!["TitleTexTbl".to_string()],
    }
}

fn inputs(targets: &[LinkedTextureTarget]) -> GraphicsTranslationInputs<'_> {
    GraphicsTranslationInputs {
        source_bin_sha256: "source-hash",
        executable_iso_path: "/SLPM_624.00;1",
        executable_sha256: "executable-hash",
        linked_texture_targets: targets,
    }
}

#[test]
fn baseline_keeps_physical_locator_separate_from_pending_semantics() {
    let targets = vec![target(19)];
    let bundle = compose_graphics_translation_bundle(inputs(&targets)).unwrap();
    let entry = &bundle.entries[0];

    assert_eq!(
        entry.source.source_surface_id,
        "linked-texture/image-d/000019"
    );
    assert_eq!(
        entry.authoring.status,
        GraphicsTranslationStatus::NeedsSemanticInventory
    );
    assert_eq!(entry.authoring.semantic_asset_id, None);
}

#[test]
fn validation_rejects_source_drift() {
    let targets = vec![target(19)];
    let mut bundle = compose_graphics_translation_bundle(inputs(&targets)).unwrap();
    bundle.entries[0].source.linear_indices_sha256 = "changed".to_string();

    let error = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap_err();
    assert!(error.to_string().contains("protected source fields differ"));
}

#[test]
fn semantic_inventory_can_advance_without_claiming_region_readiness() {
    let targets = vec![target(19)];
    let mut bundle = compose_graphics_translation_bundle(inputs(&targets)).unwrap();
    let authoring = &mut bundle.entries[0].authoring;
    authoring.status = GraphicsTranslationStatus::NeedsRegionInventory;
    authoring.semantic_asset_id = Some("title/primary-branding-sheet".to_string());
    authoring.semantic_role = Some("composite_title_branding".to_string());
    authoring.continuity_group = Some("title_branding".to_string());

    let validation = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap();
    assert_eq!(validation.needs_region_inventory, 1);
    assert_eq!(validation.release_candidate_eligible, 0);
}

#[test]
fn complete_status_cannot_bypass_runtime_and_release_evidence() {
    let targets = vec![target(19)];
    let mut bundle = compose_graphics_translation_bundle(inputs(&targets)).unwrap();
    bundle.entries[0].authoring.status = GraphicsTranslationStatus::Complete;

    let error = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("complete graphics status is unavailable")
    );
}

#[test]
fn mixed_surface_cannot_advance_without_an_english_protection_mask() {
    let mut mixed = target(19);
    mixed.classification = TextureLanguageClassification::JapaneseEnglishMixed;
    mixed.requires_english_pixel_preservation = true;
    let targets = vec![mixed];
    let mut bundle = compose_graphics_translation_bundle(inputs(&targets)).unwrap();
    let authoring = &mut bundle.entries[0].authoring;
    authoring.status = GraphicsTranslationStatus::InProgress;
    authoring.semantic_asset_id = Some("title/primary-branding-sheet".to_string());
    authoring.semantic_role = Some("composite_title_branding".to_string());
    authoring.continuity_group = Some("title_branding".to_string());
    authoring.region_inventory_complete = true;
    authoring.regions.push(GraphicsTextRegion {
        artwork_uses_overlay_alpha: false,
        region_id: "primary-logo".to_string(),
        semantic_role: "primary_title_logo".to_string(),
        bounds: PixelBounds {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        },
        artwork_composition_bounds: None,
        source_restoration_bounds: Vec::new(),
        source_background_restoration: GraphicsSourceBackgroundRestoration::NearestSourceIndex,
        source_fill_color: None,
        source_text: "ぷよぷよ通".to_string(),
        korean_text: Some("뿌요뿌요 2".to_string()),
        treatment_profile: GraphicsTreatmentProfile::StylizedDisplayLettering,
        artwork_plan: Some(GraphicsArtworkPlan {
            method: GraphicsArtworkMethod::GeneratedOrHandAuthored,
            placement_status: GraphicsArtworkPlacementStatus::RoughRegionBounds,
        }),
        glyph_demand_source: None,
        source_pixel_mask: HashedAsset {
            path: "assets/translation/graphics/title-logo.mask.png".to_string(),
            sha256: "mask-hash".to_string(),
        },
        translator_notes: Vec::new(),
    });

    let error = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap_err();
    assert!(error.to_string().contains("in-progress graphics entry"));
}

#[test]
fn rough_stylized_bounds_cannot_enter_human_review() {
    let targets = vec![target(19)];
    let mut bundle = compose_graphics_translation_bundle(inputs(&targets)).unwrap();
    let authoring = &mut bundle.entries[0].authoring;
    authoring.status = GraphicsTranslationStatus::NeedsHumanReview;
    authoring.semantic_asset_id = Some("title/primary-branding-sheet".to_string());
    authoring.semantic_role = Some("composite_title_branding".to_string());
    authoring.continuity_group = Some("title_branding".to_string());
    authoring.region_inventory_complete = true;
    authoring.replacement_overlay = Some(HashedAsset {
        path: "assets/translation/graphics/title-logo.roles.png".to_string(),
        sha256: "overlay-hash".to_string(),
    });
    authoring.regions.push(GraphicsTextRegion {
        artwork_uses_overlay_alpha: false,
        region_id: "primary-logo".to_string(),
        semantic_role: "primary_title_logo".to_string(),
        bounds: PixelBounds {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        },
        artwork_composition_bounds: None,
        source_restoration_bounds: Vec::new(),
        source_background_restoration: GraphicsSourceBackgroundRestoration::NearestSourceIndex,
        source_fill_color: None,
        source_text: "ぷよぷよ通".to_string(),
        korean_text: Some("뿌요뿌요 2".to_string()),
        treatment_profile: GraphicsTreatmentProfile::StylizedDisplayLettering,
        artwork_plan: Some(GraphicsArtworkPlan {
            method: GraphicsArtworkMethod::GeneratedOrHandAuthored,
            placement_status: GraphicsArtworkPlacementStatus::RoughRegionBounds,
        }),
        glyph_demand_source: None,
        source_pixel_mask: HashedAsset {
            path: "assets/translation/graphics/title-logo.mask.png".to_string(),
            sha256: "mask-hash".to_string(),
        },
        translator_notes: Vec::new(),
    });

    let error = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("consumer-aligned authored evidence")
    );

    bundle.entries[0].authoring.regions[0]
        .artwork_plan
        .as_mut()
        .unwrap()
        .placement_status = GraphicsArtworkPlacementStatus::ConsumerAligned;
    let validation = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap();
    assert_eq!(validation.needs_human_review, 1);
}

#[test]
fn tea_labels_are_excluded_from_generated_artwork() {
    let mut tea_target = target(39);
    tea_target.container_iso_path = "/IMAGE_G.BIN;1".to_string();
    tea_target.owner_tables = vec!["FaceTex_Ske_t".to_string()];
    let targets = vec![tea_target];
    let mut bundle = compose_graphics_translation_bundle(inputs(&targets)).unwrap();
    let authoring = &mut bundle.entries[0].authoring;
    authoring.status = GraphicsTranslationStatus::InProgress;
    authoring.semantic_asset_id = Some("character/skeleton-t-tea-prop-sheet".to_string());
    authoring.semantic_role = Some("skeleton_t_tea_prop".to_string());
    authoring.continuity_group = Some("skeleton_t_character_presentation".to_string());
    authoring.region_inventory_complete = true;
    authoring.regions.push(GraphicsTextRegion {
        artwork_uses_overlay_alpha: false,
        region_id: "tea-cup-label".to_string(),
        semantic_role: "skeleton_t_tea_cup_label".to_string(),
        bounds: PixelBounds {
            x: 0,
            y: 0,
            width: 18,
            height: 20,
        },
        artwork_composition_bounds: None,
        source_restoration_bounds: Vec::new(),
        source_background_restoration: GraphicsSourceBackgroundRestoration::NearestSourceIndex,
        source_fill_color: None,
        source_text: "茶".to_string(),
        korean_text: Some("차".to_string()),
        treatment_profile: GraphicsTreatmentProfile::StylizedDisplayLettering,
        artwork_plan: Some(GraphicsArtworkPlan {
            method: GraphicsArtworkMethod::GeneratedOrHandAuthored,
            placement_status: GraphicsArtworkPlacementStatus::RoughRegionBounds,
        }),
        glyph_demand_source: None,
        source_pixel_mask: HashedAsset {
            path: "assets/translation/graphics/tea-label.mask.png".to_string(),
            sha256: "mask-hash".to_string(),
        },
        translator_notes: Vec::new(),
    });

    let error = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap_err();
    assert!(error.to_string().contains("wrong artwork method"));

    bundle.entries[0].authoring.regions[0]
        .artwork_plan
        .as_mut()
        .unwrap()
        .method = GraphicsArtworkMethod::DeterministicRaster;
    let validation = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap();
    assert_eq!(validation.in_progress, 1);
}

#[test]
fn graphics_ledger_derives_font_glyph_demand_from_the_text_consumer() {
    let targets = vec![target(22)];
    let mut bundle = compose_graphics_translation_bundle(inputs(&targets)).unwrap();
    let authoring = &mut bundle.entries[0].authoring;
    authoring.status = GraphicsTranslationStatus::InProgress;
    authoring.semantic_asset_id = Some("tutorial/message-glyph-atlas".to_string());
    authoring.semantic_role = Some("tutorial_message_glyph_supply".to_string());
    authoring.continuity_group = Some("tutorial_interface".to_string());
    authoring.region_inventory_complete = true;
    authoring.regions.push(GraphicsTextRegion {
        artwork_uses_overlay_alpha: false,
        region_id: "tutorial-message-glyph-supply".to_string(),
        semantic_role: "tutorial_message_glyph_supply".to_string(),
        bounds: PixelBounds {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        },
        artwork_composition_bounds: None,
        source_restoration_bounds: Vec::new(),
        source_background_restoration: GraphicsSourceBackgroundRestoration::NearestSourceIndex,
        source_fill_color: None,
        source_text: "落下".to_string(),
        korean_text: Some("떨어짐".to_string()),
        treatment_profile: GraphicsTreatmentProfile::FontGlyphSupply,
        artwork_plan: None,
        glyph_demand_source: Some(GlyphDemandSource::TutorialMessages),
        source_pixel_mask: HashedAsset {
            path: "assets/translation/graphics/tutorial-glyphs.mask.png".to_string(),
            sha256: "mask-hash".to_string(),
        },
        translator_notes: Vec::new(),
    });

    let error = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("must derive demand from its text consumer")
    );

    bundle.entries[0].authoring.regions[0].korean_text = None;
    let validation = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap();
    assert_eq!(validation.in_progress, 1);

    bundle.entries[0].authoring.regions[0].glyph_demand_source =
        Some(GlyphDemandSource::PuzzlePuyoMessages);
    let error = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("not bound to the exact source consumer")
    );
}

#[test]
fn graphics_ledger_rejects_an_english_only_translation_region() {
    let targets = vec![target(19)];
    let mut bundle = compose_graphics_translation_bundle(inputs(&targets)).unwrap();
    let authoring = &mut bundle.entries[0].authoring;
    authoring.status = GraphicsTranslationStatus::InProgress;
    authoring.semantic_asset_id = Some("title/english-label".to_string());
    authoring.semantic_role = Some("english_label".to_string());
    authoring.continuity_group = Some("title_branding".to_string());
    authoring.region_inventory_complete = true;
    authoring.regions.push(GraphicsTextRegion {
        artwork_uses_overlay_alpha: false,
        region_id: "english-label".to_string(),
        semantic_role: "english_label".to_string(),
        bounds: PixelBounds {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        },
        artwork_composition_bounds: None,
        source_restoration_bounds: Vec::new(),
        source_background_restoration: GraphicsSourceBackgroundRestoration::NearestSourceIndex,
        source_fill_color: None,
        source_text: "GAME OVER".to_string(),
        korean_text: Some("게임 오버".to_string()),
        treatment_profile: GraphicsTreatmentProfile::FunctionalUiText,
        artwork_plan: None,
        glyph_demand_source: None,
        source_pixel_mask: HashedAsset {
            path: "assets/translation/graphics/english-label.mask.png".to_string(),
            sha256: "mask-hash".to_string(),
        },
        translator_notes: Vec::new(),
    });

    let error = validate_graphics_translation_bundle(&bundle, inputs(&targets)).unwrap_err();
    assert!(error.to_string().contains("Japanese transcription"));
}
