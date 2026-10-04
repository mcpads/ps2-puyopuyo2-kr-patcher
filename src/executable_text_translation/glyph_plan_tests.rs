use crate::executable_text_translation::{
    ExecutableTextTranslationEntry, ExecutableTextTranslationPopulation,
    ExecutableTextTranslationScope, ExecutableTextTranslationSource,
};
use crate::font_atlas::{FontGlyph, FontGlyphSet, GlyphRectangle};
use crate::memory_card_translation::{
    KoreanTranslation, TranslationBundleSource, TranslationStatus,
};

use super::*;

fn glyph_set(slot_count: usize) -> FontGlyphSet {
    FontGlyphSet {
        symbol: "FontList_Test".to_string(),
        source_encoding: "Shift-JIS",
        role: "test fullwidth lookup",
        glyphs: (0..slot_count)
            .map(|index| FontGlyph {
                index,
                source_hex: format!("81{:02x}", 0x40 + index),
                character: "가".to_string(),
                rectangle: GlyphRectangle {
                    x: 0,
                    y: 0,
                    width: 32,
                    height: 32,
                },
            })
            .collect(),
    }
}

fn translation_bundle(entries: &[(&str, &str)]) -> ExecutableTextTranslationBundle {
    ExecutableTextTranslationBundle {
        schema: 1,
        source: TranslationBundleSource {
            source_bin_sha256: "source".to_string(),
            executable_iso_path: "/EXEC;1".to_string(),
            executable_sha256: "executable".to_string(),
        },
        scope: ExecutableTextTranslationScope {
            consumer_families: Vec::new(),
            target: String::new(),
            preserve: String::new(),
            build_policy: String::new(),
            release_candidate: false,
        },
        population: ExecutableTextTranslationPopulation {
            tutorial_messages: 0,
            puzzle_puyo_messages: 0,
            memory_card_metadata_titles: 0,
            total_entries: entries.len(),
        },
        entries: entries
            .iter()
            .enumerate()
            .map(
                |(index, (consumer_family, text))| ExecutableTextTranslationEntry {
                    source: ExecutableTextTranslationSource {
                        asset_id: format!("entry-{index}"),
                        consumer_family: (*consumer_family).to_string(),
                        symbol: "Test".to_string(),
                        source_address: "00000000".to_string(),
                        source_file_offset: "00000000".to_string(),
                        storage_byte_length: 64,
                        payload_byte_length: 1,
                        source_sha256: "source-text".to_string(),
                        source_hex: "00".to_string(),
                        terminator: "nul".to_string(),
                        source_text: "日".to_string(),
                        protected_ascii: Vec::new(),
                        reference_ids: Vec::new(),
                    },
                    translation: KoreanTranslation {
                        status: TranslationStatus::NeedsHumanReview,
                        text: (*text).to_string(),
                        translator_notes: Vec::new(),
                        human_approval: None,
                        independent_review: None,
                    },
                },
            )
            .collect(),
    }
}

#[test]
fn allocation_reserves_the_runtime_fallback_slot() {
    let plan = plan_family_glyphs("tutorial", &glyph_set(4), 1, &['한', '글', '화']).unwrap();

    assert_eq!(plan.assignable_slots, 3);
    assert_eq!(plan.remaining_slots, 0);
    assert_eq!(
        plan.assignments
            .iter()
            .map(|assignment| assignment.slot_index)
            .collect::<Vec<_>>(),
        [0, 2, 3]
    );
    assert_eq!(plan.assignments[0].code_hex, "f040");
}

#[test]
fn glyph_demand_uses_first_appearance_order_within_one_consumer() {
    let bundle = translation_bundle(&[
        ("tutorial", "가나다가"),
        ("puzzle_puyo", "섞지마"),
        ("tutorial", "라마나"),
    ]);

    assert_eq!(
        unique_family_hangul(&bundle, "tutorial"),
        ['가', '나', '다', '라', '마']
    );
    assert_eq!(
        unique_family_hangul(&bundle, "puzzle_puyo"),
        ['섞', '지', '마']
    );
}

#[test]
fn allocation_rejects_demand_that_would_overwrite_the_fallback() {
    let error = plan_family_glyphs("tutorial", &glyph_set(3), 1, &['한', '글', '화']).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("after reserving fallback slot 1")
    );
}

#[test]
fn halfwidth_allocation_uses_only_shadowed_space_slots_for_missing_ascii() {
    let halfwidth = FontGlyphSet {
        symbol: "FontList_Nazo_Han".to_string(),
        source_encoding: "ASCII subset",
        role: "test halfwidth lookup",
        glyphs: [b'A', b' ', b' ', b' ']
            .into_iter()
            .enumerate()
            .map(|(index, character)| FontGlyph {
                index,
                source_hex: format!("{character:02x}"),
                character: char::from(character).to_string(),
                rectangle: GlyphRectangle {
                    x: u32::try_from(index).unwrap() * 16,
                    y: 224,
                    width: 16,
                    height: 32,
                },
            })
            .collect(),
    };

    let plan = plan_missing_halfwidth_glyphs("puzzle_puyo", &halfwidth, &[2, 3], &['A', ' ', '?'])
        .unwrap();

    assert_eq!(plan.required_added_glyphs, 1);
    assert_eq!(plan.assignments[0].character, "?");
    assert_eq!(plan.assignments[0].code_hex, "3f");
    assert_eq!(plan.assignments[0].slot_index, 2);
    assert_eq!(plan.remaining_shadowed_space_slots, 1);
}

#[test]
fn halfwidth_allocation_rejects_missing_ascii_beyond_shadowed_space_capacity() {
    let halfwidth = FontGlyphSet {
        symbol: "FontList_Nazo_Han".to_string(),
        source_encoding: "ASCII subset",
        role: "test halfwidth lookup",
        glyphs: glyph_set(2).glyphs,
    };

    let error =
        plan_missing_halfwidth_glyphs("puzzle_puyo", &halfwidth, &[1], &['?', '!']).unwrap_err();

    assert!(error.to_string().contains("only 1 shadowed space slots"));
}
