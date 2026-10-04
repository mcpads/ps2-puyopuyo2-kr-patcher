use super::*;

fn record(flags: u32, operation: u32, file_record: usize) -> TextureLoadRecord {
    TextureLoadRecord {
        flags,
        operation,
        file_record,
        destination: "0x00000000".to_string(),
    }
}

#[test]
fn load_order_pairs_preserve_image_palette_order_and_alpha_flag() {
    let records = [
        record(1, 1, 10),
        record(0, 1, 11),
        record(0, 0, 90),
        record(2, 2, 50),
        record(0, 2, 51),
    ];
    let pairs = plan_load_order_pairs(1, &records).unwrap();
    assert_eq!(
        pairs,
        [
            PreviewKey {
                file_id: 1,
                image_file_record: 10,
                palette_file_record: 50,
                clear_first_alpha: true,
            },
            PreviewKey {
                file_id: 1,
                image_file_record: 11,
                palette_file_record: 51,
                clear_first_alpha: false,
            },
        ]
    );
}

#[test]
fn load_order_pairs_leave_mismatched_tables_for_consumer_analysis() {
    let records = [record(0, 1, 10), record(0, 1, 11), record(0, 2, 50)];
    assert!(plan_load_order_pairs(2, &records).is_none());
}
