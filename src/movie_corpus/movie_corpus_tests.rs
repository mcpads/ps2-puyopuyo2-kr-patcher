use super::*;

#[test]
fn mixed_movie_scope_targets_japanese_and_protects_english() {
    assert_eq!(
        language_scope(
            MovieLanguageClassification::JapaneseEnglishMixed,
            ProductScope::Included,
        ),
        (true, true)
    );
}

#[test]
fn other_title_promotion_is_not_a_translation_target() {
    assert_eq!(
        language_scope(
            MovieLanguageClassification::JapaneseEnglishMixed,
            ProductScope::ExcludedOtherTitlePromotion,
        ),
        (false, true)
    );
}

#[test]
fn english_only_movie_is_not_a_translation_target() {
    assert_eq!(
        language_scope(
            MovieLanguageClassification::EnglishOnly,
            ProductScope::Included,
        ),
        (false, true)
    );
}
