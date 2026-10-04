use super::*;
use crate::graphics_translation::raster::RgbaSurface;

#[test]
fn protected_pixel_check_counts_editable_and_protected_changes_separately() {
    let source = [0, 0, 0, 255, 1, 1, 1, 255, 2, 2, 2, 255];
    let replacement = [0, 0, 0, 255, 9, 9, 9, 255, 8, 8, 8, 255];
    let mask = RgbaSurface {
        width: 3,
        height: 1,
        pixels: vec![255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255],
    };

    assert_eq!(
        verify_protected_pixels(&source, &replacement, &mask).unwrap(),
        (2, 1, 2)
    );
}
