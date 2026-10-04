use super::*;

fn surface(width: usize, height: usize, background: [u8; 4]) -> RgbaSurface {
    RgbaSurface {
        width,
        height,
        pixels: background.repeat(width * height),
    }
}

fn set_pixel(surface: &mut RgbaSurface, x: usize, y: usize, color: [u8; 4]) {
    let offset = (y * surface.width + x) * 4;
    surface.pixels[offset..offset + 4].copy_from_slice(&color);
}

#[test]
fn gold_selection_excludes_neutral_background_pixels() {
    let mut source = surface(12, 8, [70, 90, 90, 255]);
    set_pixel(&mut source, 2, 3, [255, 220, 0, 255]);
    set_pixel(&mut source, 7, 5, [120, 90, 10, 255]);
    set_pixel(&mut source, 4, 4, [120, 120, 120, 255]);

    let selected = select_gold_pixels(
        &source,
        PixelBounds {
            x: 1,
            y: 2,
            width: 8,
            height: 5,
        },
    )
    .unwrap();

    assert_eq!(selected.iter().filter(|value| **value).count(), 2);
    assert!(selected[3 * source.width + 2]);
    assert!(selected[5 * source.width + 7]);
    assert!(!selected[4 * source.width + 4]);
}

#[test]
fn dilated_gold_mask_covers_the_label_outline_without_leaving_its_window() {
    let mut seeds = vec![false; 10 * 8];
    seeds[3 * 10 + 2] = true;
    seeds[5 * 10 + 7] = true;
    let limit = PixelBounds {
        x: 1,
        y: 2,
        width: 8,
        height: 5,
    };
    let selected = dilate_mask(&seeds, 10, 8, 2, limit).unwrap();
    let bounds = selected_bounds(&selected, 10, 8, "test label").unwrap();

    assert_eq!(
        bounds,
        PixelBounds {
            x: 1,
            y: 2,
            width: 8,
            height: 5,
        }
    );
}

#[test]
fn japanese_and_protected_masks_form_an_exact_partition() {
    let mut japanese = vec![false; 6 * 4];
    for y in 1..3 {
        for x in 1..4 {
            japanese[y * 6 + x] = true;
        }
    }
    let protected = japanese
        .iter()
        .map(|selected| !selected)
        .collect::<Vec<_>>();
    let partition = summarize_partition(&japanese, &protected).unwrap();

    assert_eq!(partition.japanese_region_pixels, 6);
    assert_eq!(partition.protected_pixels, 18);
    assert_eq!(partition.overlap_pixels, 0);
    assert_eq!(partition.uncovered_pixels, 0);
}

#[test]
fn selected_bounds_rejects_an_empty_label_mask() {
    let error = selected_bounds(&[false; 16], 4, 4, "test label")
        .unwrap_err()
        .to_string();
    assert!(error.contains("mask is empty"));
}

#[test]
fn dilation_does_not_cross_a_neighboring_label_window() {
    let mut seeds = vec![false; 12 * 5];
    seeds[2 * 12 + 5] = true;
    let dilated = dilate_mask(
        &seeds,
        12,
        5,
        3,
        PixelBounds {
            x: 0,
            y: 0,
            width: 6,
            height: 5,
        },
    )
    .unwrap();
    assert!(
        dilated
            .iter()
            .enumerate()
            .all(|(index, selected)| !*selected || index % 12 < 6)
    );
}

#[test]
fn source_label_erasure_uses_a_clean_pixel_from_the_same_column() {
    let mut source = surface(5, 5, [8, 9, 10, 255]);
    set_pixel(&mut source, 2, 2, [255, 220, 0, 255]);
    let mut mask = vec![false; 25];
    mask[2 * 5 + 2] = true;
    let mut output = source.clone();

    let erased = erase_source_label(
        &source,
        &mut output,
        &mask,
        PixelBounds {
            x: 0,
            y: 0,
            width: 5,
            height: 5,
        },
    )
    .unwrap();

    assert_eq!(erased, 1);
    assert_eq!(
        &output.pixels[(2 * 5 + 2) * 4..(2 * 5 + 3) * 4],
        &[8, 9, 10, 255]
    );
}

#[test]
fn changed_pixel_report_separates_editable_and_protected_changes() {
    let source = surface(4, 2, [0, 0, 0, 255]);
    let mut output = source.clone();
    set_pixel(&mut output, 1, 0, [1, 1, 1, 255]);
    set_pixel(&mut output, 3, 1, [2, 2, 2, 255]);
    let editable = windows_mask(
        4,
        2,
        &[PixelBounds {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        }],
    )
    .unwrap();

    assert_eq!(
        changed_pixel_counts(&source, &output, &editable).unwrap(),
        (2, 1)
    );
}
