use super::*;

#[test]
fn parser_preserves_consumer_order_and_rectangles() {
    let mut source = Vec::new();
    for word in [0_u32, 2, 16, 32, 48, 64] {
        source.extend_from_slice(&word.to_le_bytes());
    }
    let records = parse_pattern_records("MenuPtn", &source).unwrap();
    assert_eq!(
        records,
        [TexturePatternRecord {
            image_load_index: 2,
            source_x: 16,
            source_y: 32,
            width: 48,
            height: 64,
        }]
    );
}

#[test]
fn rectangles_reject_pixels_outside_the_loaded_image() {
    let records = [TexturePatternRecord {
        image_load_index: 2,
        source_x: 500,
        source_y: 0,
        width: 32,
        height: 32,
    }];
    let error = validate_pattern_rectangles("menu pattern", 2, 512, 512, records.iter())
        .unwrap_err()
        .to_string();
    assert!(error.contains("exceeds image load 2"));
}

#[test]
fn indices_reject_a_reordered_consumer_record() {
    let records = [
        TexturePatternRecord {
            image_load_index: 1,
            source_x: 0,
            source_y: 0,
            width: 1,
            height: 1,
        },
        TexturePatternRecord {
            image_load_index: 0,
            source_x: 0,
            source_y: 0,
            width: 1,
            height: 1,
        },
    ];
    let error = validate_pattern_indices("MenuPtn", &records, &[0, 1])
        .unwrap_err()
        .to_string();
    assert!(error.contains("texture indices changed"));
}
