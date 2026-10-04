use super::super::protected_english::plan_protected_english_row_for_test;
use super::compositor::{
    StoryDialogueLinePlacement, clear_source_text, place_story_dialogue_line,
    verify_story_replacement_pixels,
};
use super::palette::{StoryDialoguePaletteIndices, select_story_dialogue_palette_indices};
use crate::story_dialogue_translation::font::{
    RasterizedStoryDialogueLine, StoryDialogueGlyphPixel,
};

#[test]
fn source_only_pixels_are_cleared_while_pixels_outside_both_text_masks_stay_unchanged() {
    let source = vec![1, 1, 0, 7];
    let source_mask = vec![true, true, false, false];
    let mut replacement = source.clone();
    clear_source_text(&mut replacement, &source_mask, 0).unwrap();
    let mut korean_mask = vec![false; source.len()];
    let protected_english = vec![false; source.len()];
    let line = RasterizedStoryDialogueLine {
        width: 2,
        height: 1,
        pixels: vec![
            StoryDialogueGlyphPixel::Outline,
            StoryDialogueGlyphPixel::Fill,
        ],
    };
    place_story_dialogue_line(
        &mut replacement,
        &mut korean_mask,
        StoryDialogueLinePlacement {
            surface_width: 4,
            surface_height: 1,
            x: 1,
            y: 0,
            line: &line,
            outline_index: 1,
            fill_index: 2,
            protected_pixel_mask: &protected_english,
        },
    )
    .unwrap();

    let counts = verify_story_replacement_pixels(
        &source,
        &replacement,
        &source_mask,
        &korean_mask,
        &protected_english,
        0,
    )
    .unwrap();

    assert_eq!(replacement, [0, 1, 2, 7]);
    assert_eq!(counts.protected_pixels, 1);
    assert_eq!(counts.changed_protected_pixels, 0);
    assert_eq!(counts.uncleared_source_only_pixels, 0);
}

#[test]
fn protected_pixel_changes_are_rejected() {
    let source = [1, 7];
    let replacement = [0, 6];
    let source_mask = [true, false];
    let korean_mask = [false, false];
    let protected_english = [false, false];

    let error = verify_story_replacement_pixels(
        &source,
        &replacement,
        &source_mask,
        &korean_mask,
        &protected_english,
        0,
    )
    .unwrap_err();

    assert!(error.to_string().contains("changed protected pixels"));
}

#[test]
fn protected_english_pixels_remain_exact_and_cannot_become_korean_pixels() {
    let source = [1, 7, 8, 0];
    let source_mask = [true, false, false, false];
    let korean_mask = [false, false, false, false];
    let protected_english = [false, true, true, false];
    let replacement = [0, 7, 8, 0];

    let counts = verify_story_replacement_pixels(
        &source,
        &replacement,
        &source_mask,
        &korean_mask,
        &protected_english,
        0,
    )
    .unwrap();

    assert_eq!(counts.protected_english_pixels, 2);
    assert_eq!(counts.changed_protected_english_pixels, 0);

    let changed_english = [0, 7, 6, 0];
    assert!(
        verify_story_replacement_pixels(
            &source,
            &changed_english,
            &source_mask,
            &korean_mask,
            &protected_english,
            0,
        )
        .is_err()
    );
    let overlapping_korean = [false, true, false, false];
    assert!(
        verify_story_replacement_pixels(
            &source,
            &replacement,
            &source_mask,
            &overlapping_korean,
            &protected_english,
            0,
        )
        .is_err()
    );
}

#[test]
fn line_placement_rejects_a_visible_glyph_on_a_protected_english_pixel() {
    let mut replacement = vec![7];
    let mut korean_mask = vec![false];
    let protected_english = vec![true];
    let line = RasterizedStoryDialogueLine {
        width: 1,
        height: 1,
        pixels: vec![StoryDialogueGlyphPixel::Fill],
    };

    let error = place_story_dialogue_line(
        &mut replacement,
        &mut korean_mask,
        StoryDialogueLinePlacement {
            surface_width: 1,
            surface_height: 1,
            x: 0,
            y: 0,
            line: &line,
            outline_index: 1,
            fill_index: 2,
            protected_pixel_mask: &protected_english,
        },
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("overlaps a protected source pixel")
    );
    assert_eq!(replacement, [7]);
    assert_eq!(korean_mask, [false]);
}

#[test]
fn korean_fragments_are_placed_around_the_source_ok_cells() {
    let width = 288;
    let height = 168;
    let mut protected_english = vec![false; width * height];
    protected_english[(144 + 8) * width + 150] = true;
    protected_english[(144 + 8) * width + 174] = true;

    let slots = plan_protected_english_row_for_test(
        "むっ。これでＯＫだわさ。",
        "음. 이걸로 ＯＫ다와사",
        &protected_english,
        width,
        height,
        6,
        288,
    )
    .unwrap();

    assert_eq!(
        slots,
        vec![
            ("음. 이걸로 ".to_string(), 0, 144),
            ("다와사".to_string(), 192, 288),
        ]
    );
}

#[test]
fn palette_roles_are_selected_from_colors_used_by_the_source_texture() {
    let mut palette = vec![0; 256 * 4];
    palette[4..8].copy_from_slice(&[0, 0, 1, 0x80]);
    palette[8..12].copy_from_slice(&[255, 255, 255, 0x80]);

    let selected = select_story_dialogue_palette_indices(&[0, 0, 1, 2], &palette).unwrap();

    assert_eq!(
        selected,
        StoryDialoguePaletteIndices {
            transparent: 0,
            outline: 1,
            fill: 2,
        }
    );
}
