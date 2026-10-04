use super::*;

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_nomi_search_demo_06_uses_two_created_pattern_actors() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/nomi-search-callouts-06.json",
    ))
    .unwrap();
    validate_set_asset_header(&asset).unwrap();

    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("Nomi search demo 06 must use the legacy demo route");
    assert_eq!(route.demo_index, 6);
    assert_eq!(route.selected_load_table_symbol, "DEMOTexTbl06");
    assert_eq!(route.selected_pattern_table_symbol, "DEMOPtn06");
    assert_eq!(asset.members.len(), 2);

    for (member, pattern, actor, command_offset, origin, depth) in [
        (&asset.members[0], 7, 10, 0x54, (358, 212), 19),
        (&asset.members[1], 8, 8, 0x6c2, (192, 192), 24),
    ] {
        let StylizedArtworkConsumer::SequencePattern(consumer) = &member.consumer else {
            panic!("Nomi search demo 06 callout must use a sequence-pattern consumer")
        };
        assert_eq!(consumer.pattern_record_index, pattern);
        assert_eq!(consumer.sequence_actor_index, actor);
        assert_eq!(consumer.sequence_command_offset, command_offset);
        assert_eq!(
            (
                consumer.pattern_screen_origin.x,
                consumer.pattern_screen_origin.y
            ),
            origin
        );
        assert_eq!(consumer.pattern_depth, depth);
        assert_eq!(
            consumer.sequence_operation,
            StylizedArtworkSequenceOperation::CreateActor { flags: 0 }
        );
    }
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_nomi_search_demo_07_selects_four_scripted_callout_frames() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/nomi-search-callouts-07.json",
    ))
    .unwrap();
    validate_set_asset_header(&asset).unwrap();

    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("Nomi search demo 07 must use the legacy demo route");
    assert_eq!(route.demo_index, 7);
    assert_eq!(route.selected_animation_table_symbol, "DEMOAnime07");
    assert_eq!(asset.members.len(), 4);

    for (member, pattern, create_offset, animation_offset, animation_index, symbol) in [
        (&asset.members[0], 1, 0x11c, 0x13a, 9, "Anime07_KOKO_1"),
        (&asset.members[1], 2, 0x166, 0x184, 11, "Anime07_KOKO_3"),
        (&asset.members[2], 3, 0x166, 0x1a8, 13, "Anime07_KOKO_5"),
        (&asset.members[3], 4, 0x1d2, 0x1f0, 15, "Anime07_KOKO_7"),
    ] {
        let StylizedArtworkConsumer::ScriptedAnimation(consumer) = &member.consumer else {
            panic!("Nomi search demo 07 callout must use a scripted-animation consumer")
        };
        assert_eq!(consumer.pattern_record_index, pattern);
        assert_eq!(consumer.create_command_offset, create_offset);
        assert_eq!(consumer.animation_command_offset, animation_offset);
        assert_eq!(consumer.animation_index, animation_index);
        assert_eq!(consumer.animation_table_symbol, symbol);
        assert_eq!(consumer.animation_script_record_index, 0);
        assert_eq!(consumer.animation_frame_duration, 57);
    }
}
