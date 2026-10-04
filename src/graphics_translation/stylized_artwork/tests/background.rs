use super::*;

#[test]
fn border_background_removal_preserves_enclosed_white_artwork_and_dark_shadow() {
    let mut pixels = [240, 241, 240, 255].repeat(25);
    let pixel = |x: usize, y: usize| (y * 5 + x) * 4;
    for (x, y) in [(2, 1), (1, 2), (3, 2), (2, 3)] {
        pixels[pixel(x, y)..pixel(x, y) + 4].copy_from_slice(&[190, 10, 5, 255]);
    }
    pixels[pixel(2, 2)..pixel(2, 2) + 4].copy_from_slice(&[250, 250, 250, 255]);
    pixels[pixel(0, 2)..pixel(0, 2) + 4].copy_from_slice(&[100, 100, 100, 255]);

    let removed = clear_border_connected_light_neutral(&mut pixels, 5, 5, 220, 18).unwrap();

    assert_eq!(removed, 19);
    assert_eq!(&pixels[pixel(0, 0)..pixel(0, 0) + 4], &[0, 0, 0, 0]);
    assert_eq!(&pixels[pixel(2, 2)..pixel(2, 2) + 4], &[250, 250, 250, 255]);
    assert_eq!(&pixels[pixel(0, 2)..pixel(0, 2) + 4], &[100, 100, 100, 255]);
}

#[test]
fn border_background_removal_rejects_mismatched_dimensions() {
    assert!(clear_border_connected_light_neutral(&mut [0; 8], 2, 2, 220, 18).is_err());
}
