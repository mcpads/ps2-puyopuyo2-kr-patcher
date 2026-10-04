use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use anyhow::{Context, Result, ensure};
use object::{Architecture, BinaryFormat, Endianness, Object};
use r5900::{GeneralRegister, InstructionForm};
use serde::{Deserialize, Serialize};

use super::raster::{read_binary_mask, read_rgba_surface, verify_hashed_asset};
use super::static_regions;
use super::static_regions::StaticRegionCatalog;
use super::static_ui::{
    StaticUiIndexCompositionInputs, compose_static_ui_indices, select_composition_color_indices,
};
use super::{
    GraphicsArtworkMethod, GraphicsArtworkPlacementStatus, GraphicsSourceBackgroundRestoration,
    GraphicsTextRegion, GraphicsTranslationBundle, GraphicsTranslationEntry,
    GraphicsTranslationStatus, GraphicsTreatmentProfile, HashedAsset, PixelBounds,
};
use crate::cpx::{read_address_range, read_symbol_bytes, read_u32_at};
use crate::executable::disassemble_symbol;
use crate::indexed_texture::{
    colorize_psmt8, parse_file_records, parse_image_sentinel_texture_registration_pairs,
    parse_load_records, parse_texture_registration_pairs, read_decoded_image, read_palette, sha256,
    unswizzle_psmt8_uploaded_as_psmct32, write_rgba_png,
};
use crate::texture_pattern::{TexturePatternRecord, parse_pattern_records};
use crate::texture_preview_corpus::{TexturePreview, TexturePreviewCorpus};
use crate::typed_evidence::{
    TypedInstructionEvidence, require_branch_two, require_memory, require_shift,
    require_signed_immediate, require_three_general, require_unsigned_immediate, symbol_address,
    validate_jump_target, validate_loaded_address, validate_loaded_memory_address,
};

const SEQUENCE_COMMAND_SYMBOL: &str = "SeqCommand";
const SEQUENCE_COMMAND_SIZE_SYMBOL: &str = "SeqComSize";
const COMMAND_DISPATCH_FIRST_OPCODE: usize = 3;
const COMMAND_DISPATCH_ENTRIES: usize = 53;
const COMMAND_DISPATCH_TABLE_ADDRESS: u64 = 0x004e_fd10;
const SEQUENCE_COMMAND_INSTRUCTIONS: usize = 1_514;

mod background;
mod composition;
mod consumer;
mod model;
mod render;

pub use model::*;
pub use render::{render_stylized_artwork_draft, render_stylized_artwork_set_draft};

#[cfg(test)]
mod tests;
