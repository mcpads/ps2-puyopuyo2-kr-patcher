use encoding_rs::SHIFT_JIS;

use super::*;

fn fixture(text: &str) -> Vec<u8> {
    let (encoded, encoding, had_errors) = SHIFT_JIS.encode(text);
    assert_eq!(encoding, SHIFT_JIS);
    assert!(!had_errors);
    let mut bytes = encoded.into_owned();
    bytes.push(0);
    bytes
}

#[test]
fn scan_separates_text_data_and_unowned_japanese_runs() {
    let mut source = fixture("関数");
    let data_start = source.len();
    source.extend(fixture("資料"));
    let gap_start = source.len();
    source.extend(fixture("候補"));
    let section_address = 0x0040_0000;
    let scan = scan_section_runs(
        section_address,
        0x80,
        &source,
        &[NamedRange {
            start: section_address,
            end: section_address + data_start as u64,
        }],
        &[NamedRange {
            start: section_address + data_start as u64,
            end: section_address + gap_start as u64,
        }],
    );

    assert_eq!(scan.japanese_script_runs, 3);
    assert_eq!(scan.text_symbol_owned_japanese_runs, 1);
    assert_eq!(scan.data_symbol_owned_japanese_runs, 1);
    assert_eq!(scan.candidates.len(), 1);
    assert_eq!(scan.candidates[0].display_text, "候補");
    assert_eq!(scan.candidates[0].file_offset, 0x80 + gap_start as u64);
}

#[test]
fn scan_requires_the_terminator_to_fit_inside_the_data_owner() {
    let source = fixture("境界");
    let section_address = 0x0040_0000;
    let scan = scan_section_runs(
        section_address,
        0x80,
        &source,
        &[],
        &[NamedRange {
            start: section_address,
            end: section_address + source.len() as u64 - 1,
        }],
    );

    assert_eq!(scan.data_symbol_owned_japanese_runs, 0);
    assert_eq!(scan.candidates.len(), 1);
}
