use super::*;

fn target(
    role: &'static str,
    file_list_symbol: &'static str,
    image_file_record: usize,
) -> FontFileRecordTarget {
    FontFileRecordTarget {
        role,
        file_list_symbol,
        image_file_record,
        expected_stored_length: 0,
    }
}

#[test]
fn both_main_font_clone_stored_lengths_are_required() {
    validate_font_file_record_targets(&[
        target("primary", "FileList_Image", 21),
        target("Nazo clone", "FileList_Image2", 122),
    ])
    .unwrap();
}

#[test]
fn omitting_the_nazo_clone_stored_length_is_rejected() {
    let error = validate_font_file_record_targets(&[target("primary", "FileList_Image", 21)])
        .unwrap_err()
        .to_string();

    assert!(error.contains("record 122"));
}

#[test]
fn substituting_a_different_record_is_rejected() {
    let error = validate_font_file_record_targets(&[
        target("primary", "FileList_Image", 21),
        target("wrong clone", "FileList_Image2", 121),
    ])
    .unwrap_err()
    .to_string();

    assert!(error.contains("record 122"));
}
