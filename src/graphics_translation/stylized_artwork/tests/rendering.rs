use super::*;

#[test]
fn palette_ties_fail_for_named_roles_and_choose_the_lowest_source_index() {
    let roles = vec![
        StylizedArtworkPaletteRole {
            role: "left".to_string(),
            palette_index: 1,
            rgba: [0, 0, 0, 255],
        },
        StylizedArtworkPaletteRole {
            role: "right".to_string(),
            palette_index: 2,
            rgba: [2, 0, 0, 255],
        },
    ];
    assert!(nearest_palette_role([1, 0, 0], &roles, true).is_err());
    assert_eq!(
        nearest_palette_role([1, 0, 0], &roles, false)
            .unwrap()
            .palette_index,
        1
    );
}

#[test]
fn render_palette_is_a_unique_verified_source_subset() {
    let source_roles = roles();
    let selected = select_render_palette_roles(&source_roles, &[15, 10]).unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|role| role.palette_index)
            .collect::<Vec<_>>(),
        [10, 15]
    );
    assert!(select_render_palette_roles(&source_roles, &[10]).is_err());
    assert!(select_render_palette_roles(&source_roles, &[10, 10]).is_err());
    assert!(select_render_palette_roles(&source_roles, &[10, 99]).is_err());
}

#[test]
fn native_render_uses_only_declared_opaque_palette_colors() {
    let master = vec![0, 0, 1, 255, 231, 231, 0, 255, 199, 95, 0, 255, 0, 0, 0, 0];
    let (native, usage) = render_native_artwork(
        &master,
        2,
        2,
        PixelBounds {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        },
        2,
        2,
        128,
        &roles(),
        true,
        true,
    )
    .unwrap();
    assert!(
        native
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 0] || roles().iter().any(|role| pixel == role.rgba))
    );
    assert_eq!(
        usage.iter().map(|role| role.native_pixels).sum::<usize>(),
        3
    );
}

