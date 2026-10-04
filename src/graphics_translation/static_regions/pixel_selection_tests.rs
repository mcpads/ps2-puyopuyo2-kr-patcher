use super::{ChannelRange, RegionPixelSelection, RgbaColor, select_region_pixels};
use crate::graphics_translation::PixelBounds;
use crate::graphics_translation::raster::{RgbaSurface, write_binary_mask};
use crate::indexed_texture::sha256;

fn surface_with_colors(pixels: &[(usize, usize, [u8; 4])]) -> RgbaSurface {
    let mut surface = RgbaSurface {
        width: 5,
        height: 4,
        pixels: vec![0; 5 * 4 * 4],
    };
    for &(x, y, color) in pixels {
        surface.pixels[(y * surface.width + x) * 4..(y * surface.width + x + 1) * 4]
            .copy_from_slice(&color);
    }
    surface
}

fn pixel_index(surface: &RgbaSurface, x: usize, y: usize) -> usize {
    y * surface.width + x
}

#[test]
fn visible_selection_claims_only_nontransparent_pixels_inside_tight_bounds() {
    let surface = surface_with_colors(&[
        (0, 0, [1, 2, 3, 255]),
        (2, 1, [4, 5, 6, 255]),
        (3, 1, [7, 8, 9, 255]),
        (3, 2, [10, 11, 12, 255]),
    ]);
    let mut claimed = vec![false; surface.width * surface.height];
    let selected = select_region_pixels(
        &surface,
        &vec![0; surface.width * surface.height],
        PixelBounds {
            x: 2,
            y: 1,
            width: 2,
            height: 2,
        },
        &[],
        &RegionPixelSelection::Visible,
        &mut claimed,
        "synthetic/label",
    )
    .unwrap();

    assert!(!selected[0]);
    assert_eq!(selected.iter().filter(|pixel| **pixel).count(), 3);
    assert_eq!(selected, claimed);
}

#[test]
fn exact_rgba_selection_excludes_an_opaque_background_in_the_same_bounds() {
    let text = RgbaColor {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    };
    let surface = surface_with_colors(&[
        (1, 1, [0, 128, 0, 255]),
        (2, 1, [255, 255, 255, 255]),
        (3, 1, [0, 128, 0, 255]),
        (2, 2, [255, 255, 255, 255]),
    ]);
    let mut claimed = vec![false; surface.width * surface.height];
    let selected = select_region_pixels(
        &surface,
        &vec![0; surface.width * surface.height],
        PixelBounds {
            x: 2,
            y: 1,
            width: 1,
            height: 2,
        },
        &[],
        &RegionPixelSelection::ExactRgbaColors { colors: vec![text] },
        &mut claimed,
        "synthetic/text-on-panel",
    )
    .unwrap();

    assert!(selected[pixel_index(&surface, 2, 1)]);
    assert!(selected[pixel_index(&surface, 2, 2)]);
    assert!(!selected[pixel_index(&surface, 1, 1)]);
    assert!(!selected[pixel_index(&surface, 3, 1)]);
}

#[test]
fn near_grayscale_selection_keeps_tinted_letter_edges_but_excludes_a_colored_panel() {
    let surface = surface_with_colors(&[
        (1, 1, [1, 81, 0, 255]),
        (2, 1, [16, 19, 15, 255]),
        (3, 1, [1, 81, 0, 255]),
        (2, 2, [255, 255, 255, 255]),
    ]);
    let mut claimed = vec![false; surface.width * surface.height];
    let selected = select_region_pixels(
        &surface,
        &vec![0; surface.width * surface.height],
        PixelBounds {
            x: 2,
            y: 1,
            width: 1,
            height: 2,
        },
        &[],
        &RegionPixelSelection::NearGrayscale {
            maximum_rgb_delta: 4,
        },
        &mut claimed,
        "synthetic/antialiased-text",
    )
    .unwrap();

    assert!(selected[pixel_index(&surface, 2, 1)]);
    assert!(selected[pixel_index(&surface, 2, 2)]);
    assert!(!selected[pixel_index(&surface, 1, 1)]);
    assert!(!selected[pixel_index(&surface, 3, 1)]);
}

