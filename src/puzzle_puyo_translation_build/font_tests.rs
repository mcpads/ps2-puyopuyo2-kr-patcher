use crate::font_atlas::{FontGlyph, FontGlyphSet, GlyphRectangle};

use super::*;

#[test]
fn protected_pixel_check_excludes_only_assigned_halfwidth_cells() {
    let glyph_set = FontGlyphSet {
        symbol: "FontList_Nazo_Han".to_string(),
        source_encoding: "ASCII subset",
        role: "test halfwidth lookup",
        glyphs: (0..2)
            .map(|index| FontGlyph {
                index,
                source_hex: "20".to_string(),
                character: " ".to_string(),
                rectangle: GlyphRectangle {
                    x: u32::try_from(index).unwrap() * 16,
                    y: 224,
                    width: 16,
                    height: 32,
                },
            })
            .collect(),
    };
    let mut replacement = vec![0_u8; 512 * 256];
    replacement[224 * 512] = 1;
    replacement[224 * 512 + 16] = 2;
    let assignment = ExecutableTextGlyphAssignment {
        character: "?".to_string(),
        code_hex: "3f".to_string(),
        slot_index: 0,
    };

    let (protected, changed) = count_changed_pixels_outside_assignments(
        &vec![0_u8; 512 * 256],
        &replacement,
        &glyph_set,
        &[assignment],
    )
    .unwrap();

    assert_eq!(protected, 16 * 32);
    assert_eq!(changed, 1);
}
