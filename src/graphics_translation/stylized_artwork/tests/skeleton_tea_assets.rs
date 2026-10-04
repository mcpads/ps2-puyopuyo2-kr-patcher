use crate::graphics_translation::deterministic_label::DeterministicLabelMasterConfiguration;
use crate::indexed_texture::sha256;

use super::*;

const MASTER_PATH: &str =
    "assets/translation/graphics/character-reactions/skeleton-tea-label-master.png";
const MASTER_SHA256: &str = "5227cfc9f8936e8d47827288ee9b8039b80041f560b377e5966c49ddeaa5c9b8";

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_skeleton_tea_master_has_fixed_font_and_bytes() {
    let configuration: DeterministicLabelMasterConfiguration = serde_json::from_slice(
        include_bytes!("../../../../config/deterministic-tea-label.json"),
    )
    .unwrap();
    assert_eq!(configuration.schema, 1);
    assert_eq!(configuration.id, "skeleton-t-tea-label");
    assert_eq!(configuration.text, "차");
    assert_eq!(configuration.font_id, "galmuri11-functional-ui");
    assert_eq!(configuration.pixel_size, 56);
    assert_eq!(configuration.coverage_threshold, 127);
    assert_eq!(configuration.outline_radius, 2);
    assert_eq!(configuration.fill_rgba, [255, 64, 32, 255]);
    assert_eq!(configuration.outline_rgba, [96, 0, 0, 255]);
    assert_eq!(
        sha256(&crate::test_input::read(
            "assets/translation/graphics/character-reactions/skeleton-tea-label-master.png"
        )),
        MASTER_SHA256
    );
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_skeleton_battle_cup_uses_the_regular_character_texture_path() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/character-reactions/skeleton-tea-label-battle.json",
    ))
    .unwrap();
    validate_set_asset_header(&asset).unwrap();
    assert_eq!(
        asset.artwork_method,
        GraphicsArtworkMethod::DeterministicRaster
    );
    assert_eq!(asset.members.len(), 1);

    let route = asset
        .character_face_registration_route
        .as_ref()
        .expect("Skeleton T cup must declare character-face registration");
    assert_eq!(route.character_index, 6);
    assert_eq!(route.selected_face_load_table_symbol, "FaceTex_Ske_t");
    assert_eq!(route.image_load_index, 1);
    assert_eq!(route.palette_load_index, 1);
    assert_eq!(route.registration_palette_slot, 8);
    assert_eq!(route.texture_slot, 7);

    let member = &asset.members[0];
    let master = member
        .transparent_master
        .as_ref()
        .expect("isolated Skeleton T label must own its master");
    assert_eq!(master.path, MASTER_PATH);
    assert_eq!(master.sha256, MASTER_SHA256);
    assert_eq!(
        member.native_placement,
        PixelBounds {
            x: 8,
            y: 33,
            width: 18,
            height: 18
        }
    );
    let StylizedArtworkConsumer::BattleTexture(consumer) = &member.consumer else {
        panic!("Skeleton T battle cup must use a battle texture")
    };
    assert_eq!(consumer.load_route.image_file_record, 39);
    assert_eq!(consumer.load_route.palette_file_record, 162);
    assert_eq!(consumer.pattern_texture_slot, 7);
    assert_eq!(consumer.actor_symbol, "set_default_win_act");
    let StylizedArtworkBattleTextureUse::CharacterActionPatternRecord {
        pattern_pointer_table_symbol,
        character_index,
        pattern_table_symbol,
        pattern_record_index,
        action_pointer_table_symbol,
        action_table_symbol,
        action_pattern_indices,
        ..
    } = &consumer.texture_use
    else {
        panic!("Skeleton T battle cup must bind its victory action")
    };
    assert_eq!(pattern_pointer_table_symbol, "CHR_Ptn");
    assert_eq!(*character_index, 6);
    assert_eq!(pattern_table_symbol, "CHR_Skel");
    assert_eq!(*pattern_record_index, 0);
    assert_eq!(action_pointer_table_symbol, "CHR_Win_Anime");
    assert_eq!(action_table_symbol, "CHR_Win_Skel");
    assert_eq!(action_pattern_indices, &[0]);
}

