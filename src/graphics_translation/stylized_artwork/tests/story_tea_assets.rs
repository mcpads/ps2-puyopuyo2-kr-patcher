use std::collections::BTreeMap;

use super::*;

const MASTER_PATH: &str =
    "assets/translation/graphics/character-reactions/skeleton-tea-label-master.png";
const MASTER_SHA256: &str = "5227cfc9f8936e8d47827288ee9b8039b80041f560b377e5966c49ddeaa5c9b8";

fn catalog_bounds(source_surface_id: &str) -> BTreeMap<String, PixelBounds> {
    let catalog: StaticRegionCatalog = serde_json::from_slice(&crate::test_input::read(
        "config/static-graphics-regions.json",
    ))
    .unwrap();
    catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == source_surface_id)
        .unwrap_or_else(|| panic!("missing static-region surface {source_surface_id}"))
        .regions
        .iter()
        .map(|region| (region.region_id.clone(), region.bounds))
        .collect()
}

fn assert_shared_story_label_contract(asset: &StylizedArtworkSetAsset, demo_index: usize) {
    validate_set_asset_header(asset).unwrap();
    assert_eq!(
        asset.artwork_method,
        GraphicsArtworkMethod::DeterministicRaster
    );
    let route = asset
        .legacy_demo_registration_route
        .as_ref()
        .expect("story tea labels must declare their legacy demo route");
    let suffix = format!("{demo_index:02X}");
    assert_eq!(route.demo_index, demo_index);
    assert_eq!(
        route.selected_load_table_symbol,
        format!("DEMOTexTbl{suffix}")
    );
    assert_eq!(
        route.selected_pattern_table_symbol,
        format!("DEMOPtn{suffix}")
    );
    assert_eq!(
        route.selected_animation_table_symbol,
        format!("DEMOAnime{suffix}")
    );
    assert_eq!(route.selected_sequence_symbol, format!("DEMO{suffix}"));
    assert_eq!(
        (
            route.image_load_index,
            route.palette_slot,
            route.texture_slot
        ),
        (1, 1, 1)
    );

    let expected_bounds = catalog_bounds(&asset.source_surface_id);
    assert_eq!(asset.members.len(), expected_bounds.len());
    for member in &asset.members {
        let master = member
            .transparent_master
            .as_ref()
            .expect("isolated story label must own its master");
        assert_eq!(master.path, MASTER_PATH);
        assert_eq!(master.sha256, MASTER_SHA256);
        assert_eq!(
            expected_bounds.get(&member.region_id),
            Some(&member.native_placement),
            "{} does not use the reviewed source footprint",
            member.region_id
        );
    }
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn demo_08_covers_six_animation_frames_and_the_direct_pattern_selection() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/character-reactions/skeleton-tea-label-story-08.json",
    ))
    .unwrap();
    assert_shared_story_label_contract(&asset, 8);

    let mut patterns = Vec::new();
    for member in &asset.members {
        match &member.consumer {
            StylizedArtworkConsumer::ScriptedAnimation(consumer) => {
                patterns.push(consumer.pattern_record_index);
                assert_eq!(consumer.image_file_record, 300);
                assert_eq!(consumer.palette_file_record, 630);
                assert_eq!(consumer.sequence_actor_index, 2);
            }
            StylizedArtworkConsumer::SequencePattern(consumer) => {
                patterns.push(consumer.pattern_record_index);
                assert_eq!(consumer.pattern_record_index, 8);
                assert_eq!(consumer.sequence_command_offset, 0x146);
                assert_eq!(consumer.sequence_opcode, 24);
                let StylizedArtworkSequenceOperation::SetPattern {
                    create_command_offset,
                    initial_pattern_record_index,
                    ..
                } = consumer.sequence_operation
                else {
                    panic!("DEMO08 record 8 must use the actor pattern-selection command")
                };
                assert_eq!(create_command_offset, 0x1c);
                assert_eq!(initial_pattern_record_index, 2);
            }
            _ => panic!("story tea label uses an unrelated consumer"),
        }
    }
    assert_eq!(patterns, (2..=8).collect::<Vec<_>>());

    let looping = match &asset.members[1].consumer {
        StylizedArtworkConsumer::ScriptedAnimation(consumer) => consumer,
        _ => panic!("DEMO08 record 3 must use Anime08_SUKT_1"),
    };
    assert_eq!(looping.animation_table_symbol, "Anime08_SUKT_1");
    assert_eq!(
        (
            looping.animation_terminator,
            looping.animation_terminator_operand
        ),
        (0xfffe, 0)
    );
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn demo_09_covers_every_tea_pose_and_preserves_the_loop_target() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/character-reactions/skeleton-tea-label-story-09.json",
    ))
    .unwrap();
    assert_shared_story_label_contract(&asset, 9);

    let mut patterns = Vec::new();
    for member in &asset.members {
        let StylizedArtworkConsumer::ScriptedAnimation(consumer) = &member.consumer else {
            panic!("DEMO09 tea pose must be selected through a declared animation record")
        };
        patterns.push(consumer.pattern_record_index);
        assert_eq!(consumer.image_file_record, 302);
        assert_eq!(consumer.palette_file_record, 632);
        assert_eq!(consumer.sequence_actor_index, 2);
        if (7..=10).contains(&consumer.pattern_record_index) {
            assert_eq!(consumer.animation_table_symbol, "Anime09_SUKT_3");
            assert_eq!(
                (
                    consumer.animation_terminator,
                    consumer.animation_terminator_operand
                ),
                (0xfffe, 1)
            );
        }
    }
    assert_eq!(patterns, (2..=24).collect::<Vec<_>>());
}
