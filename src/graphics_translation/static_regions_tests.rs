use super::{
    StaticRegionCatalog, clear_mask_inside_bounds, dilate_mask_inside_bounds,
    validate_asset_file_component, validate_declared_language_regions,
    validate_source_restoration_bounds,
};
use crate::graphics_translation::validation::validate_region_translation_source;
use crate::graphics_translation::{GlyphDemandSource, GraphicsTreatmentProfile};
use crate::texture_language_survey::TextureLanguageClassification;

#[test]
fn region_ids_cannot_escape_the_mask_directory() {
    assert!(validate_asset_file_component("continue-question").is_ok());
    assert!(validate_asset_file_component("../outside").is_err());
    assert!(validate_asset_file_component("nested/label").is_err());
}

#[test]
fn source_mask_dilation_is_clipped_to_declared_bounds() {
    let mut mask = vec![false; 5 * 5];
    mask[2 * 5 + 2] = true;
    let dilated = dilate_mask_inside_bounds(
        &mask,
        5,
        5,
        crate::graphics_translation::PixelBounds {
            x: 2,
            y: 1,
            width: 2,
            height: 3,
        },
        1,
    )
    .unwrap();
    let selected = dilated
        .iter()
        .enumerate()
        .filter_map(|(index, selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(selected, vec![7, 8, 12, 13, 17, 18]);
}

#[test]
fn protected_pixels_are_removed_after_source_mask_dilation() {
    let mut mask = vec![false; 5 * 5];
    mask[2 * 5 + 2] = true;
    let mut dilated = dilate_mask_inside_bounds(
        &mask,
        5,
        5,
        crate::graphics_translation::PixelBounds {
            x: 1,
            y: 1,
            width: 3,
            height: 3,
        },
        1,
    )
    .unwrap();
    clear_mask_inside_bounds(
        &mut dilated,
        5,
        5,
        crate::graphics_translation::PixelBounds {
            x: 1,
            y: 1,
            width: 3,
            height: 1,
        },
    )
    .unwrap();
    assert_eq!(
        dilated
            .iter()
            .enumerate()
            .filter_map(|(index, selected)| selected.then_some(index))
            .collect::<Vec<_>>(),
        vec![11, 12, 13, 16, 17, 18]
    );
}

#[test]
fn source_restoration_partitions_cover_each_selected_state_without_overlap() {
    let mut mask = vec![false; 6 * 2];
    mask[1] = true;
    mask[4] = true;
    let partitions = [
        crate::graphics_translation::PixelBounds {
            x: 0,
            y: 0,
            width: 3,
            height: 2,
        },
        crate::graphics_translation::PixelBounds {
            x: 3,
            y: 0,
            width: 3,
            height: 2,
        },
    ];
    validate_source_restoration_bounds(&mask, 6, 2, &partitions, "synthetic/states").unwrap();
    assert!(
        validate_source_restoration_bounds(
            &mask,
            6,
            2,
            &partitions[..1],
            "synthetic/missing-state",
        )
        .is_err()
    );
}

#[test]
#[ignore = "requires config/static-graphics-regions.json"]
fn tracked_top_menu_labels_dilate_their_source_masks_but_not_the_prompt() {
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();
    let surface = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == "linked-texture/image-d/000049")
        .unwrap();
    assert_eq!(
        surface
            .regions
            .iter()
            .map(|region| (
                region.region_id.as_str(),
                region.source_mask_dilation_radius
            ))
            .collect::<Vec<_>>(),
        [
            ("solo-mode-labels", 2),
            ("two-player-mode-labels", 2),
            ("options-mode-labels", 2),
            ("training-mode-labels", 2),
            ("mode-selection-prompt", 0),
            ("puzzle-mode-labels", 2),
        ]
    );
    assert_eq!(
        surface
            .regions
            .iter()
            .map(|region| region.protected_subregions.len())
            .collect::<Vec<_>>(),
        [3, 1, 3, 3, 0, 3]
    );
    assert_eq!(
        surface
            .regions
            .iter()
            .map(|region| region.source_restoration_bounds.len())
            .collect::<Vec<_>>(),
        [3, 3, 3, 3, 0, 3]
    );
}

#[test]
#[ignore = "requires config/static-graphics-regions.json"]
fn rule_editor_translation_units_stay_split_at_composed_state_boundaries() {
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    // Original RuleEdit_Ptn source cells, not a snapshot of the authored crop.
    // Tightening a Japanese crop is allowed; crossing into another cell is not.
    assert_regions_fit_consumers(
        &catalog,
        "linked-texture/image-d/000039",
        &[
            ("editor-rule-label", (128, 16, 240, 40)),
            ("editor-creation-status", (128, 16, 240, 40)),
            ("confirmation-score-suffix", (392, 208, 40, 40)),
            ("confirmation-question", (432, 192, 256, 48)),
            ("save-to-card-prompt", (432, 240, 464, 48)),
            ("save-rule-prompt", (432, 288, 336, 48)),
            ("start-game-action", (432, 336, 256, 40)),
            ("create-rule-action", (688, 336, 224, 40)),
            ("action-question", (432, 376, 224, 40)),
            ("copy-action-state", (656, 376, 352, 40)),
            ("rule-number-label", (512, 464, 224, 40)),
            ("right-rule-label", (896, 32, 112, 32)),
        ],
    );
    // D40 display consumption remains unproven; these are source authoring cells.
    assert_source_region_bounds(
        &catalog,
        "linked-texture/image-d/000040",
        &[
            ("action-question-normal", (576, 320, 221, 34)),
            ("action-question-selected", (800, 320, 221, 34)),
            ("create-rule-action-normal", (576, 368, 220, 33)),
            ("create-rule-action-selected", (800, 368, 220, 33)),
            ("start-game-action", (576, 416, 251, 34)),
            ("copy-action-normal", (1, 460, 351, 38)),
            ("copy-action-selected", (352, 460, 352, 38)),
            ("selected-start-game-action", (704, 460, 251, 38)),
        ],
    );
}

type RegionBoundsExpectation<'a> = (&'a str, (u32, u32, u32, u32));

