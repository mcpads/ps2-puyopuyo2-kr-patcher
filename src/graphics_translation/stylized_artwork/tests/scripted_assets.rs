use super::*;

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_zombie_reaction_selects_the_blinking_script_frame() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/zombie-why.json",
    ))
    .unwrap();
    validate_set_asset_header(&asset).unwrap();

    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("zombie reaction must use the legacy demo route");
    assert_eq!(route.demo_index, 31);
    assert_eq!(route.selected_load_table_symbol, "DEMOTexTbl1F");
    assert_eq!(route.selected_pattern_table_symbol, "DEMOPtn1F");
    assert_eq!(route.selected_animation_table_symbol, "DEMOAnime1F");

    let member = asset.members.first().unwrap();
    assert_eq!(member.expected_source_text, "ここにも");
    assert_eq!(member.expected_korean_text, "여기에도");
    let StylizedArtworkConsumer::ScriptedAnimation(consumer) = &member.consumer else {
        panic!("zombie reaction must use the scripted-animation consumer")
    };
    assert_eq!(consumer.pattern_record_index, 17);
    assert_eq!(consumer.create_command_offset, 0x1c8);
    assert_eq!(consumer.animation_command_offset, 0x1d6);
    assert_eq!(consumer.animation_index, 7);
    assert_eq!(consumer.animation_table_symbol, "Anime1F_KOKO_1");
    assert_eq!(consumer.animation_script_record_index, 1);
    assert_eq!(consumer.animation_frame_duration, 30);
    assert_eq!(consumer.derived_logical_screen_bounds.x, 516);
    assert_eq!(consumer.derived_logical_screen_bounds.y, 223);
}
