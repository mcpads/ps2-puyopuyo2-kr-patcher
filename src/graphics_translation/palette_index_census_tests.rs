use super::*;

#[test]
fn census_groups_indices_and_colors_inside_the_requested_rectangle() {
    let surface = RgbaSurface {
        width: 4,
        height: 3,
        pixels: vec![
            0, 0, 0, 0, 10, 20, 30, 255, 10, 20, 30, 255, 0, 0, 0, 0, 0, 0, 0, 0, 40, 50, 60, 255,
            10, 20, 30, 255, 0, 0, 0, 0, 0, 0, 0, 0, 40, 50, 60, 255, 40, 50, 60, 255, 0, 0, 0, 0,
        ],
    };
    let indices = vec![0, 7, 7, 0, 0, 8, 7, 0, 0, 8, 8, 0];

    let report = census_palette_indices(
        &surface,
        &indices,
        PixelBounds {
            x: 1,
            y: 0,
            width: 2,
            height: 3,
        },
    )
    .expect("census should succeed");

    assert_eq!(report.len(), 2);
    assert_eq!(report[0].palette_index, 7);
    assert_eq!(report[0].pixel_count, 3);
    assert_eq!(
        report[0].occurrence_bounds,
        PixelBounds {
            x: 1,
            y: 0,
            width: 2,
            height: 2,
        }
    );
    assert_eq!(report[0].colors.len(), 1);
    assert_eq!(report[1].palette_index, 8);
    assert_eq!(report[1].pixel_count, 3);
    assert_eq!(
        report[1].occurrence_bounds,
        PixelBounds {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        }
    );
}

#[test]
fn census_rejects_out_of_bounds_or_mismatched_inputs() {
    let surface = RgbaSurface {
        width: 2,
        height: 2,
        pixels: vec![0; 16],
    };
    let invalid_bounds = PixelBounds {
        x: 1,
        y: 1,
        width: 2,
        height: 1,
    };

    assert!(census_palette_indices(&surface, &[0; 4], invalid_bounds).is_err());
    assert!(
        census_palette_indices(
            &surface,
            &[0; 3],
            PixelBounds {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
        )
        .is_err()
    );
}
