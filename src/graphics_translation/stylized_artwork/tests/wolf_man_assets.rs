use super::*;

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_wolf_man_callouts_use_two_parent_relative_patterns() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/wolf-man-callouts.json",
    ))
    .unwrap();
    validate_set_asset_header(&asset).unwrap();

    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("Wolf Man callouts must use the legacy demo route");
    assert_eq!(route.demo_index, 67);
    assert_eq!(route.selected_pattern_table_symbol, "DEMOPtn43");
    assert_eq!(asset.members.len(), 2);

    for (member, pattern, actor, origin_y, depth) in [
        (&asset.members[0], 24, 6, 360, 21),
        (&asset.members[1], 25, 7, 250, 22),
    ] {
        let StylizedArtworkConsumer::SequencePattern(consumer) = &member.consumer else {
            panic!("Wolf Man callout must use a sequence-pattern consumer")
        };
        assert_eq!(consumer.pattern_record_index, pattern);
        assert_eq!(consumer.sequence_actor_index, actor);
        assert_eq!(consumer.pattern_screen_origin.x, 380);
        assert_eq!(consumer.pattern_screen_origin.y, origin_y);
        assert_eq!(consumer.pattern_depth, depth);
        let StylizedArtworkSequenceOperation::CreateChildActor {
            flags,
            parent_actor_index,
            parent_create_command_offset,
            ..
        } = consumer.sequence_operation
        else {
            panic!("Wolf Man callout must be parent-relative")
        };
        assert_eq!(flags, 256);
        assert_eq!(parent_actor_index, 2);
        assert_eq!(parent_create_command_offset, 0x1c);
    }
}
