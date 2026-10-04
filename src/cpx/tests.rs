use super::*;

fn block(decoded_length: usize, stream: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::from(CPX_MAGIC.as_slice());
    bytes.extend_from_slice(&(decoded_length as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(stream);
    bytes
}

fn padded_block(decoded: &[u8], stream: &[u8]) -> Vec<u8> {
    let mut bytes = block(decoded.len(), stream);
    let length = align_up(bytes.len(), CPX_BLOCK_ALIGNMENT).unwrap();
    bytes.resize(length, 0);
    bytes
}

fn file_list_record(fields: [u32; 5]) -> Vec<u8> {
    fields.into_iter().flat_map(u32::to_le_bytes).collect()
}

#[test]
fn decodes_each_command_shape() {
    let stream = [
        0x45, b'a', b'b', b'c', b'd', b'e', 0x03, 0x05, 0x10, 0x0a, 0x00, 0x20, 0x03, 0x0c, 0x30,
        0x04, 0x0f, 0x00, 0xa2, b'X', b'Y', 0x02, 0x60, 0x03, b'x', b'y', b'z', 0x00,
    ];
    let decoded = decode_cpx_block(&block(28, &stream)).unwrap();
    assert_eq!(decoded.bytes, b"abcdeabcdeababcabcdXYXYXYxyz");
    assert_eq!(decoded.commands.short_literal, 1);
    assert_eq!(decoded.commands.short_match_8, 1);
    assert_eq!(decoded.commands.short_match_16, 1);
    assert_eq!(decoded.commands.extended_match_8, 1);
    assert_eq!(decoded.commands.extended_match_16, 1);
    assert_eq!(decoded.commands.literal_then_match, 1);
    assert_eq!(decoded.commands.extended_literal, 1);
}

#[test]
fn match_copy_can_overlap_its_own_output() {
    let decoded = decode_cpx_block(&block(5, &[0x41, b'A', 0x02, 0x01, 0x00])).unwrap();
    assert_eq!(decoded.bytes, b"AAAAA");
}

#[test]
fn encoder_round_trips_literals_repetition_and_long_distance() {
    let mut source: Vec<u8> = (0..=255).cycle().take(70_000).collect();
    source.extend_from_slice(&vec![0; 12_000]);
    source.extend_from_slice(b"KOREAN-POC-KOREAN-POC-KOREAN-POC");
    let encoded = encode_cpx_block(&source).unwrap();
    let decoded = decode_cpx_block(&encoded).unwrap();
    assert_eq!(decoded.header_value as usize, source.len());
    assert_eq!(decoded.bytes, source);
    assert!(encoded.len() < source.len());
}

#[test]
fn encoder_is_deterministic_for_incompressible_input() {
    let source: Vec<u8> = (0..10_000)
        .map(|index| ((index * 73 + index / 31) & 0xff) as u8)
        .collect();
    let first = encode_cpx_block(&source).unwrap();
    let second = encode_cpx_block(&source).unwrap();
    assert_eq!(first, second);
    assert_eq!(decode_cpx_block(&first).unwrap().bytes, source);
}

#[test]
fn encoder_combines_a_short_literal_run_with_a_near_match() {
    let source = b"abcXabc";
    let encoded = encode_cpx_block(source).unwrap();
    let decoded = decode_cpx_block(&encoded).unwrap();

    assert_eq!(decoded.bytes, source);
    assert_eq!(decoded.commands.literal_then_match, 1);
    assert_eq!(decoded.commands.short_literal, 0);
    assert_eq!(decoded.commands.short_match_8, 0);
}

#[test]
fn chunked_match_length_equals_the_bytewise_cyclic_matcher() {
    let sources = [
        vec![0_u8; 512],
        (0..512).map(|index| (index % 7) as u8).collect(),
        (0..512)
            .map(|index| ((index * 73 + index / 11) & 0xff) as u8)
            .collect(),
    ];
    for source in &sources {
        for cursor in 3..source.len() {
            let maximum_length = (source.len() - cursor).min(MAX_MATCH_LENGTH);
            for candidate in 0..cursor {
                if prefix_key(source, cursor) != prefix_key(source, candidate) {
                    continue;
                }
                let distance = cursor - candidate;
                let mut expected = 3;
                while expected < maximum_length
                    && source[candidate + (expected % distance)] == source[cursor + expected]
                {
                    expected += 1;
                }
                assert_eq!(
                    cyclic_match_length(source, cursor, candidate, distance, maximum_length),
                    expected,
                    "cursor {cursor}, candidate {candidate}"
                );
            }
        }
    }
}

#[test]
fn encoder_preserves_an_image_header_smaller_than_decoded_storage() {
    let mut source = vec![0x31; 32];
    source.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);

    let encoded = encode_cpx_block_with_header_value(&source, 32).unwrap();
    let decoded = decode_cpx_block(&encoded).unwrap();

    assert_eq!(decoded.header_value, 32);
    assert_eq!(decoded.bytes, source);
}

#[test]
fn rejects_truncated_literal() {
    let error = decode_cpx_block(&block(5, &[0x45, b'a']))
        .unwrap_err()
        .to_string();
    assert!(error.contains("literal exceeds input"));
}

#[test]
fn rejects_match_without_decoded_history() {
    let error = decode_cpx_block(&block(3, &[0x01, 0x01, 0x00]))
        .unwrap_err()
        .to_string();
    assert!(error.contains("has no decoded history"));
}

#[test]
fn preserves_header_value_without_treating_it_as_output_size() {
    let decoded = decode_cpx_block(&block(2, &[0x41, b'A', 0x00])).unwrap();
    assert_eq!(decoded.header_value, 2);
    assert_eq!(decoded.bytes, b"A");
}

#[test]
fn container_requires_zero_padding_between_blocks() {
    let mut container = padded_block(b"A", &[0x41, b'A', 0x00]);
    container[32] = 1;
    let error = survey_cpx_container("/IMAGE.BIN;1", &container)
        .unwrap_err()
        .to_string();
    assert!(error.contains("nonzero alignment padding"));
}

#[test]
fn container_walks_consecutive_aligned_blocks() {
    let mut container = padded_block(b"A", &[0x41, b'A', 0x00]);
    container.extend_from_slice(&padded_block(b"BBBB", &[0x41, b'B', 0x01, 0x01, 0x00]));
    let survey = survey_cpx_container("/IMAGE.BIN;1", &container).unwrap();
    assert_eq!(survey.summary.blocks, 2);
    assert_eq!(survey.summary.decoded_bytes, 5);
    assert_eq!(survey.blocks[1].offset, CPX_BLOCK_ALIGNMENT as u64);
    assert!(survey.trailing_region.is_none());
}

#[test]
fn container_records_non_cpx_trailing_region() {
    let mut container = padded_block(b"A", &[0x41, b'A', 0x00]);
    container.extend_from_slice(&[0x5a; CPX_BLOCK_ALIGNMENT]);
    let survey = survey_cpx_container("/IMAGE.BIN;1", &container).unwrap();
    let trailing = survey.trailing_region.unwrap();
    assert_eq!(trailing.offset, CPX_BLOCK_ALIGNMENT as u64);
    assert_eq!(trailing.byte_length, CPX_BLOCK_ALIGNMENT as u64);
}

#[test]
fn container_rejects_cpx_after_non_cpx_region() {
    let mut container = padded_block(b"A", &[0x41, b'A', 0x00]);
    container.extend_from_slice(&[0x5a; CPX_BLOCK_ALIGNMENT]);
    container.extend_from_slice(&padded_block(b"B", &[0x41, b'B', 0x00]));
    let error = survey_cpx_container("/IMAGE.BIN;1", &container)
        .unwrap_err()
        .to_string();
    assert!(error.contains("resume after a non-CPX region"));
}

#[test]
fn file_list_links_cpx_dimensions_and_trailing_records() {
    let mut source = padded_block(b"A", &[0x41, b'A', 0x00]);
    source.extend_from_slice(&[0x5a; CPX_BLOCK_ALIGNMENT]);
    let mut survey = survey_cpx_container("/IMAGE.BIN;1", &source).unwrap();
    let mut records = file_list_record([0, 1, 18, 1, 1]);
    records.extend_from_slice(&file_list_record([1, 1, 4, 0, 0]));

    validate_file_list(&mut survey, "FileList_Image", &records).unwrap();

    let file_list = survey.file_list.unwrap();
    assert_eq!(file_list.records, 2);
    assert_eq!(file_list.cpx_records, 1);
    assert_eq!(file_list.non_cpx_records, 1);
    assert_eq!(file_list.non_cpx_payload_bytes, 4);
    let image_record = survey.blocks[0].image_record.as_ref().unwrap();
    assert_eq!(image_record.image_byte_length, 1);
    assert_eq!(image_record.decoded_trailing_byte_length, 0);
}

#[test]
fn file_list_rejects_a_changed_cpx_stored_length() {
    let source = padded_block(b"A", &[0x41, b'A', 0x00]);
    let mut survey = survey_cpx_container("/IMAGE.BIN;1", &source).unwrap();
    let records = file_list_record([0, 1, 17, 1, 1]);

    let error = validate_file_list(&mut survey, "FileList_Image", &records)
        .unwrap_err()
        .to_string();

    assert!(error.contains("stored length differs"));
}

#[test]
fn tight_slot_search_recovers_a_match_beyond_the_fast_budget() {
    let prefix = [0x12, 0x34, 0x56];
    let mut source = Vec::new();
    let mut seed = 0x81324567_u32;
    for _ in 0..650 {
        source.extend_from_slice(&prefix);
        for _ in 0..48 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            source.push(seed as u8);
        }
    }
    source.extend_from_within(..51);
    let standard = encode_cpx_block(&source).unwrap();
    let fitted = encode_cpx_block_to_fit(&source, source.len() as u32, standard.len() - 1).unwrap();
    assert!(fitted.len() <= standard.len());
    let cursor = source.len() - 51;
    let mut history = HashMap::new();
    for position in 0..cursor {
        remember_position(&source, position, &mut history);
    }
    assert!(
        find_match(&source, cursor, &history, MAX_MATCH_DISTANCE)
            .unwrap()
            .1
            > find_match(&source, cursor, &history, MATCH_SEARCH_LIMIT)
                .unwrap()
                .1
    );
    assert_eq!(decode_cpx_block(&fitted).unwrap().bytes, source);
    assert_eq!(
        encode_cpx_block_to_fit(&source, source.len() as u32, standard.len()).unwrap(),
        standard
    );
}

#[test]
fn tight_slot_can_defer_a_short_match_for_a_longer_following_match() {
    let source = b"xbcdefgh|axbZ!axbcdefgh";
    let header_value = 8;
    let greedy = encode_with_search_limit(source, header_value, MAX_MATCH_DISTANCE, None).unwrap();
    let lazy = encode_with_search_limit(source, header_value, MAX_MATCH_DISTANCE, Some(2)).unwrap();
    assert!(lazy.len() < greedy.len());
    let fitted = encode_cpx_block_to_fit(source, header_value, lazy.len()).unwrap();
    assert!(fitted.len() <= lazy.len());
    let decoded = decode_cpx_block(&fitted).unwrap();
    assert_eq!(decoded.bytes, source);
    assert_eq!(decoded.header_value, header_value);
}
