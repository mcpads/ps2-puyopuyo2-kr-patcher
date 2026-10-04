mod background_restoration;
mod font;
mod index_composition;
mod layout;
mod model;
mod palette;
mod rendering;

#[cfg(test)]
mod tests;

pub(crate) use background_restoration::restore_source_region;
pub(crate) use font::{LoadedStaticUiFont, load_static_ui_font};
pub(crate) use index_composition::{StaticUiIndexCompositionInputs, compose_static_ui_indices};
pub(crate) use layout::{LineLayout, SurfaceDimensions, fit_region_lines};
pub(crate) use model::RegionRestorationInputs;
pub use model::{
    DeferredStaticTextSurface, SkippedStaticUiSurface, StaticTextDraftInputs,
    StaticTextDraftReport, StaticTextDraftScope, StaticUiBackgroundReconstruction,
    StaticUiFontConfiguration, StaticUiLineAlignment, StaticUiLinePlacement,
    StaticUiRasterizationConfiguration, StaticUiRegionDraft, StaticUiSurfaceDraft,
};
pub(crate) use palette::{
    OVERLAY_FILL, OVERLAY_OUTLINE, OVERLAY_TRANSPARENT, select_composition_color_indices,
    select_preview_colors,
};
pub use rendering::render_static_text_drafts;
