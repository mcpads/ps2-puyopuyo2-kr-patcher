use super::*;

#[test]
fn glyph_mapping_matches_tutorial_renderer_coordinates() {
    let mut source = vec![0x82, 0xa0];
    source.extend(std::iter::repeat_n([0x82, 0xa2], 16).flatten());
    source.push(0);
    let glyphs = lookup::parse_tutorial_glyphs(&source, 512, 256).unwrap();
    assert_eq!(glyphs[0].character, "あ");
    assert_eq!(glyphs[0].rectangle.x, 0);
    assert_eq!(glyphs[0].rectangle.y, 0);
    assert_eq!(glyphs[16].rectangle.x, 0);
    assert_eq!(glyphs[16].rectangle.y, 32);
}

#[test]
fn lookup_summary_preserves_shadowed_duplicate_slots() {
    let summary =
        lookup::summarize_tutorial_slots(&[0x82, 0xa0, 0x81, 0x40, 0x81, 0x40, 0]).unwrap();
    assert_eq!(summary.slots, 3);
    assert_eq!(summary.unique_codes, 2);
    assert_eq!(summary.shadowed_duplicate_slots, 1);
    assert_eq!(summary.duplicates[0].source_hex, "8140");
    assert_eq!(summary.duplicates[0].first_slot, 1);
    assert_eq!(summary.duplicates[0].shadowed_slots, [2]);
}

#[test]
fn glyph_mapping_rejects_a_partial_shift_jis_slot() {
    let error = lookup::parse_tutorial_glyphs(&[0x82, 0], 512, 256)
        .unwrap_err()
        .to_string();
    assert!(error.contains("partial Shift-JIS pair"));
}