#[test]
fn native_render_accepts_one_exact_declared_palette_color() {
    let ink = StylizedArtworkPaletteRole {
        role: "ink".to_string(),
        palette_index: 108,
        rgba: [0, 0, 1, 255],
    };
    let (native, usage) = render_native_artwork(
        &[0, 0, 1, 255],
        1,
        1,
        PixelBounds {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        1,
        1,
        16,
        std::slice::from_ref(&ink),
        true,
        true,
    )
    .unwrap();
    assert_eq!(native, ink.rgba);
    assert_eq!(usage[0].native_pixels, 1);
}

#[test]
fn native_render_rejects_visible_pixels_outside_the_declared_crop() {
    let mut master = vec![0_u8; 3 * 3 * 4];
    master[3] = 255;
    assert!(
        render_native_artwork(
            &master,
            3,
            3,
            PixelBounds {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
            2,
            2,
            128,
            &roles(),
            true,
            true,
        )
        .is_err()
    );
}

#[test]
fn shared_artwork_sheet_crops_one_member_without_requiring_transparent_neighbors() {
    let mut master = [0_u8; 3 * 4].repeat(2);
    master[3] = 255;
    master[7] = 255;
    let (native, _) = render_native_artwork(
        &master,
        3,
        2,
        PixelBounds {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
        },
        1,
        1,
        128,
        &roles(),
        false,
        false,
    )
    .unwrap();
    assert_eq!(native[3], 255);
}

#[test]
fn full_composition_keeps_the_source_card_transparency() {
    let mut native = [9, 8, 7, 255].repeat(2);
    let source_indices = [0, 1];
    let mut palette = vec![0_u8; 256 * 4];
    palette[4..8].copy_from_slice(&[1, 2, 3, 0x80]);
    clear_native_pixels_where_source_is_transparent(
        &mut native,
        PixelBounds {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        },
        &source_indices,
        &palette,
        2,
        1,
    )
    .unwrap();
    assert_eq!(&native[..4], &[0, 0, 0, 0]);
    assert_eq!(&native[4..], &[9, 8, 7, 255]);
}

#[test]
fn enclosing_bounds_cover_disjoint_mode_state_patterns() {
    let states = [
        PixelBounds {
            x: 8,
            y: 136,
            width: 192,
            height: 128,
        },
        PixelBounds {
            x: 208,
            y: 136,
            width: 192,
            height: 128,
        },
        PixelBounds {
            x: 408,
            y: 136,
            width: 192,
            height: 128,
        },
    ];
    assert_eq!(
        enclosing_pixel_bounds(&states).unwrap(),
        PixelBounds {
            x: 8,
            y: 136,
            width: 592,
            height: 128,
        }
    );
    assert!(enclosing_pixel_bounds(&[]).is_err());
}

#[test]
fn repeated_native_artwork_is_clipped_and_cannot_overlap() {
    let native = [12, 34, 56, 255].repeat(4);
    let mut surface = vec![0_u8; 3 * 2 * 4];
    let mut claimed = vec![false; 3 * 2];
    let placement = StylizedArtworkSignedPoint { x: -1, y: 0 };
    let clip = PixelBounds {
        x: 0,
        y: 0,
        width: 3,
        height: 2,
    };
    paste_native_artwork_clipped(
        &mut surface,
        &mut claimed,
        3,
        2,
        &native,
        2,
        2,
        placement,
        clip,
    )
    .unwrap();
    assert_eq!(claimed, [true, false, false, true, false, false]);
    assert!(
        paste_native_artwork_clipped(
            &mut surface,
            &mut claimed,
            3,
            2,
            &native,
            2,
            2,
            placement,
            clip,
        )
        .is_err()
    );
}

#[test]
fn nonoverlapping_stylized_paste_preserves_preclaimed_base_pixels() {
    let mut surface = vec![11, 22, 33, 255, 0, 0, 0, 0];
    let mut claimed = vec![true, false];
    paste_native_artwork_clipped(
        &mut surface,
        &mut claimed,
        2,
        1,
        &[44, 55, 66, 255],
        1,
        1,
        StylizedArtworkSignedPoint { x: 1, y: 0 },
        PixelBounds {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        },
    )
    .unwrap();
    assert_eq!(&surface[..4], &[11, 22, 33, 255]);
    assert_eq!(&surface[4..], &[44, 55, 66, 255]);
    assert_eq!(claimed, [true, true]);
}

#[test]
fn scrolling_grid_uses_pattern_dimensions_for_spacing_and_wrap() {
    let mut consumer = StylizedArtworkScrollingTileConsumer {
        load_routes: Vec::new(),
        registration_routes: Vec::new(),
        pattern_table_symbol: "Pattern".to_string(),
        pattern_record_index: 0,
        pattern_texture_slot: 0,
        pattern_source_bounds: PixelBounds {
            x: 0,
            y: 0,
            width: 10,
            height: 20,
        },
        actor_symbol: "Actor".to_string(),
        actor_instruction_count: 1,
        pattern_setter_symbol: "SetPattern".to_string(),
        position_table_symbol: "Positions".to_string(),
        positions: vec![
            StylizedArtworkScreenPoint { x: -10, y: -20 },
            StylizedArtworkScreenPoint { x: 0, y: -20 },
            StylizedArtworkScreenPoint { x: -10, y: 0 },
            StylizedArtworkScreenPoint { x: 0, y: 0 },
        ],
        horizontal_wrap_distance: 20,
        vertical_wrap_distance: 40,
    };
    validate_scrolling_tile_grid(&consumer).unwrap();
    consumer.vertical_wrap_distance = 39;
    assert!(validate_scrolling_tile_grid(&consumer).is_err());
}

#[test]
fn derives_native_screen_bounds_from_the_same_pattern_transform() {
    assert_eq!(
        derive_screen_bounds(
            StylizedArtworkScreenPoint { x: 88, y: 128 },
            PixelBounds {
                x: 280,
                y: 24,
                width: 208,
                height: 168,
            },
            PixelBounds {
                x: 324,
                y: 90,
                width: 96,
                height: 30,
            },
        )
        .unwrap(),
        PixelBounds {
            x: 132,
            y: 194,
            width: 96,
            height: 30,
        }
    );
}
