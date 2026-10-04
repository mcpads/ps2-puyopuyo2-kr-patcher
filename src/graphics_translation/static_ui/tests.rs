use std::collections::BTreeMap;

use super::super::{GraphicsTreatmentProfile, PixelBounds};
use super::background_restoration::{
    restore_source_index_rows, restore_source_indices, select_uniform_transparent_background_index,
};
use super::layout::{infer_alignment, visible_unselected_pixels};
use super::model::{RegionRestorationInputs, StaticTextDraftScope, StaticUiLineAlignment};
use super::palette::{
    RegionOverlayPalette, select_composition_color_indices, select_preview_colors,
    select_source_color_indices,
};

#[test]
fn deterministic_text_scope_selects_only_functional_regions() {
    assert!(
        StaticTextDraftScope::DeterministicText.selects(GraphicsTreatmentProfile::FunctionalUiText)
    );
    assert!(
        !StaticTextDraftScope::DeterministicText
            .selects(GraphicsTreatmentProfile::StylizedDisplayLettering)
    );
    assert!(!StaticTextDraftScope::DeterministicText.selects(GraphicsTreatmentProfile::FontAtlas));
}

#[test]
fn stylized_palette_mapping_preserves_unique_source_indices() {
    let source_indices = [10, 11];
    let source_mask = [true, true];
    let mut palette = vec![0_u8; 256 * 4];
    for (index, color) in palette.chunks_exact_mut(4).enumerate() {
        color.copy_from_slice(&[index as u8, 0, 0, u8::MAX]);
    }
    let mapping = select_source_color_indices(&source_indices, &palette, &source_mask).unwrap();
    assert_eq!(
        mapping
            .values()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([10, 11])
    );
}

#[test]
fn stylized_palette_mapping_rejects_ambiguous_source_colors() {
    let source_indices = [10, 11];
    let source_mask = [true, true];
    let mut palette = vec![0_u8; 256 * 4];
    for color in palette.chunks_exact_mut(4) {
        color.copy_from_slice(&[1, 2, 3, u8::MAX]);
    }
    assert!(select_source_color_indices(&source_indices, &palette, &source_mask).is_err());
}

#[test]
fn composition_palette_uses_the_most_common_index_for_duplicate_visible_colors() {
    let source_indices = [10, 10, 11];
    let mut palette = vec![0_u8; 256 * 4];
    for color in palette.chunks_exact_mut(4) {
        color.copy_from_slice(&[1, 2, 3, u8::MAX]);
    }
    let mapping = select_composition_color_indices(
        &source_indices,
        &palette,
        3,
        1,
        PixelBounds {
            x: 0,
            y: 0,
            width: 3,
            height: 1,
        },
        None,
    )
    .unwrap();
    assert_eq!(mapping[&[1, 2, 3, u8::MAX]], 10);
}

