use super::*;

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_mode_logo_manifest_selects_the_scrolling_tile_consumer() {
    let asset: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/mode-selection/puyo-tsu-logo.json",
    ))
    .unwrap();
    assert_eq!(asset.expected_korean_text, "뿌요뿌요 통");
    assert_eq!(
        asset.source_palette_selection,
        StylizedArtworkPaletteSelection::OpaqueSourceMask
    );
    let StylizedArtworkConsumer::ScrollingTile(consumer) = asset.consumer else {
        panic!("mode logo must use the scrolling-tile consumer")
    };
    assert_eq!(consumer.pattern_source_bounds.width, 336);
    assert_eq!(consumer.positions.len(), 12);
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_story_mode_panel_set_covers_each_menu_pattern() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-mode-selection/panel-labels.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();
    assert_eq!(
        asset.master_layout,
        StylizedArtworkSetMasterLayout::SharedArtworkSheet
    );
    assert!(
        asset
            .maximum_source_palette_colors_per_member
            .is_some_and(|limit| (2..=256).contains(&limit))
    );
    assert!(asset.shared_artwork_sheet.is_some());
    assert!(
        asset
            .shared_generation_prompt
            .as_deref()
            .is_some_and(|prompt| {
                ["연습", "노멀", "통", "처음", "익숙함", "졸업"]
                    .iter()
                    .all(|label| prompt.contains(label))
            })
    );
    assert_eq!(
        asset
            .members
            .iter()
            .map(|member| member.expected_korean_text.as_str())
            .collect::<Vec<_>>(),
        ["연습", "노멀", "통", "처음", "익숙함", "졸업"]
    );
    for (member, expected_pattern) in asset.members.iter().zip(2..=7) {
        assert!(member.generation_prompt.is_none());
        assert!(member.transparent_master.is_none());
        assert_eq!(
            member.source_palette_selection,
            StylizedArtworkPaletteSelection::OpaqueSourceMask
        );
        let StylizedArtworkConsumer::MenuPanel(consumer) = &member.consumer else {
            panic!("story-mode panel must use the menu-panel consumer")
        };
        assert_eq!(consumer.pattern_record_index, expected_pattern);
        assert_eq!(consumer.load_route.image_file_record, 29);
        assert_eq!(consumer.load_route.palette_file_record, 89);
        assert_eq!(consumer.registration_route.texture_slot, 1);
        assert_eq!(member.native_placement, consumer.pattern_source_bounds);
        let composition_bounds = catalog
            .surfaces
            .iter()
            .find(|surface| surface.source_surface_id == asset.source_surface_id)
            .and_then(|surface| {
                surface
                    .regions
                    .iter()
                    .find(|region| region.region_id == member.region_id)
            })
            .and_then(|region| region.artwork_composition_bounds)
            .expect("every story-mode card must declare its full composition bounds");
        assert_eq!(composition_bounds, consumer.pattern_source_bounds);
    }
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_rule_selection_heading_uses_the_direct_pattern_display() {
    let asset: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/rule-selection/selection-heading.json",
    ))
    .unwrap();
    assert_eq!(asset.expected_korean_text, "규칙을 선택해 주세요");
    assert_eq!(
        asset.source_palette_selection,
        StylizedArtworkPaletteSelection::OpaqueSourceMask
    );
    let StylizedArtworkConsumer::DirectPatternDisplay(consumer) = asset.consumer else {
        panic!("rule-selection heading must use the direct-pattern display consumer")
    };
    assert_eq!(consumer.pattern_record_index, 1);
    assert_eq!(consumer.pattern_screen_origin.x, 56);
    assert_eq!(consumer.pattern_screen_origin.y, 8);
    assert_eq!(consumer.derived_logical_screen_bounds.x, 56);
    assert_eq!(consumer.derived_logical_screen_bounds.y, 16);
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_scylla_elapsed_card_preserves_the_number_and_uses_its_create_command() {
    let asset: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/scylla-elapsed-time.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    assert_eq!(asset.expected_source_text, "分経過");
    assert_eq!(asset.expected_korean_text, "분 경과");
    assert_eq!(asset.native_placement.x, 122);
    assert_eq!(asset.native_placement.y, 4);
    assert_eq!(asset.native_placement.width, 388);
    assert_eq!(asset.native_placement.height, 120);
    assert_eq!(asset.source_palette_roles.len(), 1);
    assert_eq!(asset.source_palette_roles[0].palette_index, 2);

    let StylizedArtworkConsumer::SequencePattern(consumer) = &asset.consumer else {
        panic!("Scylla elapsed card must use its sequence-pattern consumer")
    };
    assert_eq!(consumer.load_table_symbol, "DEMOTexTbl45");
    assert_eq!(consumer.image_load_index, 6);
    assert_eq!(consumer.image_file_record, 178);
    assert_eq!(consumer.palette_slot, 6);
    assert_eq!(consumer.palette_file_record, 508);
    assert_eq!(consumer.pattern_table_symbol, "DEMOPtn45");
    assert_eq!(consumer.pattern_record_index, 3);
    assert_eq!(consumer.sequence_symbol, "DEMO45");
    assert_eq!(consumer.sequence_command_offset, 0x2de);
    assert_eq!(consumer.sequence_opcode, 0x10);
    assert_eq!(consumer.sequence_command_words, 7);
    assert_eq!(consumer.sequence_actor_index, 5);
    assert_eq!(
        consumer.sequence_operation,
        StylizedArtworkSequenceOperation::CreateActor { flags: 8 }
    );
    assert_eq!(consumer.sequence_case_address, 0x0045_7be8);
    assert_eq!(consumer.pattern_screen_origin.x, 640);
    assert_eq!(consumer.pattern_screen_origin.y, 100);
    assert_eq!(consumer.pattern_depth, 20);

    let declaration = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == asset.source_surface_id)
        .and_then(|surface| {
            surface
                .regions
                .iter()
                .find(|region| region.region_id == asset.region_id)
        })
        .expect("elapsed card must have a static-region declaration");
    assert_eq!(declaration.bounds.x, 122);
    assert_eq!(declaration.source_text, "分経過");
    assert_eq!(declaration.korean_text.as_deref(), Some("분 경과"));

    let overlay = read_rgba_surface(Path::new(&asset.replacement_overlay_path)).unwrap();
    for (pixel, rgba) in overlay.pixels.chunks_exact(4).enumerate() {
        if rgba[3] == 0 {
            continue;
        }
        let x = u32::try_from(pixel % overlay.width).unwrap();
        let y = u32::try_from(pixel / overlay.width).unwrap();
        assert!(point_is_inside(asset.native_placement, x, y));
        assert!(
            x >= 122,
            "generated lettering overlaps the protected number 30"
        );
    }
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_ending_shock_callouts_share_one_design_and_separate_consumer_evidence() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/ending-shock-callouts.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    let route = asset
        .demo_registration_route
        .as_ref()
        .expect("ending shock callouts must declare their demo registration route");
    assert_eq!(route.demo_index, 0x4c);
    assert_eq!(route.registration_unit_index, 2);
    assert_eq!(route.registration_group_index, 2);
    assert_eq!(route.registration_set_symbol, "DEMOTex4C_3");
    assert_eq!(route.image_load_index, 5);
    assert_eq!(route.palette_slot, 5);
    assert_eq!(route.texture_slot, 0);
    assert_eq!(route.transition_command_offsets, [0x2c, 0x72a]);

    assert_eq!(asset.members.len(), 2);
    assert_eq!(
        asset.members[0].transparent_master,
        asset.members[1].transparent_master
    );
    assert_eq!(asset.members[0].expected_korean_text, "쿠궁");
    assert_eq!(asset.members[1].expected_korean_text, "쿠궁");

    let StylizedArtworkConsumer::SequencePattern(upper) = &asset.members[0].consumer else {
        panic!("upper ending shock callout must use its selected sequence pattern")
    };
    assert_eq!(upper.image_load_index, 5);
    assert_eq!(upper.pattern_texture_slot, Some(0));
    assert_eq!(upper.pattern_record_index, 28);
    assert_eq!(upper.sequence_command_offset, 0x786);
    assert_eq!(
        upper.sequence_operation,
        StylizedArtworkSequenceOperation::CreateActor { flags: 0 }
    );

    let StylizedArtworkConsumer::SourcePattern(lower) = &asset.members[1].consumer else {
        panic!("lower ending shock callout must remain a source-only pattern")
    };
    assert_eq!(lower.image_load_index, 5);
    assert_eq!(lower.pattern_texture_slot, 0);
    assert_eq!(lower.pattern_record_index, 30);
    assert_eq!(lower.animation_pointer_table_symbol, "DEMOAnime4C");
    assert_eq!(lower.animation_table_symbols.len(), 14);

    let declaration = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == asset.source_surface_id)
        .expect("ending shock callouts must have a static-region declaration");
    assert_eq!(
        declaration.regions[0]
            .artwork_plan
            .expect("upper callout must have an artwork plan")
            .placement_status,
        GraphicsArtworkPlacementStatus::ConsumerAligned
    );
    assert_eq!(
        declaration.regions[1]
            .artwork_plan
            .expect("lower callout must have an artwork plan")
            .placement_status,
        GraphicsArtworkPlacementStatus::SourceAligned
    );

    let overlay = read_rgba_surface(Path::new(&asset.replacement_overlay_path)).unwrap();
    let mut upper_pixels = 0;
    let mut lower_pixels = 0;
    for (pixel, rgba) in overlay.pixels.chunks_exact(4).enumerate() {
        if rgba[3] == 0 {
            continue;
        }
        let x = u32::try_from(pixel % overlay.width).unwrap();
        let y = u32::try_from(pixel / overlay.width).unwrap();
        if point_is_inside(asset.members[0].native_placement, x, y) {
            upper_pixels += 1;
        } else if point_is_inside(asset.members[1].native_placement, x, y) {
            lower_pixels += 1;
        } else {
            panic!("ending shock overlay pixel leaves both exact source footprints")
        }
    }
    assert!(upper_pixels > 0);
    assert!(lower_pixels > 0);
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_animated_ending_shock_callout_reuses_the_design_and_follows_gaan() {
    let asset: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/ending-animated-shock-callout.json",
    ))
    .unwrap();
    let shared: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/ending-shock-callouts.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    assert_eq!(
        Some(&asset.transparent_master),
        shared.members[0].transparent_master.as_ref()
    );
    assert_eq!(asset.expected_korean_text, "쿠궁");
    assert_eq!(asset.native_placement.x, 8);
    assert_eq!(asset.native_placement.y, 96);
    assert_eq!(asset.native_placement.width, 328);
    assert_eq!(asset.native_placement.height, 402);
    assert_eq!(asset.render_palette_indices, [2, 19, 30]);

    let route = asset
        .demo_registration_route
        .as_ref()
        .expect("animated ending callout must declare its demo registration route");
    assert_eq!(route.demo_index, 0x4c);
    assert_eq!(route.registration_group_index, 1);
    assert_eq!(route.registration_set_symbol, "DEMOTex4C_2");
    assert_eq!(route.registration_pair_index, 3);
    assert_eq!(route.image_load_index, 4);
    assert_eq!(route.palette_slot, 4);
    assert_eq!(route.texture_slot, 3);
    assert_eq!(route.transition_command_offsets, [0x2c]);

    let StylizedArtworkConsumer::SequencePattern(consumer) = &asset.consumer else {
        panic!("animated ending callout must use its selected sequence pattern")
    };
    assert_eq!(consumer.pattern_record_index, 23);
    assert_eq!(consumer.pattern_texture_slot, Some(3));
    assert_eq!(consumer.sequence_command_offset, 0xfa);
    assert_eq!(consumer.sequence_actor_index, 7);
    assert_eq!(
        consumer.sequence_operation,
        StylizedArtworkSequenceOperation::CreateActor { flags: 7 }
    );
    let movesys = consumer
        .movesys_route
        .as_ref()
        .expect("animated ending callout must declare its GAAN route");
    assert_eq!(movesys.demo_pointer_table_symbol, "DEMOMovesys");
    assert_eq!(movesys.selected_pointer_table_symbol, "MovesysDEMO4C");
    assert_eq!(movesys.script_index, 1);
    assert_eq!(movesys.script_symbol, "MovesysDEMO4C_GAAN");
    assert_eq!(movesys.transform_command_offset, 0x108);
    assert_eq!(movesys.transform_anchor.x, 136);
    assert_eq!(movesys.transform_anchor.y, 224);
    assert_eq!(movesys.initialize_command_offset, 0x110);
    assert_eq!(movesys.commands.len(), 9);
    assert_eq!(movesys.commands.last().unwrap().opcode, 0);

    let declaration = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == asset.source_surface_id)
        .and_then(|surface| {
            surface
                .regions
                .iter()
                .find(|region| region.region_id == asset.region_id)
        })
        .expect("animated ending callout must have a static-region declaration");
    assert_eq!(
        declaration
            .artwork_plan
            .expect("animated ending callout must have an artwork plan")
            .placement_status,
        GraphicsArtworkPlacementStatus::ConsumerAligned
    );

    let overlay = read_rgba_surface(Path::new(&asset.replacement_overlay_path)).unwrap();
    let mut opaque_pixels = 0;
    for (pixel, rgba) in overlay.pixels.chunks_exact(4).enumerate() {
        if rgba[3] == 0 {
            continue;
        }
        opaque_pixels += 1;
        let x = u32::try_from(pixel % overlay.width).unwrap();
        let y = u32::try_from(pixel / overlay.width).unwrap();
        assert!(point_is_inside(asset.native_placement, x, y));
    }
    assert!(opaque_pixels > 0);
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_summit_goal_title_uses_the_first_demo_4e_texture_state() {
    let asset: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/summit-goal-title.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    assert_eq!(asset.expected_korean_text, "정상을 노려라, 사탄 님");
    assert_eq!(
        asset.source_background_restoration,
        GraphicsSourceBackgroundRestoration::DominantSourceIndexPerRow
    );
    assert_eq!(asset.render_palette_indices, [1, 80, 185]);
    assert_eq!(asset.native_placement.x, 0);
    assert_eq!(asset.native_placement.y, 10);
    assert_eq!(asset.native_placement.width, 640);
    assert_eq!(asset.native_placement.height, 236);

    let route = asset
        .demo_registration_route
        .as_ref()
        .expect("summit goal title must declare its demo registration route");
    assert_eq!(route.demo_index, 0x4e);
    assert_eq!(route.registration_unit_index, 4);
    assert_eq!(route.registration_group_index, 0);
    assert_eq!(route.registration_set_symbol, "DEMOTex4E_1");
    assert_eq!(route.registration_pair_index, 1);
    assert_eq!(route.image_load_index, 1);
    assert_eq!(route.palette_slot, 1);
    assert_eq!(route.texture_slot, 1);
    assert!(route.transition_command_offsets.is_empty());

    let StylizedArtworkConsumer::SequencePattern(consumer) = &asset.consumer else {
        panic!("summit goal title must use its selected sequence pattern")
    };
    assert_eq!(consumer.pattern_table_symbol, "DEMOPtn4E");
    assert_eq!(consumer.pattern_record_index, 1);
    assert_eq!(consumer.pattern_texture_slot, Some(1));
    assert_eq!(consumer.sequence_symbol, "DEMO4E");
    assert_eq!(consumer.sequence_command_offset, 0);
    assert_eq!(consumer.sequence_actor_index, 0);
    assert_eq!(
        consumer.sequence_operation,
        StylizedArtworkSequenceOperation::CreateActor { flags: 0 }
    );
    assert_eq!(consumer.pattern_screen_origin.x, 0);
    assert_eq!(consumer.pattern_screen_origin.y, 0);
    assert_eq!(consumer.pattern_depth, 8);

    let declaration = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == asset.source_surface_id)
        .and_then(|surface| {
            surface
                .regions
                .iter()
                .find(|region| region.region_id == asset.region_id)
        })
        .expect("summit goal title must have a static-region declaration");
    assert_eq!(declaration.source_mask_dilation_radius, 2);
    assert_eq!(
        declaration
            .artwork_plan
            .expect("summit goal title must have an artwork plan")
            .placement_status,
        GraphicsArtworkPlacementStatus::ConsumerAligned
    );

    let overlay = read_rgba_surface(Path::new(&asset.replacement_overlay_path)).unwrap();
    let mut opaque_pixels = 0;
    for (pixel, rgba) in overlay.pixels.chunks_exact(4).enumerate() {
        if rgba[3] == 0 {
            continue;
        }
        opaque_pixels += 1;
        let x = u32::try_from(pixel % overlay.width).unwrap();
        let y = u32::try_from(pixel / overlay.width).unwrap();
        assert!(point_is_inside(asset.native_placement, x, y));
    }
    assert!(opaque_pixels > 0);
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_summit_finish_labels_use_the_second_demo_4e_actor() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/summit-finish-labels.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    let route = asset
        .demo_registration_route
        .as_ref()
        .expect("summit finish labels must declare their demo registration route");
    assert_eq!(route.demo_index, 0x4e);
    assert_eq!(route.registration_unit_index, 4);
    assert_eq!(route.registration_group_index, 0);
    assert_eq!(route.registration_set_symbol, "DEMOTex4E_1");
    assert_eq!(route.registration_pair_index, 0);
    assert_eq!(route.image_load_index, 0);
    assert_eq!(route.palette_slot, 0);
    assert_eq!(route.texture_slot, 0);
    assert!(route.transition_command_offsets.is_empty());

    assert_eq!(asset.members.len(), 3);
    assert_eq!(asset.members[0].region_id, "tower-top-label");
    assert_eq!(asset.members[0].expected_korean_text, "꼭대기");
    assert_eq!(asset.members[2].region_id, "spit-callout");
    assert_eq!(asset.members[2].expected_korean_text, "퉤!");
    assert_eq!(asset.members[1].region_id, "ending-label");
    assert_eq!(asset.members[1].expected_korean_text, "끝");
    assert_eq!(asset.members[1].native_placement.x, 295);
    assert_eq!(asset.members[1].native_placement.y, 377);
    assert_eq!(asset.members[1].native_placement.width, 48);
    assert_eq!(asset.members[1].native_placement.height, 54);

    for member in &asset.members {
        let StylizedArtworkConsumer::SequencePattern(consumer) = &member.consumer else {
            panic!("summit finish labels must use their selected sequence pattern")
        };
        assert_eq!(consumer.image_load_index, 0);
        assert_eq!(consumer.palette_slot, 0);
        assert_eq!(consumer.pattern_table_symbol, "DEMOPtn4E");
        assert_eq!(consumer.pattern_record_index, 0);
        assert_eq!(consumer.pattern_texture_slot, Some(0));
        assert_eq!(consumer.sequence_symbol, "DEMO4E");
        assert_eq!(consumer.sequence_command_offset, 14);
        assert_eq!(consumer.sequence_actor_index, 1);
        assert_eq!(
            consumer.sequence_operation,
            StylizedArtworkSequenceOperation::CreateActor { flags: 0 }
        );
        assert_eq!(consumer.pattern_screen_origin.x, 0);
        assert_eq!(consumer.pattern_screen_origin.y, 0);
        assert_eq!(consumer.pattern_depth, 10);

        let declaration = catalog
            .surfaces
            .iter()
            .find(|surface| surface.source_surface_id == asset.source_surface_id)
            .and_then(|surface| {
                surface
                    .regions
                    .iter()
                    .find(|region| region.region_id == member.region_id)
            })
            .expect("summit finish label must have a static-region declaration");
        assert_eq!(
            declaration
                .artwork_plan
                .expect("summit finish label must have an artwork plan")
                .placement_status,
            GraphicsArtworkPlacementStatus::ConsumerAligned
        );
    }

    let overlay = read_rgba_surface(Path::new(&asset.replacement_overlay_path)).unwrap();
    let mut top_pixels = 0;
    let mut ending_pixels = 0;
    let mut spit_pixels = 0;
    for (pixel, rgba) in overlay.pixels.chunks_exact(4).enumerate() {
        if rgba[3] == 0 {
            continue;
        }
        let x = u32::try_from(pixel % overlay.width).unwrap();
        let y = u32::try_from(pixel / overlay.width).unwrap();
        if point_is_inside(asset.members[0].native_placement, x, y) {
            top_pixels += 1;
        } else if point_is_inside(asset.members[1].native_placement, x, y) {
            ending_pixels += 1;
        } else if point_is_inside(asset.members[2].native_placement, x, y) {
            spit_pixels += 1;
        } else {
            panic!("summit finish overlay pixel leaves its declared label placements")
        }
    }
    assert!(top_pixels > 0);
    assert!(ending_pixels > 0);
    assert!(spit_pixels > 0);
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_cockatrice_dialogue_follows_the_legacy_demo_actor_animation() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/cockatrice-dialogue.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("cockatrice dialogue must declare its legacy demo registration route");
    assert_eq!(route.demo_index, 0x48);
    assert_eq!(route.selected_load_table_symbol, "DEMOTexTbl48");
    assert_eq!(route.selected_pattern_table_symbol, "DEMOPtn48");
    assert_eq!(route.selected_animation_table_symbol, "DEMOAnime48");
    assert_eq!(route.selected_sequence_symbol, "DEMO48");
    assert_eq!(route.registration_set_symbol, "DEMOTex");
    assert_eq!(route.registration_pair_index, 1);
    assert_eq!(route.image_load_index, 1);
    assert_eq!(route.palette_slot, 1);
    assert_eq!(route.texture_slot, 1);

    assert_eq!(asset.members.len(), 2);
    let hurt = &asset.members[0];
    assert_eq!(hurt.region_id, "cockatrice-hurt-retort-dialogue");
    assert_eq!(
        hurt.expected_korean_text,
        "내가 신경 쓰는 걸\n말하다니!\n정말 너무해!"
    );
    let StylizedArtworkConsumer::SequencePattern(hurt_consumer) = &hurt.consumer else {
        panic!("first cockatrice dialogue state must use its actor-creation pattern")
    };
    assert_eq!(hurt_consumer.pattern_record_index, 15);
    assert_eq!(hurt_consumer.sequence_command_offset, 0x216);
    assert_eq!(hurt_consumer.sequence_actor_index, 6);
    assert_eq!(
        hurt_consumer.sequence_operation,
        StylizedArtworkSequenceOperation::CreateActor { flags: 256 }
    );

    let puyo_hell = &asset.members[1];
    assert_eq!(puyo_hell.region_id, "cockatrice-puyo-hell-dialogue");
    assert_eq!(
        puyo_hell.expected_korean_text,
        "미안하단\n말로 끝날\n일이면\n뿌요\n뿌요\n지옥도\n필요\n없지!"
    );
    let StylizedArtworkConsumer::AnimatedPattern(animated) = &puyo_hell.consumer else {
        panic!("second cockatrice dialogue state must use its animation-selected pattern")
    };
    assert_eq!(animated.initial_pattern_record_index, 15);
    assert_eq!(animated.pattern_record_index, 16);
    assert_eq!(animated.create_command_offset, 0x216);
    assert_eq!(animated.animation_command_offset, 0x266);
    assert_eq!(animated.sequence_actor_index, 6);
    assert_eq!(animated.animation_index, 10);
    assert_eq!(animated.animation_table_symbol, "Anime48_MOJI_2");
    assert_eq!(animated.animation_frame_duration, 6);

    for member in &asset.members {
        let declaration = catalog
            .surfaces
            .iter()
            .find(|surface| surface.source_surface_id == asset.source_surface_id)
            .and_then(|surface| {
                surface
                    .regions
                    .iter()
                    .find(|region| region.region_id == member.region_id)
            })
            .expect("cockatrice dialogue state must have a static-region declaration");
        assert_eq!(
            declaration
                .artwork_plan
                .expect("cockatrice dialogue state must have an artwork plan")
                .placement_status,
            GraphicsArtworkPlacementStatus::ConsumerAligned
        );
    }

    let overlay = read_rgba_surface(Path::new(&asset.replacement_overlay_path)).unwrap();
    let mut member_pixels = [0_usize; 2];
    for (pixel, rgba) in overlay.pixels.chunks_exact(4).enumerate() {
        if rgba[3] == 0 {
            continue;
        }
        let x = u32::try_from(pixel % overlay.width).unwrap();
        let y = u32::try_from(pixel / overlay.width).unwrap();
        let member_index = asset
            .members
            .iter()
            .position(|member| point_is_inside(member.native_placement, x, y))
            .expect("cockatrice dialogue overlay pixel leaves both exact placements");
        member_pixels[member_index] += 1;
    }
    assert!(member_pixels.iter().all(|pixels| *pixels > 0));
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_rulue_crying_callout_separates_animation_from_unused_source_art() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/rulue-crying-callouts.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    validate_set_asset_header(&asset).unwrap();
    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("Rulue crying callout must declare its legacy demo registration route");
    assert_eq!(route.demo_index, 0x35);
    assert_eq!(route.selected_load_table_symbol, "DEMOTexTbl35");
    assert_eq!(route.selected_pattern_table_symbol, "DEMOPtn35");
    assert_eq!(route.selected_animation_table_symbol, "DEMOAnime35");
    assert_eq!(route.selected_sequence_symbol, "DEMO35");
    assert_eq!(route.registration_pair_index, 1);
    assert_eq!(route.image_load_index, 1);
    assert_eq!(route.palette_slot, 1);
    assert_eq!(route.texture_slot, 1);

    assert_eq!(asset.members.len(), 2);
    let animated_member = &asset.members[0];
    assert_eq!(animated_member.region_id, "crying-callout-left");
    assert_eq!(animated_member.expected_korean_text, "으아앙~");
    let StylizedArtworkConsumer::AnimatedPattern(animated) = &animated_member.consumer else {
        panic!("left Rulue callout must use the two-frame animation")
    };
    assert_eq!(animated.pattern_record_index, 36);
    assert_eq!(animated.initial_pattern_record_index, 36);
    assert_eq!(animated.create_command_offset, 0xd72);
    assert_eq!(animated.animation_command_offset, 0xd80);
    assert_eq!(animated.sequence_actor_index, 19);
    assert_eq!(animated.animation_index, 9);
    assert_eq!(animated.animation_table_symbol, "Anime35_UAAN_1");
    assert_eq!(animated.animation_frame_duration, 12);
    assert_eq!(animated.animation_terminator, 0xfffe);
    assert_eq!(animated.subsequent_animation_frames.len(), 1);
    assert_eq!(
        animated.subsequent_animation_frames[0].pattern_record_index,
        37
    );
    assert_eq!(
        animated.subsequent_animation_frames[0]
            .derived_logical_screen_bounds
            .y,
        236
    );

    let source_member = &asset.members[1];
    assert_eq!(source_member.region_id, "crying-callout-right");
    assert_eq!(
        source_member.transparent_master,
        animated_member.transparent_master
    );
    let StylizedArtworkConsumer::UnreferencedSource(source) = &source_member.consumer else {
        panic!("right Rulue callout must remain source-aligned without a pattern claim")
    };
    assert_eq!(source.pattern_table_symbol, "DEMOPtn35");
    assert_eq!(
        source.source_bounds,
        PixelBounds {
            x: 136,
            y: 678,
            width: 126,
            height: 42,
        }
    );
    assert_eq!(source.animation_table_symbols.len(), 12);

    let surface = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == asset.source_surface_id)
        .expect("Rulue callout must have a static-region surface");
    assert_eq!(
        surface.regions[0]
            .artwork_plan
            .expect("animated Rulue callout must have an artwork plan")
            .placement_status,
        GraphicsArtworkPlacementStatus::ConsumerAligned
    );
    assert_eq!(
        surface.regions[1]
            .artwork_plan
            .expect("unused Rulue source copy must have an artwork plan")
            .placement_status,
        GraphicsArtworkPlacementStatus::SourceAligned
    );

    let overlay = read_rgba_surface(Path::new(&asset.replacement_overlay_path)).unwrap();
    let mut member_pixels = [0_usize; 2];
    for (pixel, rgba) in overlay.pixels.chunks_exact(4).enumerate() {
        if rgba[3] == 0 {
            continue;
        }
        let x = u32::try_from(pixel % overlay.width).unwrap();
        let y = u32::try_from(pixel / overlay.width).unwrap();
        let member_index = asset
            .members
            .iter()
            .position(|member| point_is_inside(member.native_placement, x, y))
            .expect("Rulue crying overlay pixel leaves both exact placements");
        member_pixels[member_index] += 1;
    }
    assert!(member_pixels.iter().all(|pixels| *pixels > 0));
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_nasu_heartbeat_accepts_one_legacy_demo_member() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/nasu-heartbeat.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    validate_set_asset_header(&asset).unwrap();
    assert_eq!(asset.members.len(), 1);

    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("Nasu heartbeat must declare its legacy demo registration route");
    assert_eq!(route.demo_index, 0x0c);
    assert_eq!(route.selected_load_table_symbol, "DEMOTexTbl0C");
    assert_eq!(route.selected_pattern_table_symbol, "DEMOPtn0C");
    assert_eq!(route.selected_sequence_symbol, "DEMO0C");
    assert_eq!(route.registration_set_symbol, "DEMOTex");
    assert_eq!(route.registration_pair_index, 1);
    assert_eq!(route.image_load_index, 1);
    assert_eq!(route.palette_slot, 1);
    assert_eq!(route.texture_slot, 1);

    let heartbeat = &asset.members[0];
    assert_eq!(heartbeat.region_id, "heartbeat-callout");
    assert_eq!(heartbeat.expected_source_text, "どきゅん");
    assert_eq!(heartbeat.expected_korean_text, "두근");
    let StylizedArtworkConsumer::SequencePattern(consumer) = &heartbeat.consumer else {
        panic!("Nasu heartbeat must use its actor-creation pattern")
    };
    assert_eq!(consumer.image_file_record, 285);
    assert_eq!(consumer.palette_file_record, 615);
    assert_eq!(consumer.pattern_record_index, 9);
    assert_eq!(consumer.sequence_command_offset, 0x150);
    assert_eq!(consumer.sequence_actor_index, 7);
    assert_eq!(
        consumer.sequence_operation,
        StylizedArtworkSequenceOperation::CreateActor { flags: 0 }
    );
    assert_eq!(consumer.pattern_screen_origin.x, 156);
    assert_eq!(consumer.pattern_screen_origin.y, 224);
    assert_eq!(consumer.pattern_depth, 23);

    let declaration = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == asset.source_surface_id)
        .and_then(|surface| {
            surface
                .regions
                .iter()
                .find(|region| region.region_id == heartbeat.region_id)
        })
        .expect("Nasu heartbeat must have a static-region declaration");
    assert_eq!(
        declaration
            .artwork_plan
            .expect("Nasu heartbeat must have an artwork plan")
            .placement_status,
        GraphicsArtworkPlacementStatus::ConsumerAligned
    );
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_masked_satan_label_preserves_the_four_pattern_reading_order() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/masked-satan-abbreviation.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    validate_set_asset_header(&asset).unwrap();
    assert_eq!(asset.members.len(), 4);

    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("Masked Satan label must declare its legacy demo registration route");
    assert_eq!(route.demo_index, 0x41);
    assert_eq!(route.selected_load_table_symbol, "DEMOTexTbl41");
    assert_eq!(route.selected_pattern_table_symbol, "DEMOPtn41");
    assert_eq!(route.selected_sequence_symbol, "DEMO41");
    assert_eq!(route.registration_pair_index, 1);
    assert_eq!(route.image_load_index, 1);
    assert_eq!(route.palette_slot, 1);
    assert_eq!(route.texture_slot, 1);

    let expected_members = [
        ("マ", "마", 29, 0x8bc, 8, (320, 232), (322, 235, 59, 59)),
        ("ス", "스", 30, 0x8d4, 9, (320, 304), (322, 312, 56, 50)),
        ("カ", "카", 31, 0x8ec, 10, (248, 232), (256, 234, 51, 62)),
        ("ド", "도", 32, 0x904, 11, (248, 304), (263, 304, 47, 62)),
    ];
    for (member, expected) in asset.members.iter().zip(expected_members) {
        let (source, korean, pattern, offset, actor, origin, logical) = expected;
        assert_eq!(member.expected_source_text, source);
        assert_eq!(member.expected_korean_text, korean);
        let StylizedArtworkConsumer::SequencePattern(consumer) = &member.consumer else {
            panic!("Masked Satan glyph must use its actor-creation pattern")
        };
        assert_eq!(consumer.image_file_record, 297);
        assert_eq!(consumer.palette_file_record, 627);
        assert_eq!(consumer.pattern_record_index, pattern);
        assert_eq!(consumer.sequence_command_offset, offset);
        assert_eq!(consumer.sequence_actor_index, actor);
        assert_eq!(
            consumer.sequence_operation,
            StylizedArtworkSequenceOperation::CreateActor { flags: 0 }
        );
        assert_eq!(
            (
                consumer.pattern_screen_origin.x,
                consumer.pattern_screen_origin.y
            ),
            origin
        );
        assert_eq!(consumer.pattern_depth, 19);
        assert_eq!(
            (
                consumer.derived_logical_screen_bounds.x,
                consumer.derived_logical_screen_bounds.y,
                consumer.derived_logical_screen_bounds.width,
                consumer.derived_logical_screen_bounds.height,
            ),
            logical
        );

        let declaration = catalog
            .surfaces
            .iter()
            .find(|surface| surface.source_surface_id == asset.source_surface_id)
            .and_then(|surface| {
                surface
                    .regions
                    .iter()
                    .find(|region| region.region_id == member.region_id)
            })
            .expect("Masked Satan glyph must have a static-region declaration");
        assert_eq!(
            declaration
                .artwork_plan
                .expect("Masked Satan glyph must have an artwork plan")
                .placement_status,
            GraphicsArtworkPlacementStatus::ConsumerAligned
        );
    }
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_draco_startle_uses_the_parent_relative_pattern_origin() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-callouts/draco-startle.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();

    validate_set_asset_header(&asset).unwrap();
    assert_eq!(asset.members.len(), 1);
    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("Draco startle must declare its legacy demo registration route");
    assert_eq!(route.demo_index, 0x33);
    assert_eq!(route.selected_load_table_symbol, "DEMOTexTbl33");
    assert_eq!(route.selected_pattern_table_symbol, "DEMOPtn33");
    assert_eq!(route.selected_sequence_symbol, "DEMO33");
    assert_eq!(route.registration_pair_index, 1);
    assert_eq!(route.image_load_index, 1);
    assert_eq!(route.palette_slot, 1);
    assert_eq!(route.texture_slot, 1);

    let startle = &asset.members[0];
    assert_eq!(startle.expected_source_text, "ぎくっ");
    assert_eq!(startle.expected_korean_text, "움찔");
    let StylizedArtworkConsumer::SequencePattern(consumer) = &startle.consumer else {
        panic!("Draco startle must use its child-actor pattern")
    };
    assert_eq!(consumer.image_file_record, 305);
    assert_eq!(consumer.palette_file_record, 635);
    assert_eq!(consumer.pattern_record_index, 17);
    assert_eq!(consumer.sequence_command_offset, 0x20c);
    assert_eq!(consumer.sequence_actor_index, 7);
    assert_eq!(
        consumer.sequence_operation,
        StylizedArtworkSequenceOperation::CreateChildActor {
            flags: 0,
            parent_actor_index: 4,
            parent_create_command_offset: 0x60,
            parent_create_opcode: 0x10,
            parent_create_command_words: 7,
            parent_create_case_address: 0x0045_7be8,
        }
    );
    assert_eq!(consumer.sequence_case_address, 0x0045_84c8);
    assert_eq!(consumer.pattern_screen_origin.x, 202);
    assert_eq!(consumer.pattern_screen_origin.y, 218);
    assert_eq!(consumer.pattern_depth, 21);
    assert_eq!(consumer.derived_logical_screen_bounds.x, 210);
    assert_eq!(consumer.derived_logical_screen_bounds.y, 223);
    assert_eq!(consumer.derived_logical_screen_bounds.width, 64);
    assert_eq!(consumer.derived_logical_screen_bounds.height, 78);

    let declaration = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == asset.source_surface_id)
        .and_then(|surface| {
            surface
                .regions
                .iter()
                .find(|region| region.region_id == startle.region_id)
        })
        .expect("Draco startle must have a static-region declaration");
    assert_eq!(
        declaration
            .artwork_plan
            .expect("Draco startle must have an artwork plan")
            .placement_status,
        GraphicsArtworkPlacementStatus::ConsumerAligned
    );
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_puzzle_completion_banner_uses_the_full_screen_ending_pattern() {
    let set: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/puzzle-completion/congratulations.json",
    ))
    .unwrap();
    assert_eq!(set.members.len(), 1);
    let asset = set.members.into_iter().next().unwrap();
    assert_eq!(asset.expected_korean_text, "클리어 축하해!");
    assert_eq!(
        asset.source_palette_selection,
        StylizedArtworkPaletteSelection::OpaqueSourceMask
    );
    assert_eq!(asset.native_placement.x, 300);
    assert_eq!(asset.native_placement.y, 114);
    assert_eq!(asset.native_placement.width, 246);
    assert_eq!(asset.native_placement.height, 158);

    let StylizedArtworkConsumer::DirectPatternDisplay(consumer) = asset.consumer else {
        panic!("puzzle completion banner must use the direct-pattern display consumer")
    };
    assert_eq!(consumer.load_route.load_table_symbol, "NazoEndTexTbl");
    assert_eq!(consumer.load_route.image_load_index, 0);
    assert_eq!(consumer.load_route.image_file_record, 116);
    assert_eq!(consumer.load_route.palette_slot, 0);
    assert_eq!(consumer.load_route.palette_file_record, 237);
    assert_eq!(consumer.pattern_table_symbol, "NazoEnd_Ptn");
    assert_eq!(consumer.pattern_record_index, 0);
    assert_eq!(consumer.pattern_texture_slot, 0);
    assert_eq!(consumer.pattern_source_bounds.width, 640);
    assert_eq!(consumer.pattern_source_bounds.height, 448);
    assert_eq!(consumer.actor_symbol, "Nazo_Ending_Run");
    assert_eq!(consumer.pattern_screen_origin.x, 0);
    assert_eq!(consumer.pattern_screen_origin.y, 0);
    assert_eq!(consumer.pattern_depth, 10);
    assert_eq!(consumer.derived_logical_screen_bounds.x, 330);
    assert_eq!(consumer.derived_logical_screen_bounds.y, 128);
    assert_eq!(consumer.derived_logical_screen_bounds.width, 187);
    assert_eq!(consumer.derived_logical_screen_bounds.height, 124);
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_puzzle_menu_headings_cover_both_indexed_games_and_my_puzzle() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/puzzle-menu-headings/labels.json",
    ))
    .unwrap();
    assert_eq!(
        asset
            .base_replacement_overlay
            .as_ref()
            .map(|base| base.path.as_str()),
        Some("assets/translation/graphics/static-ui/image-g-000108.roles.png")
    );
    assert_eq!(asset.members.len(), 3);
    assert!(
        asset
            .members
            .iter()
            .all(|member| member.expected_korean_text == "퍼즐뿌요")
    );

    for (member, expected_selector) in asset.members[..2].iter().zip([0, 1]) {
        let StylizedArtworkConsumer::IndexedPatternDisplay(consumer) = &member.consumer else {
            panic!("puzzle-game heading must use the indexed-pattern display consumer")
        };
        assert_eq!(consumer.selector_value, expected_selector);
        assert_eq!(
            consumer.pattern_record_index,
            consumer.base_pattern_record_index + usize::from(expected_selector)
        );
        assert_eq!(consumer.registration_route.texture_slot, 3);
        assert_eq!(consumer.load_route.image_file_record, 108);
        assert_eq!(consumer.load_route.palette_file_record, 230);
    }

    let StylizedArtworkConsumer::DirectPatternDisplay(my_puzzle) = &asset.members[2].consumer
    else {
        panic!("My puzzle heading must use the direct-pattern display consumer")
    };
    assert_eq!(my_puzzle.pattern_record_index, 14);
    assert_eq!(my_puzzle.pattern_screen_origin.x, 184);
    assert_eq!(my_puzzle.pattern_screen_origin.y, 64);
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_opponent_confirmation_uses_registered_text_and_protected_arrow_patterns() {
    let asset: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/opponent-selection/confirmation-callout.json",
    ))
    .unwrap();
    assert_eq!(asset.expected_korean_text, "결정!");
    assert_eq!(
        asset.source_palette_selection,
        StylizedArtworkPaletteSelection::OpaqueSourceMask
    );
    let StylizedArtworkConsumer::DirectPatternDisplay(consumer) = asset.consumer else {
        panic!("opponent confirmation must use the direct-pattern display consumer")
    };
    let registration = consumer
        .registration_route
        .expect("opponent confirmation must declare texture registration");
    assert_eq!(registration.registration_set_symbol, "MenuTex_4");
    assert_eq!(registration.texture_slot, 9);
    assert_eq!(consumer.pattern_record_index, 93);
    assert_eq!(consumer.pattern_screen_origin.x, 24);
    assert_eq!(consumer.pattern_screen_origin.y, 352);
    assert_eq!(consumer.protected_pattern_uses.len(), 1);
    assert_eq!(consumer.protected_pattern_uses[0].pattern_record_index, 94);
    assert_eq!(
        consumer.protected_pattern_uses[0].pattern_setter_symbol,
        "Act_SetPtn"
    );
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_battle_callout_set_covers_the_shared_texture_consumers() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/battle-effects/callouts.json",
    ))
    .unwrap();
    assert_eq!(
        asset
            .base_replacement_overlay
            .as_ref()
            .map(|base| base.path.as_str()),
        Some("assets/translation/graphics/static-ui/image-g-000013.roles.png")
    );
    assert_eq!(
        asset
            .members
            .iter()
            .map(|member| member.expected_korean_text.as_str())
            .collect::<Vec<_>>(),
        ["휴식 중", "휴식 중", "야호!", "꽈당큐~", "싹쓸이!", "상쇄"]
    );
    let route = asset
        .sequential_registration_route
        .as_ref()
        .expect("battle callouts must declare their sequential registration route");
    assert_eq!(route.predecessor_load_table_symbol, "FrameTexTbl_05");
    assert_eq!(route.pointer_table_symbol, "GameTexTbl");
    assert_eq!(route.texture_slot, 4);

    let mut pattern_records = 0;
    let mut direct_patterns = 0;
    let mut actor_texture_boxes = 0;
    for member in &asset.members {
        let StylizedArtworkConsumer::BattleTexture(consumer) = &member.consumer else {
            panic!("battle callout must use the battle-texture consumer")
        };
        match &consumer.texture_use {
            StylizedArtworkBattleTextureUse::PatternRecord { .. } => pattern_records += 1,
            StylizedArtworkBattleTextureUse::DirectPattern { .. } => direct_patterns += 1,
            StylizedArtworkBattleTextureUse::ActorTextureBox { .. } => actor_texture_boxes += 1,
            StylizedArtworkBattleTextureUse::CharacterPatternRecord { .. }
            | StylizedArtworkBattleTextureUse::CharacterActionPatternRecord { .. }
            | StylizedArtworkBattleTextureUse::FaceConditionPatternRecord { .. }
            | StylizedArtworkBattleTextureUse::FaceActionPatternRecord { .. }
            | StylizedArtworkBattleTextureUse::SourceOnlyRegion { .. } => {
                panic!("shared battle callouts must not use character pattern records")
            }
        }
    }
    assert_eq!(
        (pattern_records, direct_patterns, actor_texture_boxes),
        (2, 3, 1)
    );

    let offset = asset.members.last().unwrap();
    assert_eq!(
        offset.source_palette_selection,
        StylizedArtworkPaletteSelection::ExactDeclaredRoles
    );
    assert_eq!(offset.source_palette_roles.len(), 1);
    assert_eq!(offset.source_palette_roles[0].palette_index, 108);
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_puzzle_callout_set_uses_the_nazo_texture_registration() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/battle-effects/puzzle-callouts.json",
    ))
    .unwrap();
    assert!(asset.sequential_registration_route.is_none());
    let route = asset
        .indexed_registration_route
        .as_ref()
        .expect("puzzle callouts must declare their indexed registration route");
    assert_eq!(route.registration.load_table_symbol, "NazoTexTbl");
    assert_eq!(
        route.registration.registration_set_symbol,
        "Nazo_TexReg_Game"
    );
    assert_eq!(route.registration.texture_slot, 4);
    assert_eq!(route.registration.image_load_index, 4);
    assert_eq!(route.registration.palette_slot, 4);
    assert_eq!(route.file_id, 1);
    assert_eq!(route.load_actor_symbol, "Nazo_Menu_Init");
    assert_eq!(route.registration_actor_symbol, "Puyo_Nazo_Run");
    assert_eq!(
        asset
            .members
            .iter()
            .map(|member| member.expected_korean_text.as_str())
            .collect::<Vec<_>>(),
        ["휴식 중", "휴식 중", "야호!", "꽈당큐~", "싹쓸이!", "상쇄"]
    );

    for member in &asset.members {
        let StylizedArtworkConsumer::BattleTexture(consumer) = &member.consumer else {
            panic!("puzzle callout must use the battle-texture consumer")
        };
        assert_eq!(consumer.load_route.load_table_symbol, "NazoTexTbl");
        assert_eq!(consumer.load_route.image_load_index, 4);
        assert_eq!(consumer.load_route.image_file_record, 117);
        assert_eq!(consumer.load_route.palette_slot, 4);
        assert_eq!(consumer.load_route.palette_file_record, 136);
        assert_eq!(consumer.pattern_texture_slot, 4);
    }
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_nomi_callout_set_uses_character_face_registration_and_state_patterns() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/battle-effects/nomi-callouts.json",
    ))
    .unwrap();
    assert!(asset.sequential_registration_route.is_none());
    assert!(asset.indexed_registration_route.is_none());
    let route = asset
        .character_face_registration_route
        .as_ref()
        .expect("Nomi callouts must declare character-face registration");
    assert_eq!(route.face_load_table_pointer_symbol, "GameFaceTexTbl");
    assert_eq!(route.character_index, 10);
    assert_eq!(route.selected_face_load_table_symbol, "FaceTex_Nomi");
    assert_eq!(route.registration_set_symbol, "FaceTexList");
    assert_eq!(route.image_load_index, 1);
    assert_eq!(route.registration_palette_slot, 8);
    assert_eq!(route.texture_slot, 7);

    assert_eq!(
        asset
            .members
            .iter()
            .map(|member| member.expected_korean_text.as_str())
            .collect::<Vec<_>>(),
        ["야호!", "꽈당큐~"]
    );
    for (member, expected_record) in asset.members.iter().zip([5, 6]) {
        let StylizedArtworkConsumer::BattleTexture(consumer) = &member.consumer else {
            panic!("Nomi callout must use the battle-texture consumer")
        };
        assert_eq!(consumer.load_route.load_table_symbol, "FaceTex_Nomi");
        assert_eq!(consumer.load_route.image_file_record, 38);
        assert_eq!(consumer.load_route.palette_file_record, 161);
        assert_eq!(consumer.pattern_texture_slot, 7);
        let StylizedArtworkBattleTextureUse::CharacterPatternRecord {
            pattern_pointer_table_symbol,
            character_index,
            pattern_table_symbol,
            pattern_record_index,
            ..
        } = &consumer.texture_use
        else {
            panic!("Nomi callout must use a character pattern record")
        };
        assert_eq!(pattern_pointer_table_symbol, "CHR_Ptn");
        assert_eq!(*character_index, 10);
        assert_eq!(pattern_table_symbol, "CHR_Nomi");
        assert_eq!(*pattern_record_index, expected_record);
    }
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_nomi_reaction_labels_separate_source_and_face_action_alignment() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/character-reactions/nomi-reaction-labels.json",
    ))
    .unwrap();
    let route = asset
        .character_face_registration_route
        .as_ref()
        .expect("Nomi reaction labels must declare character-face registration");
    let fixed = route
        .fixed_face_pattern
        .as_ref()
        .expect("Nomi reaction labels must declare the fixed face-pattern texture");
    assert_eq!(fixed.image_file_record, 17);
    assert_eq!(fixed.palette_file_record, 140);
    assert_eq!(fixed.texture_slot, 8);
    assert_eq!(fixed.registration_function_symbol, "ActRegList_FacePtn");
    assert_eq!(asset.members.len(), 7);
    assert_eq!(
        asset
            .members
            .iter()
            .map(|member| member.expected_korean_text.as_str())
            .collect::<Vec<_>>(),
        [
            "여기 있어",
            "야호!",
            "야호!",
            "여기 있어",
            "휴…",
            "기뻐하고 있어",
            "기뻐하고 있어"
        ]
    );
    let mut source_only_regions = 0;
    let mut face_action_patterns = 0;
    for member in &asset.members {
        let StylizedArtworkConsumer::BattleTexture(consumer) = &member.consumer else {
            panic!("Nomi reaction label must use the battle-texture consumer")
        };
        assert_eq!(consumer.load_route.image_file_record, 17);
        assert_eq!(consumer.load_route.palette_file_record, 140);
        assert_eq!(consumer.pattern_texture_slot, 8);
        match &consumer.texture_use {
            StylizedArtworkBattleTextureUse::SourceOnlyRegion { .. } => source_only_regions += 1,
            StylizedArtworkBattleTextureUse::FaceActionPatternRecord { .. } => {
                face_action_patterns += 1
            }
            _ => panic!("Nomi reaction label uses an unexpected texture route"),
        }
    }
    assert_eq!((source_only_regions, face_action_patterns), (1, 6));
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_top_menu_mode_set_covers_three_states_per_semantic_label() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/top-menu-mode-selection/mode-labels.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();
    let surface = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == asset.source_surface_id)
        .expect("top-menu set must have a static-region declaration");
    assert!(asset.base_replacement_overlay.is_none());
    assert_eq!(
        asset
            .members
            .iter()
            .map(|member| member.expected_korean_text.as_str())
            .collect::<Vec<_>>(),
        [
            "혼자서 뿌요뿌요",
            "둘이서 뿌요뿌요",
            "옵션",
            "특훈 뿌요뿌요",
            "퍼즐뿌요",
            "모드를 선택해 주세요"
        ]
    );
    assert!(asset.maximum_source_palette_colors_per_member.is_none());

    let expected_pointer_indices = [0, 1, 4, 2, 3];
    for (member, expected_pointer_index) in asset.members.iter().zip(expected_pointer_indices) {
        assert_eq!(
            member.source_palette_selection,
            StylizedArtworkPaletteSelection::OpaqueSourceMask
        );
        let StylizedArtworkConsumer::ModeSelectionStates(consumer) = &member.consumer else {
            panic!("top-menu label must use the mode-selection-states consumer")
        };
        assert_eq!(consumer.load_route.image_file_record, 49);
        assert_eq!(consumer.load_route.palette_file_record, 109);
        assert_eq!(consumer.pattern_texture_slot, 3);
        assert_eq!(consumer.pattern_pointer_table_index, expected_pointer_index);
        assert_eq!(consumer.state_pattern_record_indices, [0, 1, 2]);
        assert_eq!(consumer.state_pattern_source_bounds.len(), 3);
        assert_eq!(consumer.state_source_mask_bounds.len(), 3);
        assert_eq!(consumer.state_artwork_bounds.len(), 3);
        assert_eq!(
            consumer.state_artwork_bounds,
            consumer.state_pattern_source_bounds
        );
        assert_eq!(
            consumer.state_artwork_bounds.first().copied(),
            Some(member.native_placement)
        );
        assert_eq!(member.additional_variant_master_crops.len(), 2);
        assert!(member.generation_prompt.as_deref().is_some_and(|prompt| {
            member
                .expected_korean_text
                .chars()
                .filter(|character| !character.is_whitespace())
                .all(|character| prompt.contains(character))
                && prompt.contains("body")
                && prompt.contains("ground-shadow")
        }));
        let master =
            read_rgba_surface(Path::new(&member.transparent_master.as_ref().unwrap().path))
                .unwrap();
        assert!(master.pixels.chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(master.pixels.chunks_exact(4).any(|pixel| pixel[3] == 255));
        assert_eq!(consumer.blank_pattern_record_index, 3);
        let declaration = surface
            .regions
            .iter()
            .find(|region| region.region_id == member.region_id)
            .expect("top-menu member must have a matching static region");
        assert_eq!(
            declaration.source_restoration_bounds,
            consumer.state_pattern_source_bounds
        );
        assert_eq!(
            declaration.artwork_composition_bounds,
            Some(enclosing_pixel_bounds(&consumer.state_pattern_source_bounds).unwrap())
        );
        assert_eq!(declaration.source_mask_dilation_radius, 2);
        assert!(!declaration.protected_subregions.is_empty());
    }
}