#[test]
fn composite_selection_combines_a_dark_outline_and_warm_fill_without_claiming_blue_background() {
    let surface = surface_with_colors(&[
        (1, 1, [54, 163, 207, 255]),
        (2, 1, [0, 0, 1, 255]),
        (3, 1, [255, 223, 43, 255]),
        (2, 2, [116, 102, 20, 255]),
    ]);
    let mut claimed = vec![false; surface.width * surface.height];
    let selected = select_region_pixels(
        &surface,
        &vec![0; surface.width * surface.height],
        PixelBounds {
            x: 2,
            y: 1,
            width: 2,
            height: 2,
        },
        &[],
        &RegionPixelSelection::AnyOf {
            selections: vec![
                RegionPixelSelection::NearGrayscale {
                    maximum_rgb_delta: 1,
                },
                RegionPixelSelection::RgbaRanges {
                    red: ChannelRange {
                        minimum: 50,
                        maximum: 255,
                    },
                    green: ChannelRange {
                        minimum: 40,
                        maximum: 255,
                    },
                    blue: ChannelRange {
                        minimum: 0,
                        maximum: 80,
                    },
                    alpha: ChannelRange {
                        minimum: 255,
                        maximum: 255,
                    },
                },
            ],
        },
        &mut claimed,
        "synthetic/warm-lettering",
    )
    .unwrap();

    assert!(!selected[pixel_index(&surface, 1, 1)]);
    assert!(selected[pixel_index(&surface, 2, 1)]);
    assert!(selected[pixel_index(&surface, 3, 1)]);
    assert!(selected[pixel_index(&surface, 2, 2)]);
}

#[test]
fn palette_index_selection_distinguishes_same_color_pixels_and_rejects_absent_indices() {
    let surface =
        surface_with_colors(&[(1, 1, [255, 255, 255, 255]), (2, 1, [255, 255, 255, 255])]);
    let mut linear_indices = vec![0; surface.width * surface.height];
    linear_indices[pixel_index(&surface, 1, 1)] = 7;
    linear_indices[pixel_index(&surface, 2, 1)] = 8;
    let bounds = PixelBounds {
        x: 1,
        y: 1,
        width: 1,
        height: 1,
    };
    let selected = select_region_pixels(
        &surface,
        &linear_indices,
        bounds,
        &[],
        &RegionPixelSelection::PaletteIndices { indices: vec![7] },
        &mut vec![false; surface.width * surface.height],
        "synthetic/indexed-text",
    )
    .unwrap();

    assert!(selected[pixel_index(&surface, 1, 1)]);
    assert!(!selected[pixel_index(&surface, 2, 1)]);
    assert!(
        select_region_pixels(
            &surface,
            &linear_indices,
            bounds,
            &[],
            &RegionPixelSelection::PaletteIndices { indices: vec![9] },
            &mut vec![false; surface.width * surface.height],
            "synthetic/absent-index",
        )
        .is_err()
    );
}

#[test]
fn reviewed_pixel_mask_requires_hash_bound_pixels_inside_declared_bounds() {
    let surface = surface_with_colors(&[
        (1, 1, [0, 0, 0, 255]),
        (2, 1, [0, 0, 0, 255]),
        (1, 2, [0, 0, 0, 255]),
    ]);
    let directory = tempfile::tempdir_in("target").unwrap();
    let mask_path = directory.path().join("reviewed.mask.png");
    let mut reviewed = vec![false; surface.width * surface.height];
    reviewed[pixel_index(&surface, 1, 1)] = true;
    reviewed[pixel_index(&surface, 1, 2)] = true;
    write_binary_mask(&mask_path, surface.width, surface.height, &reviewed).unwrap();
    let path = mask_path
        .strip_prefix(std::env::current_dir().unwrap())
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let hash = sha256(&std::fs::read(&mask_path).unwrap());
    let selection = RegionPixelSelection::ReviewedPixelMask {
        path: path.clone(),
        sha256: hash,
    };
    let bounds = PixelBounds {
        x: 1,
        y: 1,
        width: 1,
        height: 2,
    };

    let selected = select_region_pixels(
        &surface,
        &vec![0; surface.width * surface.height],
        bounds,
        &[],
        &selection,
        &mut vec![false; surface.width * surface.height],
        "synthetic/reviewed-mask",
    )
    .unwrap();

    assert_eq!(selected, reviewed);

    // A region-sized mask uses the declared source origin, not atlas (0, 0).
    write_binary_mask(&mask_path, 1, 2, &[true, true]).unwrap();
    let cropped = RegionPixelSelection::ReviewedPixelMask {
        path: path.clone(),
        sha256: sha256(&std::fs::read(&mask_path).unwrap()),
    };
    let selected_crop = select_region_pixels(
        &surface,
        &vec![0; surface.width * surface.height],
        bounds,
        &[],
        &cropped,
        &mut vec![false; surface.width * surface.height],
        "synthetic/reviewed-crop",
    )
    .unwrap();
    assert_eq!(selected_crop, reviewed);
    write_binary_mask(&mask_path, surface.width, surface.height, &reviewed).unwrap();
    let protected = [PixelBounds {
        x: 1,
        y: 1,
        width: 1,
        height: 1,
    }];
    let filtered = select_region_pixels(
        &surface,
        &vec![0; surface.width * surface.height],
        bounds,
        &protected,
        &selection,
        &mut vec![false; surface.width * surface.height],
        "synthetic/reviewed-mask-with-protection",
    )
    .unwrap();
    let mut expected_filtered = reviewed.clone();
    expected_filtered[pixel_index(&surface, 1, 1)] = false;
    assert_eq!(filtered, expected_filtered);

    let changed = RegionPixelSelection::ReviewedPixelMask {
        path,
        sha256: "0".repeat(64),
    };
    assert!(
        select_region_pixels(
            &surface,
            &vec![0; surface.width * surface.height],
            bounds,
            &[],
            &changed,
            &mut vec![false; surface.width * surface.height],
            "synthetic/stale-reviewed-mask",
        )
        .is_err()
    );

    reviewed[pixel_index(&surface, 0, 0)] = true;
    write_binary_mask(&mask_path, surface.width, surface.height, &reviewed).unwrap();
    let outside_bounds = RegionPixelSelection::ReviewedPixelMask {
        path: mask_path
            .strip_prefix(std::env::current_dir().unwrap())
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        sha256: sha256(&std::fs::read(&mask_path).unwrap()),
    };
    assert!(
        select_region_pixels(
            &surface,
            &vec![0; surface.width * surface.height],
            bounds,
            &[],
            &outside_bounds,
            &mut vec![false; surface.width * surface.height],
            "synthetic/outside-reviewed-mask",
        )
        .is_err()
    );
}

