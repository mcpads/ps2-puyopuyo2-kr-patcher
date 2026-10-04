use super::sequence::{
    ensure_story_region_uses_story_ledger_for_test, parse_message_window_commands_for_test,
    physical_text_row_widths_for_test, physical_text_rows_for_test, row_window_widths_for_test,
};

#[test]
fn story_graphics_region_rejects_a_duplicate_korean_draft() {
    let error = ensure_story_region_uses_story_ledger_for_test(
        "いつも陽気だなぁ\nキミは",
        "いつも陽気だなぁ\nキミは",
        Some("그래픽 원장에 복사된 문장"),
    )
    .unwrap_err();

    assert!(error.to_string().contains("duplicates Korean wording"));
}

#[test]
fn story_graphics_region_rejects_source_transcription_drift() {
    let error = ensure_story_region_uses_story_ledger_for_test(
        "いつも陽気だなぁ\nキミは",
        "いつも陽気だなぁ",
        None,
    )
    .unwrap_err();

    assert!(error.to_string().contains("source text differs"));
}

#[test]
fn physical_text_rows_ignore_a_glyph_spill_at_the_edge_of_a_blank_cell() {
    let width = 4;
    let height = 72;
    let mut pixels = vec![0; width * height * 4];
    for &(x, y) in &[(0, 12), (3, 13), (1, 24), (2, 60)] {
        let offset = (y * width + x) * 4;
        pixels[offset..offset + 4].copy_from_slice(&[u8::MAX; 4]);
    }

    let rows = physical_text_rows_for_test(width, height, &pixels).unwrap();

    assert_eq!(rows, [0, 2]);
}

#[test]
fn physical_text_row_width_uses_the_rightmost_interior_source_pixel() {
    let width = 4;
    let height = 72;
    let mut pixels = vec![0; width * height * 4];
    for &(x, y) in &[(0, 12), (3, 13), (1, 24), (2, 60)] {
        let offset = (y * width + x) * 4;
        pixels[offset..offset + 4].copy_from_slice(&[u8::MAX; 4]);
    }

    let widths = physical_text_row_widths_for_test(width, height, &pixels).unwrap();

    assert_eq!(widths.into_iter().collect::<Vec<_>>(), [(0, 4), (2, 3)]);
}

#[test]
fn parses_message_window_source_rows_and_cell_rectangles() {
    let command_sizes = fixed_command_sizes();
    let mut sequence = vec![0x10, 0x00];
    sequence.extend_from_slice(&[0; 12]);
    sequence.extend_from_slice(&[
        0x34, 0x00, 0x05, 0x04, 0x64, 0x00, 0x21, 0x03, 0x07, 0x02, 0x06, 0x0f, 0x03, 0x0a, 0x79,
        0x00, 0x3c, 0x00,
    ]);

    let commands = parse_message_window_commands_for_test(&sequence, &command_sizes).unwrap();

    assert_eq!(commands, [(5, 4, 7, 10, 3)]);
}

#[test]
fn repeated_windows_preserve_their_minimum_and_maximum_partial_widths() {
    let (minimum, maximum) = row_window_widths_for_test(&[2, 4, 8]);

    assert_eq!((minimum, maximum), (48, 192));
}

#[test]
fn rejects_a_sequence_command_truncated_before_its_declared_end() {
    let command_sizes = fixed_command_sizes();
    let sequence = [0x34, 0x00, 0x05, 0x04];

    let error = parse_message_window_commands_for_test(&sequence, &command_sizes)
        .unwrap_err()
        .to_string();

    assert!(error.contains("ends inside opcode 0x34"));
}

#[test]
fn rejects_an_opcode_outside_the_exact_command_size_table() {
    let command_sizes = fixed_command_sizes();
    let sequence = [0xff, 0x00];

    let error = parse_message_window_commands_for_test(&sequence, &command_sizes)
        .unwrap_err()
        .to_string();

    assert!(error.contains("outside SeqComSize"));
}

fn fixed_command_sizes() -> Vec<usize> {
    let mut sizes = vec![1; 0x35];
    sizes[0x10] = 7;
    sizes[0x34] = 9;
    sizes
}
