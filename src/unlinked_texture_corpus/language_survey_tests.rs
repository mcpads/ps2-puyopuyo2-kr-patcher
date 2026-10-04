use super::*;

#[test]
fn japanese_candidates_are_not_admitted_without_a_consumer() {
    let (candidate, preserves_english) =
        language_scope(TextureLanguageClassification::JapaneseOnly);
    assert!(candidate);
    assert!(!preserves_english);
}

#[test]
fn english_only_pixels_are_protected_but_not_translation_candidates() {
    let (candidate, preserves_english) = language_scope(TextureLanguageClassification::EnglishOnly);
    assert!(!candidate);
    assert!(preserves_english);
}

#[test]
fn selected_dimensions_must_name_one_rendered_candidate() {
    let previews = vec![
        UnlinkedTexturePreview {
            width: 768,
            height: 512,
            decoded_trailing_byte_length: 0,
            swizzled_indices_sha256: String::new(),
            linear_indices_sha256: String::new(),
            rgba_sha256: String::new(),
            png_path: String::new(),
            png_sha256: String::new(),
        },
        UnlinkedTexturePreview {
            width: 1_024,
            height: 384,
            decoded_trailing_byte_length: 0,
            swizzled_indices_sha256: String::new(),
            linear_indices_sha256: String::new(),
            rgba_sha256: String::new(),
            png_path: String::new(),
            png_sha256: String::new(),
        },
    ];
    let entry = UnlinkedTextureLanguageEntry {
        image_index: 0,
        width: 640,
        height: 480,
        dimension_resolved: true,
        classification: TextureLanguageClassification::NonText,
        note: "uniform canvas".to_string(),
    };
    assert!(selected_preview(&previews, &entry).is_err());
}