#[test]
#[ignore = "requires assets/translation/graphics/"]
fn tracked_skeleton_face_cup_covers_every_consumed_label_frame() {
    let asset: StylizedArtworkSetAsset = serde_json::from_slice(&crate::test_input::read(
        "assets/translation/graphics/character-reactions/skeleton-tea-label-face.json",
    ))
    .unwrap();
    validate_set_asset_header(&asset).unwrap();
    assert_eq!(
        asset.artwork_method,
        GraphicsArtworkMethod::DeterministicRaster
    );
    assert_eq!(asset.members.len(), 14);
    assert!(asset.members.iter().all(|member| {
        member
            .transparent_master
            .as_ref()
            .is_some_and(|master| master.path == MASTER_PATH && master.sha256 == MASTER_SHA256)
    }));

    let route = asset
        .character_face_registration_route
        .as_ref()
        .expect("Skeleton T face atlas must declare character-face registration");
    let fixed = route
        .fixed_face_pattern
        .as_ref()
        .expect("Skeleton T face atlas must declare the fixed face texture");
    assert_eq!(route.character_index, 6);
    assert_eq!(fixed.image_file_record, 77);
    assert_eq!(fixed.palette_file_record, 197);
    assert_eq!(fixed.texture_slot, 8);

    let mut records = Vec::new();
    let mut face_conditions = 0;
    let mut face_actions = 0;
    for member in &asset.members {
        let StylizedArtworkConsumer::BattleTexture(consumer) = &member.consumer else {
            panic!("Skeleton T face label must use a battle texture")
        };
        assert_eq!(consumer.load_route.image_file_record, 77);
        assert_eq!(consumer.load_route.palette_file_record, 197);
        assert_eq!(consumer.pattern_texture_slot, 8);
        match &consumer.texture_use {
            StylizedArtworkBattleTextureUse::FaceConditionPatternRecord {
                pattern_record_index,
                creator_actor_symbol,
                ..
            } => {
                face_conditions += 1;
                records.push(*pattern_record_index);
                assert_eq!(consumer.actor_symbol, "PuyoGame_EnemyFace");
                assert_eq!(creator_actor_symbol, "PuyoGame_MakeEnemyFace");
            }
            StylizedArtworkBattleTextureUse::FaceActionPatternRecord {
                pattern_record_index,
                face_state_index,
                face_action_symbol,
                ..
            } => {
                face_actions += 1;
                records.push(*pattern_record_index);
                assert_eq!(consumer.actor_symbol, "Face_MakeParts");
                match *pattern_record_index {
                    3..=5 => {
                        assert_eq!(*face_state_index, 2);
                        assert_eq!(face_action_symbol, "FA_Ske_Head");
                    }
                    7..=10 => {
                        assert_eq!(*face_state_index, 0);
                        assert_eq!(face_action_symbol, "FA_Ske_Tea_NB");
                    }
                    11..=14 => {
                        assert_eq!(*face_state_index, 1);
                        assert_eq!(face_action_symbol, "FA_Ske_Tea_A");
                    }
                    record => panic!("unexpected Skeleton T face record {record}"),
                }
            }
            _ => panic!("Skeleton T face label uses an unexpected texture route"),
        }
    }
    assert_eq!((face_conditions, face_actions), (3, 11));
    assert_eq!(records, [0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14]);

    let clipped = &asset.members[2];
    assert_eq!(clipped.region_id, "tea-cup-label-face-base-2");
    assert_eq!(
        clipped.native_placement,
        PixelBounds {
            x: 322,
            y: 36,
            width: 44,
            height: 74
        }
    );
    assert_eq!(
        clipped.render_clip_bounds,
        Some(PixelBounds {
            x: 346,
            y: 64,
            width: 20,
            height: 46
        })
    );
}
