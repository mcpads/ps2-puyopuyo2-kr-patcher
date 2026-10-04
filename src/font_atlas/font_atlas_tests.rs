use super::*;

#[test]
fn fullwidth_mapping_matches_font_print_coordinates() {
    let glyphs = parse_fullwidth_glyphs(&[0x82, 0xa0, 0x82, 0xa2, 0], 512, 512).unwrap();
    assert_eq!(glyphs[0].character, "あ");
    assert_eq!(glyphs[0].rectangle.x, 0);
    assert_eq!(glyphs[0].rectangle.y, 96);
    assert_eq!(glyphs[1].rectangle.x, 24);
}

#[test]
fn halfwidth_mapping_matches_font_print_coordinates() {
    let mut source = vec![b' '; 33];
    source.push(0);
    let glyphs = parse_halfwidth_glyphs(&source, 512, 512).unwrap();
    assert_eq!(glyphs[0].rectangle.x, 12);
    assert_eq!(glyphs[31].rectangle.x, 384);
    assert_eq!(glyphs[32].rectangle.x, 12);
    assert_eq!(glyphs[32].rectangle.y, 48);
}

#[test]
fn glyph_mapping_rejects_an_out_of_bounds_atlas() {
    let error = parse_fullwidth_glyphs(&[0x82, 0xa0, 0], 16, 16)
        .unwrap_err()
        .to_string();
    assert!(error.contains("exceeds the atlas"));
}