fn assert_regions_fit_consumers(
    catalog: &StaticRegionCatalog,
    source_surface_id: &str,
    expected: &[RegionBoundsExpectation<'_>],
) {
    let surface = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == source_surface_id)
        .unwrap();
    for (region_id, (x, y, width, height)) in expected {
        let region = surface
            .regions
            .iter()
            .find(|region| region.region_id == *region_id)
            .unwrap();
        let bounds = region.bounds;
        assert!(
            bounds.x >= *x
                && bounds.y >= *y
                && bounds.x + bounds.width <= x + width
                && bounds.y + bounds.height <= y + height,
            "{source_surface_id}/{region_id} crosses its original consumer cell"
        );
    }
}

fn assert_source_region_bounds(
    catalog: &StaticRegionCatalog,
    source_surface_id: &str,
    expected: &[RegionBoundsExpectation<'_>],
) {
    let surface = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == source_surface_id)
        .unwrap();
    for (region_id, (x, y, width, height)) in expected {
        let region = surface
            .regions
            .iter()
            .find(|region| region.region_id == *region_id)
            .unwrap();
        assert_eq!(
            (
                region.bounds.x,
                region.bounds.y,
                region.bounds.width,
                region.bounds.height
            ),
            (*x, *y, *width, *height),
            "{source_surface_id}/{region_id} differs from its reviewed source cell"
        );
    }
}

#[test]
fn mixed_surface_requires_an_explicit_english_region() {
    let error = validate_declared_language_regions(
        TextureLanguageClassification::JapaneseEnglishMixed,
        true,
        0,
        "mixed-surface",
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("lacks protected English regions")
    );
    assert!(
        validate_declared_language_regions(
            TextureLanguageClassification::JapaneseEnglishMixed,
            true,
            1,
            "mixed-surface",
        )
        .unwrap()
    );
}

#[test]
fn japanese_only_surface_rejects_an_english_region() {
    let error = validate_declared_language_regions(
        TextureLanguageClassification::JapaneseOnly,
        false,
        1,
        "Japanese-only-surface",
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("declares protected English pixels")
    );
    assert!(
        !validate_declared_language_regions(
            TextureLanguageClassification::JapaneseOnly,
            false,
            0,
            "Japanese-only-surface",
        )
        .unwrap()
    );
}

