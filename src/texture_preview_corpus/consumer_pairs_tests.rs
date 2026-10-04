use super::*;

fn load(operation: u32, file_record: usize, flags: u32) -> TextureLoadRecord {
    TextureLoadRecord {
        flags,
        operation,
        file_record,
        destination: "0x00000000".to_string(),
    }
}

#[test]
fn registration_pairs_select_exact_image_and_palette_loads() {
    let records = [
        load(1, 10, 0),
        load(1, 11, 0),
        load(2, 50, 0),
        load(2, 51, 2),
    ];
    let pairs = [
        TextureRegistrationPair {
            image_load_index: 1,
            palette_slot: 0,
        },
        TextureRegistrationPair {
            image_load_index: 0,
            palette_slot: 1,
        },
    ];
    assert_eq!(
        map_registration_pairs(2, &records, &pairs).unwrap(),
        [
            PreviewKey {
                file_id: 2,
                image_file_record: 11,
                palette_file_record: 50,
                clear_first_alpha: false,
            },
            PreviewKey {
                file_id: 2,
                image_file_record: 10,
                palette_file_record: 51,
                clear_first_alpha: true,
            },
        ]
    );
}

#[test]
fn registration_pairs_reject_out_of_range_indices() {
    let records = [load(1, 10, 0), load(2, 50, 0)];
    let pair = TextureRegistrationPair {
        image_load_index: 1,
        palette_slot: 0,
    };
    assert!(map_registration_pairs(0, &records, &[pair]).is_err());
}