#[test]
#[ignore = "requires assets/translation/graphics/ and config/static-graphics-regions.json"]
fn tracked_title_branding_stays_inside_its_complete_and_split_body_patterns() {
    let asset: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/title/title-logo.json",
    ))
    .unwrap();
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();
    let StylizedArtworkConsumer::TitleBranding(consumer) = &asset.consumer else {
        panic!("title logo must use the title-branding consumer")
    };
    assert_eq!(consumer.load_route.load_table_symbol, "TitleTexTbl");
    assert_eq!(consumer.load_route.image_file_record, 19);
    assert_eq!(consumer.load_route.palette_file_record, 79);
    assert_eq!(
        consumer
            .registration_routes
            .iter()
            .map(|route| route.registration_set_symbol.as_str())
            .collect::<Vec<_>>(),
        ["TitleTex_1", "TitleTex_2"]
    );
    assert_eq!(consumer.body_pattern.pattern_record_index, 2);
    assert_eq!(consumer.stored_body_text, "뿌요");
    assert_eq!(consumer.assembled_body_text, "뿌요뿌요");
    assert_eq!(
        consumer
            .body_fragment_patterns
            .iter()
            .map(|fragment| fragment.pattern_record_index)
            .collect::<Vec<_>>(),
        [43, 44]
    );
    assert_eq!(
        consumer
            .body_fragment_patterns
            .iter()
            .map(|fragment| fragment.stored_korean_text.as_str())
            .collect::<Vec<_>>(),
        ["뿌", "요"]
    );
    assert_eq!(consumer.sequence_symbol, "TitleDemo");
    assert_eq!(consumer.sequence_opcode, 0x10);
    assert_eq!(consumer.sequence_command_words, 7);
    assert_eq!(consumer.sequence_case_address, 0x0045_7be8);
    assert_eq!(consumer.pattern_setter_symbol, "Act_SetPtn");
    assert_eq!(
        consumer
            .assembly_commands
            .iter()
            .map(|command| command.sequence_command_offset)
            .collect::<Vec<_>>(),
        [0x45c, 0x46a, 0x478, 0x486]
    );
    assert_eq!(
        consumer
            .assembly_commands
            .iter()
            .map(|command| command.pattern_record_index)
            .collect::<Vec<_>>(),
        [43, 44, 43, 44]
    );
    assert_eq!(consumer.suffix_pattern.pattern_record_index, 3);
    assert_eq!(consumer.protected_english_pattern.pattern_record_index, 45);
    let declaration = catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == asset.source_surface_id)
        .and_then(|surface| {
            surface
                .regions
                .iter()
                .find(|region| region.region_id == asset.region_id)
        })
        .expect("title logo must have a static-region declaration");
    assert_eq!(
        declaration.source_restoration_bounds,
        [
            consumer.body_pattern.pattern_source_bounds,
            consumer.suffix_pattern.pattern_source_bounds,
        ]
    );

    let overlay = read_rgba_surface(Path::new(&asset.replacement_overlay_path)).unwrap();
    let mut visible_pixels = 0_usize;
    let mut pixels_clipped_by_fragments = BTreeMap::<u32, usize>::new();
    for (pixel, rgba) in overlay.pixels.chunks_exact(4).enumerate() {
        if rgba[3] == 0 {
            continue;
        }
        visible_pixels += 1;
        let x = u32::try_from(pixel % overlay.width).unwrap();
        let y = u32::try_from(pixel / overlay.width).unwrap();
        assert!(point_is_inside(
            consumer.body_pattern.pattern_source_bounds,
            x,
            y
        ));
        if !consumer
            .body_fragment_patterns
            .iter()
            .any(|fragment| point_is_inside(fragment.pattern_source_bounds, x, y))
        {
            *pixels_clipped_by_fragments.entry(x).or_default() += 1;
        }
    }
    assert!(visible_pixels > 0);
    assert!(
        pixels_clipped_by_fragments.is_empty(),
        "title overlay pixels remain in the split-animation gap: {pixels_clipped_by_fragments:?}"
    );
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn consumer_specific_report_boundary_describes_the_verified_path() {
    let mode_logo: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/mode-selection/puyo-tsu-logo.json",
    ))
    .unwrap();
    let tutorial_callout: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/tutorial/all-clear-callout.json",
    ))
    .unwrap();
    let story_panels: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/story-mode-selection/panel-labels.json",
    ))
    .unwrap();
    let rule_heading: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/rule-selection/selection-heading.json",
    ))
    .unwrap();
    let opponent_confirmation: StylizedArtworkAsset =
        serde_json::from_slice(&crate::test_input::read(
            "assets/translation/graphics/opponent-selection/confirmation-callout.json",
        ))
        .unwrap();
    let top_menu_modes: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/top-menu-mode-selection/mode-labels.json",
    ))
    .unwrap();
    let title_logo: StylizedArtworkAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/title/title-logo.json",
    ))
    .unwrap();

    let mode_boundary = mode_logo.consumer.report_evidence_boundary();
    assert!(mode_boundary.contains("scrolling-tile overlay"));
    assert!(mode_boundary.contains("actor movement and wrap instructions"));
    assert!(!mode_boundary.contains("sequence command"));

    let tutorial_boundary = tutorial_callout.consumer.report_evidence_boundary();
    assert!(tutorial_boundary.contains("sequence command"));
    assert!(!tutorial_boundary.contains("scrolling-tile overlay"));

    let story_boundary = story_panels.members[0].consumer.report_evidence_boundary();
    assert!(story_boundary.contains("menu-panel set"));
    assert!(story_boundary.contains("actor pattern selection and screen positioning"));
    assert!(!story_boundary.contains("scrolling-tile overlay"));

    let rule_boundary = rule_heading.consumer.report_evidence_boundary();
    assert!(rule_boundary.contains("direct display construction and screen position"));
    assert!(!rule_boundary.contains("texture registration"));
    assert!(!rule_boundary.contains("protected companion pattern use"));
    assert!(!rule_boundary.contains("sequence command"));

    let opponent_boundary = opponent_confirmation.consumer.report_evidence_boundary();
    assert!(opponent_boundary.contains("texture registration"));
    assert!(opponent_boundary.contains("protected companion pattern use"));

    let top_menu_boundary = top_menu_modes.members[0]
        .consumer
        .report_evidence_boundary();
    assert!(top_menu_boundary.contains("three native mode-selection states"));
    assert!(top_menu_boundary.contains("blank fourth frame"));
    assert!(!top_menu_boundary.contains("scrolling-tile overlay"));

    let title_boundary = title_logo.consumer.report_evidence_boundary();
    assert!(title_boundary.contains("exact title-body pattern"));
    assert!(title_boundary.contains("split-animation body fragments"));
    assert!(title_boundary.contains("TitleDemo 43-44-43-44 repeat assembly"));
    assert!(title_boundary.contains("typed SeqCommand decoder"));
    assert!(title_boundary.contains("protected English badge pattern"));
}
