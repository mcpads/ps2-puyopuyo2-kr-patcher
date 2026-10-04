use super::*;

#[test]
fn fullwidth_mapping_matches_nazo_renderer_coordinates() {
    let mut source = vec![0x82, 0xa0];
    source.extend(std::iter::repeat_n([0x82, 0xa2], 16).flatten());
    source.push(0);
    let glyphs = parse_nazo_fullwidth_glyphs(&source, 512, 256).unwrap();
    assert_eq!(glyphs[0].character, "あ");
    assert_eq!(glyphs[0].rectangle.x, 0);
    assert_eq!(glyphs[0].rectangle.y, 0);
    assert_eq!(glyphs[16].rectangle.x, 0);
    assert_eq!(glyphs[16].rectangle.y, 32);
}

#[test]
fn halfwidth_mapping_uses_the_final_atlas_row() {
    let mut source = vec![b' '; 32];
    source.push(0);
    let glyphs = parse_nazo_halfwidth_glyphs(&source, 512, 256).unwrap();
    assert_eq!(glyphs[0].rectangle.x, 0);
    assert_eq!(glyphs[0].rectangle.y, 224);
    assert_eq!(glyphs[31].rectangle.x, 496);
    assert_eq!(glyphs[31].rectangle.y, 224);
}

#[test]
fn lookup_summary_distinguishes_first_match_from_shadowed_slots() {
    let summary =
        lookup::summarize_lookup_slots(&[0x82, 0xa0, 0x82, 0xa2, 0x82, 0xa0, 0], 2).unwrap();
    assert_eq!(summary.slots, 3);
    assert_eq!(summary.unique_codes, 2);
    assert_eq!(summary.shadowed_duplicate_slots, 1);
    assert_eq!(summary.duplicates[0].source_hex, "82a0");
    assert_eq!(summary.duplicates[0].first_slot, 0);
    assert_eq!(summary.duplicates[0].shadowed_slots, [2]);
}
