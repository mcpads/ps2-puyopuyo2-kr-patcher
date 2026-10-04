use super::*;

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_schezo_wordplay_uses_four_animation_frames_and_two_created_labels() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/schezo-wordplay.json",
    ))
    .unwrap();
    validate_set_asset_header(&asset).unwrap();

    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("Schezo wordplay must use the legacy demo route");
    assert_eq!(route.demo_index, 0x3d);
    assert_eq!(route.selected_load_table_symbol, "DEMOTexTbl3D");
    assert_eq!(route.selected_pattern_table_symbol, "DEMOPtn3D");
    assert_eq!(route.selected_animation_table_symbol, "DEMOAnime3D");
    assert_eq!(route.selected_sequence_symbol, "DEMO3D");
    assert_eq!(asset.members.len(), 6);

    for (member, pattern, script_record, duration) in [
        (&asset.members[0], 12, 11, 59),
        (&asset.members[1], 13, 12, 105),
        (&asset.members[2], 14, 13, 128),
        (&asset.members[3], 15, 14, 24),
    ] {
        let StylizedArtworkConsumer::ScriptedAnimation(consumer) = &member.consumer else {
            panic!("Schezo wordplay transition must use a scripted-animation consumer")
        };
        assert_eq!(consumer.pattern_record_index, pattern);
        assert_eq!(consumer.create_command_offset, 0x5e);
        assert_eq!(consumer.animation_command_offset, 0x192);
        assert_eq!(consumer.animation_index, 5);
        assert_eq!(consumer.animation_table_symbol, "Anime3D_SHEZ_1");
        assert_eq!(consumer.animation_script_record_index, script_record);
        assert_eq!(consumer.animation_frame_duration, duration);
    }

    for (member, pattern, command_offset, origin_y) in [
        (&asset.members[4], 41, 0x1a2, 140),
        (&asset.members[5], 42, 0x258, 224),
    ] {
        let StylizedArtworkConsumer::SequencePattern(consumer) = &member.consumer else {
            panic!("Schezo lower wordplay label must use a sequence-pattern consumer")
        };
        assert_eq!(consumer.pattern_record_index, pattern);
        assert_eq!(consumer.sequence_command_offset, command_offset);
        assert_eq!(consumer.sequence_actor_index, 10);
        assert_eq!(consumer.pattern_screen_origin.x, 370);
        assert_eq!(consumer.pattern_screen_origin.y, origin_y);
        assert_eq!(consumer.pattern_depth, 21);
        assert_eq!(
            consumer.sequence_operation,
            StylizedArtworkSequenceOperation::CreateActor { flags: 0 }
        );
    }
}
