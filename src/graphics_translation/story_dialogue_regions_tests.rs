use super::{RgbaSurface, partition_story_visible_pixels, visible_pixel_mask};
use crate::graphics_translation::raster::selected_bounds;
use crate::texture_language_survey::TextureLanguageClassification;

#[test]
fn visible_pixels_define_the_editable_story_bounds() {
    let mut surface = RgbaSurface {
        width: 4,
        height: 3,
        pixels: vec![0; 4 * 3 * 4],
    };
    for (x, y) in [(1_usize, 0_usize), (2, 0), (2, 1)] {
        surface.pixels[(y * surface.width + x) * 4 + 3] = 255;
    }
    let mask = visible_pixel_mask(&surface).unwrap();
    assert_eq!(mask.iter().filter(|selected| **selected).count(), 3);
    assert_eq!(
        selected_bounds(&mask, surface.width, surface.height, "synthetic").unwrap(),
        crate::graphics_translation::PixelBounds {
            x: 1,
            y: 0,
            width: 2,
            height: 2,
        }
    );
}

#[test]
fn fully_opaque_surface_is_not_admitted_as_story_text() {
    let surface = RgbaSurface {
        width: 2,
        height: 2,
        pixels: vec![255; 2 * 2 * 4],
    };
    assert!(visible_pixel_mask(&surface).is_err());
}

#[test]
fn fullwidth_ok_cells_are_removed_from_the_japanese_source_mask() {
    let width = 288;
    let height = 168;
    let mut surface = RgbaSurface {
        width,
        height,
        pixels: vec![0; width * height * 4],
    };
    let mut make_visible = |x: usize, y: usize| {
        surface.pixels[(y * width + x) * 4 + 3] = u8::MAX;
    };
    for row in 0..6 {
        make_visible(3, row * 24 + 8);
    }
    make_visible(3, 144 + 8);
    make_visible(150, 144 + 8);
    make_visible(174, 144 + 8);
    make_visible(198, 144 + 8);
    let visible = visible_pixel_mask(&surface).unwrap();

    let partition = partition_story_visible_pixels(
        &surface,
        &visible,
        "あ\nあ\nあ\nあ\nあ\nあ\nむっ。これでＯＫだ",
        TextureLanguageClassification::JapaneseEnglishMixed,
        "story-dialogue/demo-12",
    )
    .unwrap();

    assert!(partition.japanese_text[152 * width + 3]);
    assert!(partition.protected_english[152 * width + 150]);
    assert!(partition.protected_english[152 * width + 174]);
    assert!(!partition.japanese_text[152 * width + 150]);
    assert!(!partition.protected_english[152 * width + 198]);
    for (index, selected) in partition.protected_english.iter().enumerate() {
        if *selected {
            let x = index % width;
            let y = index / width;
            assert!((144..192).contains(&x));
            assert!((144..168).contains(&y));
        }
    }
    assert!((0..width * height).all(|index| {
        usize::from(partition.japanese_text[index])
            + usize::from(partition.protected_english[index])
            + usize::from(partition.protected_nontext[index])
            == 1
    }));
}

#[test]
fn mixed_story_surface_without_fixed_cell_english_is_rejected() {
    let surface = RgbaSurface {
        width: 24,
        height: 24,
        pixels: vec![u8::MAX; 24 * 24 * 4],
    };
    let visible = vec![true; 24 * 24];

    let error = partition_story_visible_pixels(
        &surface,
        &visible,
        "あ",
        TextureLanguageClassification::JapaneseEnglishMixed,
        "mixed",
    )
    .err()
    .expect("mixed surface without English must fail");

    assert!(error.to_string().contains("no fixed-cell English"));
}

#[test]
fn fullwidth_digits_remain_part_of_japanese_story_text() {
    let surface = RgbaSurface {
        width: 24,
        height: 24,
        pixels: vec![0; 24 * 24 * 4],
    };
    let mut visible = vec![false; 24 * 24];
    visible[8 * 24 + 8] = true;

    let partition = partition_story_visible_pixels(
        &surface,
        &visible,
        "１",
        TextureLanguageClassification::JapaneseOnly,
        "Japanese digit",
    )
    .unwrap();

    assert!(partition.japanese_text[8 * 24 + 8]);
    assert!(!partition.protected_english[8 * 24 + 8]);
}
