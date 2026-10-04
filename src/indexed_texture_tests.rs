use super::*;
use crate::cpx::encode_cpx_block;

#[test]
fn unswizzle_matches_the_fixed_sixteen_by_four_address_map() {
    let swizzled: Vec<u8> = (0..64).collect();
    let linear = unswizzle_psmt8_uploaded_as_psmct32(&swizzled, 16, 4).unwrap();
    assert_eq!(
        linear,
        [
            0, 4, 8, 12, 16, 20, 24, 28, 2, 6, 10, 14, 18, 22, 26, 30, 32, 36, 40, 44, 48, 52, 56,
            60, 34, 38, 42, 46, 50, 54, 58, 62, 17, 21, 25, 29, 1, 5, 9, 13, 19, 23, 27, 31, 3, 7,
            11, 15, 49, 53, 57, 61, 33, 37, 41, 45, 51, 55, 59, 63, 35, 39, 43, 47,
        ]
    );
}

#[test]
fn swizzle_and_unswizzle_are_exact_inverses() {
    let linear: Vec<u8> = (0..64).collect();
    let swizzled = swizzle_psmt8_uploaded_as_psmct32(&linear, 16, 4).unwrap();
    assert_eq!(
        unswizzle_psmt8_uploaded_as_psmct32(&swizzled, 16, 4).unwrap(),
        linear
    );
}

#[test]
fn cpx_offset_decoder_recovers_exact_linear_palette_indices() {
    let linear: Vec<u8> = (0..64).collect();
    let swizzled = swizzle_psmt8_uploaded_as_psmct32(&linear, 16, 4).unwrap();
    let encoded = encode_cpx_block(&swizzled).unwrap();
    let mut container = vec![0; 32];
    container.extend_from_slice(&encoded);

    assert_eq!(
        decode_linear_indices_at_cpx_offset(&container, 32, 16, 4).unwrap(),
        linear
    );
}

#[test]
fn csm1_lookup_uses_the_uploaded_palette_swap() {
    let mut palette = Vec::with_capacity(PALETTE_BYTES);
    for index in 0..=255_u8 {
        palette.extend_from_slice(&[index, 0, 0, 0x80]);
    }
    let rgba = colorize_psmt8(&[0, 8, 16, 24, 40, 48], &palette);
    assert_eq!(
        rgba,
        [
            0, 0, 0, 255, 16, 0, 0, 255, 8, 0, 0, 255, 24, 0, 0, 255, 48, 0, 0, 255, 40, 0, 0, 255,
        ]
    );
}

#[test]
fn load_records_require_a_single_final_terminator() {
    let mut source = Vec::new();
    source.extend_from_slice(&1_u32.to_le_bytes());
    source.extend_from_slice(&1_u32.to_le_bytes());
    source.extend_from_slice(&2_u32.to_le_bytes());
    source.extend_from_slice(&3_u32.to_le_bytes());
    source.extend_from_slice(&u32::MAX.to_le_bytes());
    source.extend_from_slice(&[0; 12]);
    let records = parse_load_records(&source).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].file_record, 2);
}

#[test]
fn texture_registration_pairs_preserve_consumer_order() {
    let mut source = Vec::new();
    source.extend_from_slice(&25_u32.to_le_bytes());
    source.extend_from_slice(&7_u32.to_le_bytes());
    source.extend_from_slice(&u32::MAX.to_le_bytes());
    source.extend_from_slice(&u32::MAX.to_le_bytes());
    let pairs = parse_texture_registration_pairs("FontTexReg", &source).unwrap();
    assert_eq!(
        pairs,
        [TextureRegistrationPair {
            image_load_index: 25,
            palette_slot: 7,
        }]
    );
}

#[test]
fn image_sentinel_registration_pairs_accept_a_four_byte_terminator() {
    let mut source = Vec::new();
    source.extend_from_slice(&1_u32.to_le_bytes());
    source.extend_from_slice(&2_u32.to_le_bytes());
    source.extend_from_slice(&4_u32.to_le_bytes());
    source.extend_from_slice(&5_u32.to_le_bytes());
    source.extend_from_slice(&u32::MAX.to_le_bytes());
    let pairs = parse_image_sentinel_texture_registration_pairs("DemoTex", &source).unwrap();
    assert_eq!(
        pairs,
        [
            TextureRegistrationPair {
                image_load_index: 1,
                palette_slot: 2,
            },
            TextureRegistrationPair {
                image_load_index: 4,
                palette_slot: 5,
            },
        ]
    );
}

#[test]
fn image_sentinel_registration_pairs_reject_fields_after_the_sentinel() {
    let mut source = Vec::new();
    source.extend_from_slice(&1_u32.to_le_bytes());
    source.extend_from_slice(&2_u32.to_le_bytes());
    source.extend_from_slice(&u32::MAX.to_le_bytes());
    source.extend_from_slice(&0_u32.to_le_bytes());
    assert!(parse_image_sentinel_texture_registration_pairs("DemoTex", &source).is_err());
}

#[test]
fn unchanged_replacement_pixels_keep_their_exact_palette_indices() {
    let mut palette = vec![0; 256 * 4];
    palette[4..8].copy_from_slice(&[10, 20, 30, 0x80]);
    palette[8..12].copy_from_slice(&[40, 50, 60, 0x80]);
    let source_indices = [1, 2, 1];
    let source_rgba = colorize_psmt8(&source_indices, &palette);

    let (indices, changed, preserved) =
        map_rgba_to_source_palette(&source_indices, &source_rgba, &source_rgba, &palette).unwrap();

    assert_eq!(indices, source_indices);
    assert_eq!(changed, 0);
    assert_eq!(preserved, 3);
}

#[test]
fn changed_replacement_pixels_use_a_palette_index_seen_in_the_source() {
    let mut palette = vec![0; 256 * 4];
    palette[4..8].copy_from_slice(&[10, 20, 30, 0x80]);
    palette[8..12].copy_from_slice(&[40, 50, 60, 0x80]);
    let source_indices = [1, 2];
    let source_rgba = colorize_psmt8(&source_indices, &palette);
    let replacement_rgba = colorize_psmt8(&[2, 2], &palette);

    let (indices, changed, preserved) =
        map_rgba_to_source_palette(&source_indices, &source_rgba, &replacement_rgba, &palette)
            .unwrap();

    assert_eq!(indices, [2, 2]);
    assert_eq!(changed, 1);
    assert_eq!(preserved, 1);
}
