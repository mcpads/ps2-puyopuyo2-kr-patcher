use super::*;
use crate::indexed_texture::write_rgba_png;

#[test]
fn rgb_png_is_read_as_opaque_rgba_pixels() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("rgb.png");
    let file = File::create(&path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 2, 1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&[1, 2, 3, 4, 5, 6]).unwrap();
    drop(writer);

    let surface = read_rgba_surface(&path).unwrap();
    assert_eq!(surface.width, 2);
    assert_eq!(surface.height, 1);
    assert_eq!(surface.pixels, [1, 2, 3, 255, 4, 5, 6, 255]);
}

#[test]
fn binary_mask_creates_parent_directory_and_preserves_selection() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("nested/mask.png");
    write_binary_mask(&path, 2, 2, &[true, false, false, true]).unwrap();

    let surface = read_rgba_surface(&path).unwrap();
    assert_eq!(surface.pixels[0..4], [255, 255, 255, 255]);
    assert_eq!(surface.pixels[4..8], [0, 0, 0, 255]);
    assert_eq!(surface.pixels[12..16], [255, 255, 255, 255]);
}

#[test]
fn rgba_reader_preserves_written_pixels() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("rgba.png");
    let pixels = [1, 2, 3, 4, 5, 6, 7, 8];
    write_rgba_png(&path, 2, 1, &pixels).unwrap();

    assert_eq!(read_rgba_surface(&path).unwrap().pixels, pixels);
}

#[test]
fn binary_mask_rejects_a_dimension_mismatch() {
    let temporary = tempfile::tempdir().unwrap();
    let error = write_binary_mask(&temporary.path().join("mask.png"), 2, 2, &[true])
        .unwrap_err()
        .to_string();
    assert!(error.contains("mask dimensions changed"));
}