#[test]
fn protected_subregion_excludes_art_that_reuses_a_text_palette_index() {
    let surface = surface_with_colors(&[
        (1, 1, [255, 0, 0, 255]),
        (2, 1, [255, 0, 0, 255]),
        (3, 2, [255, 0, 0, 255]),
    ]);
    let mut linear_indices = vec![0; surface.width * surface.height];
    linear_indices[pixel_index(&surface, 1, 1)] = 7;
    linear_indices[pixel_index(&surface, 2, 1)] = 7;
    linear_indices[pixel_index(&surface, 3, 2)] = 7;
    let selected = select_region_pixels(
        &surface,
        &linear_indices,
        PixelBounds {
            x: 1,
            y: 1,
            width: 3,
            height: 2,
        },
        &[PixelBounds {
            x: 2,
            y: 1,
            width: 1,
            height: 1,
        }],
        &RegionPixelSelection::PaletteIndices { indices: vec![7] },
        &mut vec![false; surface.width * surface.height],
        "synthetic/shared-index-art",
    )
    .unwrap();

    assert!(selected[pixel_index(&surface, 1, 1)]);
    assert!(!selected[pixel_index(&surface, 2, 1)]);
    assert!(selected[pixel_index(&surface, 3, 2)]);
}

#[test]
fn exact_rgba_selection_rejects_a_stale_or_duplicate_color_declaration() {
    let surface = surface_with_colors(&[(1, 1, [255, 255, 255, 255])]);
    let white = RgbaColor {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    };
    let absent = RgbaColor {
        red: 255,
        green: 0,
        blue: 0,
        alpha: 255,
    };
    let bounds = PixelBounds {
        x: 1,
        y: 1,
        width: 1,
        height: 1,
    };

    assert!(
        select_region_pixels(
            &surface,
            &vec![0; surface.width * surface.height],
            bounds,
            &[],
            &RegionPixelSelection::ExactRgbaColors {
                colors: vec![white, absent],
            },
            &mut vec![false; surface.width * surface.height],
            "stale",
        )
        .is_err()
    );
    assert!(
        select_region_pixels(
            &surface,
            &vec![0; surface.width * surface.height],
            bounds,
            &[],
            &RegionPixelSelection::ExactRgbaColors {
                colors: vec![white, white],
            },
            &mut vec![false; surface.width * surface.height],
            "duplicate",
        )
        .is_err()
    );
}

#[test]
fn overlapping_selected_pixels_are_rejected() {
    let surface = surface_with_colors(&[(1, 1, [255, 255, 255, 255])]);
    let mut claimed = vec![false; surface.width * surface.height];
    let bounds = PixelBounds {
        x: 1,
        y: 1,
        width: 1,
        height: 1,
    };
    select_region_pixels(
        &surface,
        &vec![0; surface.width * surface.height],
        bounds,
        &[],
        &RegionPixelSelection::Visible,
        &mut claimed,
        "first",
    )
    .unwrap();

    assert!(
        select_region_pixels(
            &surface,
            &vec![0; surface.width * surface.height],
            bounds,
            &[],
            &RegionPixelSelection::Visible,
            &mut claimed,
            "second",
        )
        .is_err()
    );
}
