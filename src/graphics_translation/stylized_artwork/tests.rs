use super::background::clear_border_connected_light_neutral;
use super::composition::{
    clear_native_pixels_where_source_is_transparent, derive_screen_bounds, nearest_palette_role,
    paste_native_artwork_clipped, render_native_artwork, select_render_palette_roles,
};
use super::consumer::validate_scrolling_tile_grid;
use super::render::{enclosing_pixel_bounds, validate_set_asset_header};
use super::*;

fn roles() -> Vec<StylizedArtworkPaletteRole> {
    vec![
        StylizedArtworkPaletteRole {
            role: "dark".to_string(),
            palette_index: 10,
            rgba: [0, 0, 1, 255],
        },
        StylizedArtworkPaletteRole {
            role: "yellow".to_string(),
            palette_index: 11,
            rgba: [231, 231, 0, 255],
        },
        StylizedArtworkPaletteRole {
            role: "orange".to_string(),
            palette_index: 15,
            rgba: [199, 95, 0, 255],
        },
    ]
}

fn point_is_inside(bounds: PixelBounds, x: u32, y: u32) -> bool {
    x >= bounds.x && x < bounds.x + bounds.width && y >= bounds.y && y < bounds.y + bounds.height
}

mod background;
mod nomi_search_assets;
mod rendering;
mod schezo_wordplay_assets;
mod scripted_assets;
mod skeleton_tea_assets;
mod story_tea_assets;
mod tracked_assets;
mod wolf_man_assets;