#[test]
fn composition_palette_limit_keeps_the_most_used_source_colors() {
    let source_indices = [1, 1, 2, 3];
    let mut palette = vec![0_u8; 256 * 4];
    for index in 1..=3 {
        palette[index * 4..index * 4 + 4].copy_from_slice(&[
            u8::try_from(index).unwrap(),
            0,
            0,
            0x80,
        ]);
    }
    let mapping = select_composition_color_indices(
        &source_indices,
        &palette,
        4,
        1,
        PixelBounds {
            x: 0,
            y: 0,
            width: 4,
            height: 1,
        },
        Some(2),
    )
    .unwrap();
    assert_eq!(mapping.values().copied().collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn stylized_overlay_rejects_partial_alpha() {
    let mapping =
        RegionOverlayPalette::ExactSourceColors(BTreeMap::from([([1, 2, 3, u8::MAX], 10)]));
    assert!(mapping.palette_index(&[1, 2, 3, 128]).is_err());
}

#[test]
fn transparent_background_restoration_uses_one_dominant_palette_index() {
    let source_indices = [1, 1, 2, 7];
    let palette = vec![0_u8; 256 * 4];
    let source_masks = [false, false, false, true];
    let protected = [false; 4];
    assert_eq!(
        select_uniform_transparent_background_index(
            &source_indices,
            &palette,
            2,
            PixelBounds {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            &source_masks,
            &protected,
        )
        .unwrap(),
        Some(1)
    );
}

#[test]
fn row_dominant_restoration_uses_each_rows_majority_background() {
    let source_indices = [9, 1, 4, 5, 9, 9, 7, 8, 2, 3, 7, 7];
    let source_mask = [
        false, false, true, true, false, false, false, false, true, true, false, false,
    ];
    let protected = [false; 12];
    let mut replacement = source_indices;
    restore_source_index_rows(
        &mut replacement,
        &source_indices,
        RegionRestorationInputs {
            surface_width: 6,
            surface_height: 2,
            bounds: PixelBounds {
                x: 0,
                y: 0,
                width: 6,
                height: 2,
            },
            source_mask: &source_mask,
            all_source_masks: &source_mask,
            protected_english: &protected,
        },
    )
    .unwrap();
    assert_eq!(replacement, [9, 1, 9, 9, 9, 9, 7, 8, 7, 7, 7, 7]);
}

#[test]
fn explicit_transparent_restoration_preserves_adjacent_art_and_rejects_missing_background() {
    let source = [0, 1, 2, 2];
    let mask = [false, false, true, true];
    let protected = [false; 4];
    let mut palette = vec![255; 1024];
    palette[3] = 0;
    let restore = |palette: &[u8], replacement: &mut [u8]| {
        restore_source_indices(
            replacement,
            &source,
            palette,
            RegionRestorationInputs {
                surface_width: 4,
                surface_height: 1,
                bounds: PixelBounds {
                    x: 0,
                    y: 0,
                    width: 4,
                    height: 1,
                },
                source_mask: &mask,
                all_source_masks: &mask,
                protected_english: &protected,
            },
            super::super::GraphicsSourceBackgroundRestoration::TransparentSourceIndex,
        )
    };
    let mut replacement = source;
    restore(&palette, &mut replacement).unwrap();
    assert_eq!(replacement, [0, 1, 0, 0]);
    palette[3] = 255;
    replacement = source;
    assert!(restore(&palette, &mut replacement).is_err());
    assert_eq!(replacement, source);
    palette[3] = 0;
    palette[7] = 0;
    assert!(restore(&palette, &mut replacement).is_err());
    assert_eq!(replacement, source);
}

#[test]
fn transparent_text_bounds_reject_visible_unselected_pixels() {
    let mut rgba = vec![0; 4 * 4 * 4];
    rgba[(4 + 1) * 4 + 3] = u8::MAX;
    let mask = vec![false; 4 * 4];
    assert_eq!(
        visible_unselected_pixels(
            &rgba,
            4,
            4,
            PixelBounds {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            },
            &mask,
        )
        .unwrap(),
        1
    );
}

#[test]
fn alignment_uses_source_line_margins() {
    let mut mask = vec![false; 10 * 2];
    mask[1] = true;
    mask[2] = true;
    assert_eq!(
        infer_alignment(&mask, 10, 0, 10, 0, 1),
        StaticUiLineAlignment::Left
    );
    let mut mask = vec![false; 10 * 2];
    mask[17] = true;
    mask[18] = true;
    assert_eq!(
        infer_alignment(&mask, 10, 0, 10, 1, 2),
        StaticUiLineAlignment::Right
    );
}

#[test]
fn preview_fill_prefers_the_dominant_ink_over_an_antialias_extreme() {
    let source_rgba = [
        5, 10, 20, 255, 30, 50, 90, 255, 30, 50, 90, 255, 245, 245, 245, 255,
    ];
    let selected = [true, true, true, true];
    assert_eq!(
        select_preview_colors(&source_rgba, &selected).unwrap(),
        ([5, 10, 20, 255], [30, 50, 90, 255])
    );
}

#[test]
fn preview_fill_does_not_reuse_a_dominant_outline_color() {
    let source_rgba = [
        5, 10, 20, 255, 5, 10, 20, 255, 5, 10, 20, 255, 245, 245, 245, 255,
    ];
    let selected = [true, true, true, true];
    assert_eq!(
        select_preview_colors(&source_rgba, &selected).unwrap(),
        ([5, 10, 20, 255], [245, 245, 245, 255])
    );
}

#[test]
fn declared_source_fill_is_shared_by_preview_and_index_composition() {
    let source = [
        16, 16, 15, 255, 101, 1, 0, 255, 101, 1, 0, 255, 232, 1, 1, 255,
    ];
    let mask = [true; 4];
    let fill = [232, 1, 1, 255];
    let mut palette = vec![0; 256 * 4];
    palette[4..8].copy_from_slice(&[16, 16, 15, 128]);
    palette[8..12].copy_from_slice(&[101, 1, 0, 128]);
    palette[12..16].copy_from_slice(&[232, 1, 1, 128]);
    assert_eq!(
        select_preview_colors(&source, &mask).unwrap().1,
        [101, 1, 0, 255]
    );
    let roles =
        super::palette::select_preview_colors_with_fill(&source, &mask, Some(fill)).unwrap();
    let indices =
        super::palette::select_source_role_indices(&[1, 2, 2, 3], &palette, &mask, Some(fill))
            .unwrap();
    assert_eq!(roles, ([16, 16, 15, 255], fill));
    assert_eq!(indices, (1, 3));
    for (color, index) in [(roles.0, indices.0), (roles.1, indices.1)] {
        assert_eq!(
            crate::indexed_texture::colorize_psmt8(&[index], &palette),
            color
        );
    }
    for invalid in [[255, 255, 255, 255], [16, 16, 15, 255], [0, 0, 0, 0]] {
        assert!(
            super::palette::select_preview_colors_with_fill(&source, &mask, Some(invalid)).is_err()
        );
        assert!(
            super::palette::select_source_role_indices(
                &[1, 2, 2, 3],
                &palette,
                &mask,
                Some(invalid)
            )
            .is_err()
        );
    }
    let without_bright_ink = [true, true, true, false];
    assert!(
        super::palette::select_preview_colors_with_fill(&source, &without_bright_ink, Some(fill))
            .is_err()
    );
    assert!(
        super::palette::select_source_role_indices(
            &[1, 2, 2, 3],
            &palette,
            &without_bright_ink,
            Some(fill)
        )
        .is_err()
    );
}

#[test]
fn menu_alignment_keeps_a_shared_anchor_across_source_margins_and_text_lengths() {
    use super::layout::LineLayout;
    for inferred in [
        StaticUiLineAlignment::Left,
        StaticUiLineAlignment::Center,
        StaticUiLineAlignment::Right,
    ] {
        for text_width in [24, 90, 124] {
            let left = LineLayout {
                alignment: Some(StaticUiLineAlignment::Left),
                ..LineLayout::default()
            };
            assert_eq!(
                left.horizontal_position(inferred, 80, 160, text_width),
                (StaticUiLineAlignment::Left, 80)
            );
            let right = LineLayout {
                alignment: Some(StaticUiLineAlignment::Right),
                ..LineLayout::default()
            };
            let (alignment, x) = right.horizontal_position(inferred, 80, 160, text_width);
            assert_eq!(alignment, StaticUiLineAlignment::Right);
            assert_eq!(x + text_width, 240);
        }
    }
    assert_eq!(
        LineLayout::default().horizontal_position(StaticUiLineAlignment::Center, 80, 160, 90),
        (StaticUiLineAlignment::Center, 115)
    );
}