#[test]
fn non_target_surface_cannot_enter_static_region_inventory() {
    let error = validate_declared_language_regions(
        TextureLanguageClassification::EnglishOnly,
        false,
        0,
        "English-only-surface",
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("is not a Japanese translation target")
    );
}

#[test]
fn font_glyph_supply_rejects_a_manually_copied_korean_demand() {
    let error = validate_region_translation_source(
        "linked-texture/image-d/000022",
        GraphicsTreatmentProfile::FontGlyphSupply,
        Some("번역에서모은글자"),
        Some(GlyphDemandSource::TutorialMessages),
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("must derive demand from its text consumer")
    );
    assert!(
        validate_region_translation_source(
            "linked-texture/image-d/000022",
            GraphicsTreatmentProfile::FontGlyphSupply,
            None,
            Some(GlyphDemandSource::TutorialMessages),
        )
        .is_ok()
    );
}

#[test]
fn directly_translated_graphics_region_requires_a_korean_draft() {
    let error = validate_region_translation_source(
        "linked-texture/image-d/000019",
        GraphicsTreatmentProfile::FontAtlas,
        None,
        None,
    )
    .unwrap_err();

    assert!(error.to_string().contains("has no direct Korean draft"));
    assert!(
        validate_region_translation_source(
            "linked-texture/image-d/000019",
            GraphicsTreatmentProfile::FontAtlas,
            Some("퍼즐뿌요"),
            None,
        )
        .is_ok()
    );
}

#[test]
fn cutscene_graphics_region_derives_korean_wording_from_the_story_ledger() {
    let error = validate_region_translation_source(
        "linked-texture/image-md/000077",
        GraphicsTreatmentProfile::CutsceneBakedText,
        Some("그래픽 원장에 복사된 문장"),
        None,
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("must derive wording from the story ledger")
    );
    assert!(
        validate_region_translation_source(
            "linked-texture/image-md/000077",
            GraphicsTreatmentProfile::CutsceneBakedText,
            None,
            None,
        )
        .is_ok()
    );
}

#[test]
fn preview_variants_can_split_source_consumers() {
    let first = vec!["NormalMenu".to_string()];
    let second = vec!["TsuMenu".to_string()];
    let expected = vec!["TsuMenu".to_string(), "NormalMenu".to_string()];
    super::validate_preview_owner_coverage([first.as_slice(), second.as_slice()], &expected)
        .unwrap();
}

#[test]
fn preview_variants_cannot_omit_or_add_source_consumers() {
    let first = vec!["NormalMenu".to_string()];
    let unrelated = vec!["OtherTexture".to_string()];
    let expected = vec!["NormalMenu".to_string(), "TsuMenu".to_string()];
    assert!(super::validate_preview_owner_coverage([first.as_slice()], &expected).is_err());
    assert!(
        super::validate_preview_owner_coverage([first.as_slice(), unrelated.as_slice()], &expected)
            .is_err()
    );
    assert!(super::validate_preview_owner_coverage([&[][..]], &expected).is_err());
}

#[test]
#[ignore = "requires config/static-graphics-regions.json"]
fn story_data_menu_labels_stay_inside_their_separate_pattern_cells() {
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();
    // Original HitoLoadPtn records: each label is displayed independently.
    assert_regions_fit_consumers(
        &catalog,
        "linked-texture/image-d/000032",
        &[
            ("continue-data-heading", (0, 0, 496, 48)),
            ("return-action", (0, 48, 96, 48)),
            ("data-slot-one", (0, 96, 144, 48)),
            ("data-slot-two", (0, 144, 144, 48)),
            ("data-slot-three", (144, 144, 144, 48)),
            ("erase-data-action", (288, 144, 192, 48)),
            ("start-from-beginning", (272, 96, 192, 48)),
            ("finished-data-label", (144, 96, 128, 48)),
            ("erase-data-heading", (96, 48, 384, 48)),
        ],
    );
}
