use super::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkAsset {
    pub schema: u32,
    pub candidate_id: String,
    pub source_surface_id: String,
    pub region_id: String,
    pub expected_source_text: String,
    pub expected_korean_text: String,
    pub tool_path: String,
    #[serde(default)]
    pub source_background_restoration: GraphicsSourceBackgroundRestoration,
    pub generation_prompt: String,
    pub transparent_master: HashedAsset,
    pub master_crop: PixelBounds,
    pub native_placement: PixelBounds,
    #[serde(default)]
    pub additional_native_placements: Vec<StylizedArtworkSignedPoint>,
    pub alpha_threshold: u8,
    #[serde(default)]
    pub source_palette_selection: StylizedArtworkPaletteSelection,
    pub source_palette_roles: Vec<StylizedArtworkPaletteRole>,
    #[serde(default)]
    pub render_palette_indices: Vec<u8>,
    pub replacement_overlay_path: String,
    #[serde(default)]
    pub demo_registration_route: Option<StylizedArtworkDemoRegistrationRoute>,
    pub consumer: StylizedArtworkConsumer,
    pub review_status: StylizedArtworkReviewStatus,
    pub evidence_boundary: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkSetAsset {
    pub schema: u32,
    pub candidate_id: String,
    pub source_surface_id: String,
    pub tool_path: String,
    #[serde(default = "generated_or_hand_authored_method")]
    pub artwork_method: GraphicsArtworkMethod,
    #[serde(default)]
    pub source_background_restoration: GraphicsSourceBackgroundRestoration,
    #[serde(default)]
    pub master_layout: StylizedArtworkSetMasterLayout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_generation_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_artwork_sheet: Option<HashedAsset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_source_palette_colors_per_member: Option<usize>,
    #[serde(default)]
    pub base_replacement_overlay: Option<HashedAsset>,
    #[serde(default)]
    pub sequential_registration_route: Option<StylizedArtworkSequentialRegistrationRoute>,
    #[serde(default)]
    pub indexed_registration_route: Option<StylizedArtworkIndexedRegistrationRoute>,
    #[serde(default)]
    pub demo_registration_route: Option<StylizedArtworkDemoRegistrationRoute>,
    #[serde(default)]
    pub legacy_demo_registration_route: Option<StylizedArtworkLegacyDemoRegistrationRoute>,
    #[serde(default)]
    pub character_face_registration_route: Option<StylizedArtworkCharacterFaceRegistrationRoute>,
    pub members: Vec<StylizedArtworkSetMember>,
    pub replacement_overlay_path: String,
    pub review_status: StylizedArtworkReviewStatus,
    pub evidence_boundary: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkIndexedRegistrationRoute {
    pub registration: StylizedArtworkRegistrationRoute,
    pub file_id: i16,
    pub load_actor_symbol: String,
    pub load_actor_instruction_count: usize,
    pub load_table_address_upper_offset: usize,
    pub load_table_address_lower_offset: usize,
    pub load_file_id_instruction_offset: usize,
    pub load_call_offset: usize,
    pub registration_actor_symbol: String,
    pub registration_actor_instruction_count: usize,
    pub registration_init_call_offset: usize,
    pub registration_load_table_address_upper_offset: usize,
    pub registration_load_table_address_lower_offset: usize,
    pub registration_set_address_upper_offset: usize,
    pub registration_set_address_lower_offset: usize,
    pub registration_file_id_instruction_offset: usize,
    pub registration_call_offset: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkDemoRegistrationRoute {
    pub load_table_pointer_symbol: String,
    pub demo_index: usize,
    pub selected_load_table_symbol: String,
    pub registration_unit_pointer_symbol: String,
    pub registration_unit_index: usize,
    pub registration_group_pointer_symbol: String,
    pub registration_group_index: usize,
    pub registration_set_symbol: String,
    pub registration_pair_index: usize,
    pub image_load_index: usize,
    pub palette_slot: usize,
    pub texture_slot: usize,
    pub transition_sequence_symbol: String,
    pub transition_opcode: usize,
    pub transition_state: u16,
    pub transition_command_offsets: Vec<usize>,
    pub runner_symbol: String,
    pub runner_instruction_count: usize,
    pub file_id: i16,
    pub registration_initializer_symbol: String,
    pub registration_function_symbol: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkLegacyDemoRegistrationRoute {
    pub load_table_pointer_symbol: String,
    pub pattern_table_pointer_symbol: String,
    pub animation_table_pointer_symbol: String,
    pub movesys_table_pointer_symbol: String,
    pub sequence_table_pointer_symbol: String,
    pub demo_index: usize,
    pub selected_load_table_symbol: String,
    pub selected_pattern_table_symbol: String,
    pub selected_animation_table_symbol: String,
    pub selected_movesys_table_symbol: String,
    pub selected_sequence_symbol: String,
    pub registration_set_symbol: String,
    pub registration_pair_index: usize,
    pub image_load_index: usize,
    pub palette_slot: usize,
    pub texture_slot: usize,
    pub loader_symbol: String,
    pub loader_instruction_count: usize,
    pub runner_symbol: String,
    pub runner_instruction_count: usize,
    pub file_id: i16,
    pub read_function_symbol: String,
    pub registration_initializer_symbol: String,
    pub registration_function_symbol: String,
    pub sequence_creator_symbol: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkCharacterFaceRegistrationRoute {
    pub face_load_table_pointer_symbol: String,
    pub character_index: usize,
    pub character_count: usize,
    pub selected_face_load_table_symbol: String,
    pub frame_load_table_pointer_symbol: String,
    pub frame_load_table_symbols: Vec<String>,
    pub game_load_table_pointer_symbol: String,
    pub game_load_table_symbols: Vec<String>,
    pub registration_set_symbol: String,
    pub registration_pair_index: usize,
    pub image_load_index: usize,
    #[serde(default)]
    pub palette_load_index: usize,
    pub registration_palette_slot: usize,
    pub texture_slot: usize,
    pub random_character_count: i16,
    pub random_character_offset: i16,
    pub file_id: i16,
    pub load_actor_symbol: String,
    pub load_actor_instruction_count: usize,
    pub registration_actor_symbol: String,
    pub registration_actor_instruction_count: usize,
    #[serde(default)]
    pub fixed_face_pattern: Option<StylizedArtworkFixedFacePatternRoute>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkFixedFacePatternRoute {
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub image_flags: u32,
    pub image_destination: u32,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub palette_flags: u32,
    pub palette_destination: u32,
    pub texture_slot: usize,
    pub registration_function_symbol: String,
    pub registration_function_instruction_count: usize,
    pub registration_width_field: i16,
    pub registration_height_field: i16,
    pub pixel_storage_format: i16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkSetMember {
    pub region_id: String,
    pub expected_source_text: String,
    pub expected_korean_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent_master: Option<HashedAsset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_background_removal: Option<StylizedArtworkMasterBackgroundRemoval>,
    pub master_crop: PixelBounds,
    #[serde(default)]
    pub additional_variant_master_crops: Vec<PixelBounds>,
    pub native_placement: PixelBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_clip_bounds: Option<PixelBounds>,
    #[serde(default)]
    pub additional_native_placements: Vec<StylizedArtworkSignedPoint>,
    pub alpha_threshold: u8,
    #[serde(default)]
    pub source_palette_selection: StylizedArtworkPaletteSelection,
    pub source_palette_roles: Vec<StylizedArtworkPaletteRole>,
    pub consumer: StylizedArtworkConsumer,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkMasterBackgroundRemoval {
    pub method: StylizedArtworkMasterBackgroundRemovalMethod,
    pub minimum_channel: u8,
    pub maximum_channel_spread: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StylizedArtworkMasterBackgroundRemovalMethod {
    BorderConnectedLightNeutral,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkSignedPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StylizedArtworkPaletteSelection {
    #[default]
    ExactDeclaredRoles,
    OpaqueSourceMask,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StylizedArtworkSetMasterLayout {
    #[default]
    IsolatedMemberArtwork,
    SharedArtworkSheet,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkPaletteRole {
    pub role: String,
    pub palette_index: u8,
    pub rgba: [u8; 4],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StylizedArtworkConsumer {
    SequencePattern(StylizedArtworkSequenceConsumer),
    AnimatedPattern(StylizedArtworkAnimatedPatternConsumer),
    ScriptedAnimation(StylizedArtworkScriptedAnimationConsumer),
    SourcePattern(StylizedArtworkSourcePatternConsumer),
    UnreferencedSource(StylizedArtworkUnreferencedSourceConsumer),
    DirectPatternDisplay(StylizedArtworkDirectPatternDisplayConsumer),
    IndexedPatternDisplay(StylizedArtworkIndexedPatternDisplayConsumer),
    ScrollingTile(StylizedArtworkScrollingTileConsumer),
    MenuPanel(StylizedArtworkMenuPanelConsumer),
    ModeSelectionStates(StylizedArtworkModeSelectionStatesConsumer),
    TitleBranding(StylizedArtworkTitleBrandingConsumer),
    BattleTexture(StylizedArtworkBattleTextureConsumer),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkSequentialRegistrationRoute {
    pub predecessor_load_table_symbol: String,
    pub pointer_table_symbol: String,
    pub load_table_symbols: Vec<String>,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub texture_slot: usize,
    pub loader_actor_symbol: String,
    pub loader_actor_instruction_count: usize,
    pub registration_actor_symbol: String,
    pub registration_actor_instruction_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkBattleTextureConsumer {
    pub load_route: StylizedArtworkLoadRoute,
    pub pattern_texture_slot: usize,
    pub actor_symbol: String,
    pub actor_instruction_count: usize,
    pub pattern_setter_symbol: String,
    pub pattern_setter_call_offset: usize,
    pub texture_use: StylizedArtworkBattleTextureUse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StylizedArtworkBattleTextureUse {
    PatternRecord {
        pattern_table_symbol: String,
        pattern_address_record_index: usize,
        pattern_record_index: usize,
        pattern_iteration_index: usize,
        pattern_iteration_count: i16,
        pattern_source_bounds: PixelBounds,
        pattern_address_upper_offset: usize,
        pattern_address_lower_offset: usize,
        pattern_argument_offset: usize,
        pattern_stride_instruction_offset: usize,
        pattern_iteration_count_instruction_offset: usize,
    },
    DirectPattern {
        pattern_source_bounds: PixelBounds,
        texture_slot_instruction_offset: usize,
        source_x_instruction_offset: usize,
        source_y_instruction_offset: usize,
        width_instruction_offset: usize,
        height_instruction_offset: usize,
    },
    ActorTextureBox {
        pattern_source_bounds: PixelBounds,
        texture_slot_instruction_offset: usize,
        texture_slot_store_offset: usize,
        source_x_instruction_offset: usize,
        source_y_instruction_offset: usize,
        width_instruction_offset: usize,
        height_instruction_offset: usize,
    },
    CharacterPatternRecord {
        pattern_pointer_table_symbol: String,
        character_index: usize,
        pattern_table_symbol: String,
        pattern_record_index: usize,
        pattern_source_bounds: PixelBounds,
        pattern_pointer_address_upper_offset: usize,
        pattern_pointer_address_lower_offset: usize,
        character_selector_offset: usize,
        character_selector_scale_offset: usize,
        selected_pattern_table_offset: usize,
        pattern_initializer_call_offset: usize,
        pattern_record_index_instruction_offset: usize,
        creator_actor_symbol: String,
        creator_actor_instruction_count: usize,
        creator_function_address_upper_offset: usize,
        creator_function_address_lower_offset: usize,
        creator_call_offset: usize,
        creator_state: StylizedArtworkCreatedActorState,
    },
    CharacterActionPatternRecord {
        pattern_pointer_table_symbol: String,
        character_index: usize,
        pattern_table_symbol: String,
        pattern_record_index: usize,
        pattern_source_bounds: PixelBounds,
        pattern_pointer_address_upper_offset: usize,
        pattern_pointer_address_lower_offset: usize,
        character_selector_offset: usize,
        character_selector_scale_offset: usize,
        selected_pattern_table_offset: usize,
        pattern_initializer_call_offset: usize,
        action_pointer_table_symbol: String,
        action_table_symbol: String,
        action_pattern_indices: Vec<usize>,
        action_pointer_address_upper_offset: usize,
        action_pointer_address_lower_offset: usize,
        action_character_selector_offset: usize,
        action_character_selector_scale_offset: usize,
        selected_action_table_offset: usize,
    },
    FaceConditionPatternRecord {
        pattern_pointer_table_symbol: String,
        character_index: usize,
        pattern_table_symbol: String,
        pattern_record_index: usize,
        pattern_source_bounds: PixelBounds,
        creator_actor_symbol: String,
        creator_actor_instruction_count: usize,
        creator_function_address_upper_offset: usize,
        creator_function_address_lower_offset: usize,
        creator_call_offset: usize,
        pattern_pointer_address_upper_offset: usize,
        pattern_pointer_address_lower_offset: usize,
        character_selector_offset: usize,
        character_selector_scale_offset: usize,
        selected_pattern_table_offset: usize,
        pattern_initializer_call_offset: usize,
        initial_pattern_setter_call_offset: usize,
        actor_pattern_pointer_load_offset: usize,
        actor_pattern_pointer_adjustment_offset: Option<usize>,
        actor_pattern_byte_offset: i16,
    },
    FaceActionPatternRecord {
        pattern_pointer_table_symbol: String,
        character_index: usize,
        pattern_table_symbol: String,
        pattern_record_index: usize,
        pattern_source_bounds: PixelBounds,
        pattern_pointer_address_upper_offset: usize,
        pattern_pointer_address_lower_offset: usize,
        character_selector_offset: usize,
        character_selector_scale_offset: usize,
        selected_pattern_table_offset: usize,
        pattern_initializer_call_offset: usize,
        face_state_pointer_table_symbol: String,
        face_state_index: usize,
        face_state_table_symbol: String,
        face_state_entry_index: usize,
        face_action_symbol: String,
        face_action_pattern_indices: Vec<usize>,
    },
    SourceOnlyRegion {
        source_bounds: PixelBounds,
        pattern_pointer_table_symbol: String,
        character_index: usize,
        character_pattern_table_symbol: String,
        additional_pattern_table_symbols: Vec<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StylizedArtworkCreatedActorState {
    ZeroInitialized,
    Explicit {
        value: i16,
        value_instruction_offset: usize,
        state_store_offset: usize,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkSequenceConsumer {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    #[serde(default)]
    pub pattern_texture_slot: Option<usize>,
    pub pattern_source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub sequence_command_offset: usize,
    pub sequence_opcode: usize,
    pub sequence_command_words: usize,
    pub sequence_actor_index: u16,
    #[serde(default)]
    pub sequence_operation: StylizedArtworkSequenceOperation,
    pub sequence_case_address: u64,
    pub pattern_setter_symbol: String,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: u16,
    pub derived_logical_screen_bounds: PixelBounds,
    #[serde(default)]
    pub movesys_route: Option<StylizedArtworkMovesysRoute>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkAnimatedPatternConsumer {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub create_command_offset: usize,
    pub create_opcode: usize,
    pub create_command_words: usize,
    pub sequence_actor_index: u16,
    pub create_flags: u16,
    pub initial_pattern_record_index: usize,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: u16,
    pub create_case_address: u64,
    pub pattern_setter_symbol: String,
    pub animation_command_offset: usize,
    pub animation_opcode: usize,
    pub animation_command_words: usize,
    pub animation_index: usize,
    pub animation_case_address: u64,
    pub animation_pointer_table_symbol: String,
    pub animation_table_symbol: String,
    pub animation_frame_duration: u16,
    #[serde(default)]
    pub subsequent_animation_frames: Vec<StylizedArtworkAnimationFrame>,
    #[serde(default = "default_animation_terminator")]
    pub animation_terminator: u16,
    pub cell_initializer_symbol: String,
    pub animation_initializer_symbol: String,
    pub derived_logical_screen_bounds: PixelBounds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkScriptedAnimationConsumer {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub create_command_offset: usize,
    pub create_opcode: usize,
    pub create_command_words: usize,
    pub sequence_actor_index: u16,
    pub create_flags: u16,
    pub initial_pattern_record_index: usize,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: u16,
    pub create_case_address: u64,
    pub pattern_setter_symbol: String,
    pub animation_command_offset: usize,
    pub animation_opcode: usize,
    pub animation_command_words: usize,
    pub animation_index: usize,
    pub animation_case_address: u64,
    pub animation_pointer_table_symbol: String,
    pub animation_table_symbol: String,
    pub animation_script_record_index: usize,
    pub animation_frame_duration: u16,
    #[serde(default = "default_animation_terminator")]
    pub animation_terminator: u16,
    #[serde(default)]
    pub animation_terminator_operand: u16,
    pub cell_initializer_symbol: String,
    pub animation_initializer_symbol: String,
    pub derived_logical_screen_bounds: PixelBounds,
}

fn default_animation_terminator() -> u16 {
    u16::MAX
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkAnimationFrame {
    pub pattern_record_index: usize,
    pub pattern_source_bounds: PixelBounds,
    pub frame_duration: u16,
    pub derived_logical_screen_bounds: PixelBounds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkMovesysRoute {
    pub demo_pointer_table_symbol: String,
    pub demo_index: usize,
    pub selected_pointer_table_symbol: String,
    pub script_index: usize,
    pub script_symbol: String,
    pub command_size_symbol: String,
    pub commands: Vec<StylizedArtworkMovesysCommand>,
    pub transform_command_offset: usize,
    pub transform_opcode: usize,
    pub transform_command_words: usize,
    pub transform_anchor: StylizedArtworkScreenPoint,
    pub initialize_command_offset: usize,
    pub initialize_opcode: usize,
    pub initialize_command_words: usize,
    pub sequence_creator_symbol: String,
    pub sequence_creator_instruction_count: usize,
    pub initializer_symbol: String,
    pub initializer_instruction_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkMovesysCommand {
    pub opcode: usize,
    pub operands: Vec<i16>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkSourcePatternConsumer {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub create_pattern_opcode: usize,
    pub update_pattern_opcode: usize,
    pub animation_pointer_table_symbol: String,
    pub animation_table_symbols: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkUnreferencedSourceConsumer {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_texture_slot: usize,
    pub source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub create_pattern_opcode: usize,
    pub update_pattern_opcode: usize,
    pub animation_pointer_table_symbol: String,
    pub animation_table_symbols: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StylizedArtworkSequenceOperation {
    #[default]
    UpdateActor,
    SetPattern {
        create_command_offset: usize,
        create_opcode: usize,
        create_command_words: usize,
        create_flags: u16,
        initial_pattern_record_index: usize,
        create_case_address: u64,
    },
    CreateActor {
        flags: u16,
    },
    CreateChildActor {
        flags: u16,
        parent_actor_index: u16,
        parent_create_command_offset: usize,
        parent_create_opcode: usize,
        parent_create_command_words: usize,
        parent_create_case_address: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkDirectPatternDisplayConsumer {
    pub load_route: StylizedArtworkLoadRoute,
    #[serde(default)]
    pub registration_route: Option<StylizedArtworkRegistrationRoute>,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub actor_instruction_count: usize,
    pub display_constructor_symbol: String,
    pub pattern_address_upper_offset: usize,
    pub pattern_address_lower_offset: usize,
    pub screen_x_instruction_offset: usize,
    pub screen_y_instruction_offset: usize,
    pub depth_instruction_offset: usize,
    pub display_call_offset: usize,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: i16,
    pub derived_logical_screen_bounds: PixelBounds,
    #[serde(default)]
    pub protected_pattern_uses: Vec<StylizedArtworkProtectedPatternUse>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkIndexedPatternDisplayConsumer {
    pub load_route: StylizedArtworkLoadRoute,
    pub registration_route: StylizedArtworkRegistrationRoute,
    pub pattern_table_symbol: String,
    pub base_pattern_record_index: usize,
    pub selector_value: u16,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub actor_instruction_count: usize,
    pub display_constructor_symbol: String,
    pub selector_address: u64,
    pub selector_guard_address_upper_offset: usize,
    pub selector_guard_load_offset: usize,
    pub selector_zero_branch_offset: usize,
    pub selector_one_value_offset: usize,
    pub selector_other_branch_offset: usize,
    pub selector_accepted_target_offset: usize,
    pub selector_other_target_offset: usize,
    pub selector_load_address_upper_offset: usize,
    pub selector_load_offset: usize,
    pub pattern_table_address_upper_offset: usize,
    pub pattern_table_address_lower_offset: usize,
    pub screen_x_instruction_offset: usize,
    pub screen_y_instruction_offset: usize,
    pub depth_instruction_offset: usize,
    pub pattern_index_instruction_offset: usize,
    pub pattern_stride_first_shift_offset: usize,
    pub pattern_stride_add_offset: usize,
    pub pattern_stride_second_shift_offset: usize,
    pub display_call_offset: usize,
    pub pattern_address_add_offset: usize,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: i16,
    pub derived_logical_screen_bounds: PixelBounds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkProtectedPatternUse {
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub pattern_address_upper_offset: usize,
    pub pattern_address_lower_offset: usize,
    pub pattern_setter_symbol: String,
    pub pattern_setter_call_offset: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkScrollingTileConsumer {
    pub load_routes: Vec<StylizedArtworkLoadRoute>,
    pub registration_routes: Vec<StylizedArtworkRegistrationRoute>,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub actor_instruction_count: usize,
    pub pattern_setter_symbol: String,
    pub position_table_symbol: String,
    pub positions: Vec<StylizedArtworkScreenPoint>,
    pub horizontal_wrap_distance: u32,
    pub vertical_wrap_distance: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkMenuPanelConsumer {
    pub load_route: StylizedArtworkLoadRoute,
    pub registration_route: StylizedArtworkRegistrationRoute,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub actor_instruction_count: usize,
    pub pattern_setter_symbol: String,
    pub selector_state_offset: i16,
    pub column_state_offset: i16,
    pub column_index: u16,
    pub screen_first_x: i16,
    pub screen_horizontal_spacing: i16,
    pub screen_y: i16,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub derived_logical_screen_bounds: PixelBounds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkModeSelectionStatesConsumer {
    pub load_route: StylizedArtworkLoadRoute,
    pub pattern_texture_slot: usize,
    pub registration_actor_symbol: String,
    pub registration_actor_instruction_count: usize,
    pub registration_function_symbol: String,
    pub load_table_address_upper_offset: usize,
    pub load_table_address_lower_offset: usize,
    pub registration_call_offset: usize,
    pub pattern_pointer_table_symbol: String,
    pub pattern_pointer_table_index: usize,
    pub pattern_table_symbol: String,
    pub state_pattern_record_indices: Vec<usize>,
    pub state_pattern_source_bounds: Vec<PixelBounds>,
    pub state_source_mask_bounds: Vec<PixelBounds>,
    pub state_artwork_bounds: Vec<PixelBounds>,
    pub blank_pattern_record_index: usize,
    pub blank_pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub actor_instruction_count: usize,
    pub pattern_setter_symbol: String,
    pub mode_index_offset: i16,
    pub animation_table_symbol: String,
    pub animation_pattern_indices: Vec<u16>,
    pub animation_frame_duration: u16,
    pub animation_initializer_symbol: String,
    pub animation_address_upper_offset: usize,
    pub animation_address_lower_offset: usize,
    pub animation_call_offset: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkTitleBrandingConsumer {
    pub load_route: StylizedArtworkLoadRoute,
    pub registration_routes: Vec<StylizedArtworkRegistrationRoute>,
    pub pattern_table_symbol: String,
    pub pattern_texture_slot: usize,
    pub body_pattern: StylizedArtworkPatternSlice,
    pub stored_body_text: String,
    pub assembled_body_text: String,
    pub body_fragment_patterns: Vec<StylizedArtworkTitleFragmentPattern>,
    pub sequence_symbol: String,
    pub sequence_opcode: usize,
    pub sequence_command_words: usize,
    pub sequence_case_address: u64,
    pub pattern_setter_symbol: String,
    pub assembly_commands: Vec<StylizedArtworkTitleAssemblyCommand>,
    pub suffix_pattern: StylizedArtworkPatternSlice,
    pub protected_english_pattern: StylizedArtworkPatternSlice,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkTitleFragmentPattern {
    pub pattern_record_index: usize,
    pub pattern_source_bounds: PixelBounds,
    pub stored_korean_text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkTitleAssemblyCommand {
    pub sequence_command_offset: usize,
    pub sequence_actor_index: u16,
    pub sequence_flags: u16,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: u16,
    pub pattern_record_index: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkPatternSlice {
    pub pattern_record_index: usize,
    pub pattern_source_bounds: PixelBounds,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkLoadRoute {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkRegistrationRoute {
    pub load_table_symbol: String,
    pub registration_set_symbol: String,
    pub texture_slot: usize,
    pub image_load_index: usize,
    pub palette_slot: usize,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct StylizedArtworkLoadRouteRef {
    pub(super) image_file_record: usize,
    pub(super) palette_file_record: usize,
}

impl StylizedArtworkConsumer {
    pub(super) fn adopted_placement_status(&self) -> GraphicsArtworkPlacementStatus {
        if matches!(
            self,
            Self::SourcePattern(_)
                | Self::UnreferencedSource(_)
                | Self::BattleTexture(StylizedArtworkBattleTextureConsumer {
                    texture_use: StylizedArtworkBattleTextureUse::SourceOnlyRegion { .. },
                    ..
                })
        ) {
            GraphicsArtworkPlacementStatus::SourceAligned
        } else {
            GraphicsArtworkPlacementStatus::ConsumerAligned
        }
    }

    pub(super) fn primary_load_route(&self) -> Result<StylizedArtworkLoadRouteRef> {
        match self {
            Self::SequencePattern(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.image_file_record,
                palette_file_record: consumer.palette_file_record,
            }),
            Self::AnimatedPattern(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.image_file_record,
                palette_file_record: consumer.palette_file_record,
            }),
            Self::ScriptedAnimation(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.image_file_record,
                palette_file_record: consumer.palette_file_record,
            }),
            Self::SourcePattern(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.image_file_record,
                palette_file_record: consumer.palette_file_record,
            }),
            Self::UnreferencedSource(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.image_file_record,
                palette_file_record: consumer.palette_file_record,
            }),
            Self::DirectPatternDisplay(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.load_route.image_file_record,
                palette_file_record: consumer.load_route.palette_file_record,
            }),
            Self::IndexedPatternDisplay(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.load_route.image_file_record,
                palette_file_record: consumer.load_route.palette_file_record,
            }),
            Self::ScrollingTile(consumer) => {
                let route = consumer
                    .load_routes
                    .first()
                    .context("scrolling stylized artwork has no load route")?;
                Ok(StylizedArtworkLoadRouteRef {
                    image_file_record: route.image_file_record,
                    palette_file_record: route.palette_file_record,
                })
            }
            Self::MenuPanel(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.load_route.image_file_record,
                palette_file_record: consumer.load_route.palette_file_record,
            }),
            Self::ModeSelectionStates(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.load_route.image_file_record,
                palette_file_record: consumer.load_route.palette_file_record,
            }),
            Self::TitleBranding(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.load_route.image_file_record,
                palette_file_record: consumer.load_route.palette_file_record,
            }),
            Self::BattleTexture(consumer) => Ok(StylizedArtworkLoadRouteRef {
                image_file_record: consumer.load_route.image_file_record,
                palette_file_record: consumer.load_route.palette_file_record,
            }),
        }
    }

    pub(super) fn owns_load_table(&self, symbol: &str) -> bool {
        match self {
            Self::SequencePattern(consumer) => consumer.load_table_symbol == symbol,
            Self::AnimatedPattern(consumer) => consumer.load_table_symbol == symbol,
            Self::ScriptedAnimation(consumer) => consumer.load_table_symbol == symbol,
            Self::SourcePattern(consumer) => consumer.load_table_symbol == symbol,
            Self::UnreferencedSource(consumer) => consumer.load_table_symbol == symbol,
            Self::DirectPatternDisplay(consumer) => consumer.load_route.load_table_symbol == symbol,
            Self::IndexedPatternDisplay(consumer) => {
                consumer.load_route.load_table_symbol == symbol
            }
            Self::ScrollingTile(consumer) => consumer
                .load_routes
                .iter()
                .any(|route| route.load_table_symbol == symbol),
            Self::MenuPanel(consumer) => consumer.load_route.load_table_symbol == symbol,
            Self::ModeSelectionStates(consumer) => consumer.load_route.load_table_symbol == symbol,
            Self::TitleBranding(consumer) => consumer.load_route.load_table_symbol == symbol,
            Self::BattleTexture(consumer) => consumer.load_route.load_table_symbol == symbol,
        }
    }

    pub(super) fn report_evidence_boundary(&self) -> &'static str {
        match self {
            Self::SequencePattern(consumer) => {
                if consumer.movesys_route.is_some() {
                    "The hash-bound ImageGen master is rendered into a deterministic native overlay. Exact source load, grouped demo registration, pattern creation, transform anchor, selected movesys script and command stream, typed sequence routing, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
                } else {
                    "The hash-bound ImageGen master is rendered into a deterministic native overlay. Exact source load, pattern, sequence command, typed opcode dispatch, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
                }
            }
            Self::AnimatedPattern(_) => {
                "The hash-bound ImageGen master is rendered into a deterministic native overlay. Exact legacy demo loading and texture registration, actor creation, animation-table selection, pattern frame, typed sequence dispatch, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
            }
            Self::ScriptedAnimation(_) => {
                "The hash-bound ImageGen master is rendered into a deterministic native overlay. Exact legacy demo loading and texture registration, actor creation, animation-table selection, the selected frame inside a control-bearing animation script, typed sequence dispatch, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
            }
            Self::SourcePattern(_) => {
                "The hash-bound ImageGen master is rendered into a deterministic native overlay at an exact source pattern. The demo texture-group registration, source pattern rectangle, absence from the declared sequence create/update commands and animation tables, palette-index reuse, source-mask clearing, and protected pixels are statically verified, so no visible consumer is claimed. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
            }
            Self::UnreferencedSource(_) => {
                "The hash-bound ImageGen master is rendered into a deterministic native overlay at an exact source-only region. Legacy demo loading and registration, the complete pattern table, and every declared sequence and animation reference are checked; no pattern rectangle selects the source region, so no visible consumer is claimed. Palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
            }
            Self::DirectPatternDisplay(consumer) => match (
                consumer.registration_route.is_some(),
                !consumer.protected_pattern_uses.is_empty(),
            ) {
                (false, false) => {
                    "The hash-bound ImageGen master is rendered into a deterministic native overlay. Exact source load, pattern rectangle, typed direct display construction and screen position, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
                }
                (true, false) => {
                    "The hash-bound ImageGen master is rendered into a deterministic native overlay. Exact source load, texture registration, pattern rectangle, typed direct display construction and screen position, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
                }
                (false, true) => {
                    "The hash-bound ImageGen master is rendered into a deterministic native overlay. Exact source load, pattern rectangle, typed direct display construction and screen position, protected companion pattern use, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
                }
                (true, true) => {
                    "The hash-bound ImageGen master is rendered into a deterministic native overlay. Exact source load, texture registration, pattern rectangle, typed direct display construction and screen position, protected companion pattern use, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
                }
            },
            Self::IndexedPatternDisplay(_) => {
                "The hash-bound ImageGen master is rendered into a deterministic native overlay. Exact source load, texture registration, pattern rectangle, the typed two-value selector guard, selector-to-pattern indexing, 24-byte pattern stride, direct display construction and screen position, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
            }
            Self::ScrollingTile(_) => {
                "The hash-bound ImageGen master is rendered and repeated into a deterministic native scrolling-tile overlay. Exact load and registration routes, pattern source rectangle, position grid, typed actor movement and wrap instructions, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
            }
            Self::MenuPanel(_) => {
                "The hash-bound ImageGen master is rendered into one member of a deterministic menu-panel set. Exact load and registration routes, pattern source rectangle, typed actor pattern selection and screen positioning, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
            }
            Self::ModeSelectionStates(_) => {
                "The hash-bound ImageGen master is independently reduced into the three native mode-selection states. Exact load and registration, mode-to-pattern pointer selection, all three pattern rectangles, the blank fourth frame, animation order, source-footprint bounds, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
            }
            Self::TitleBranding(_) => {
                "The hash-bound ImageGen title repeat-unit master is rendered only inside the exact title-body pattern. Exact load and registration routes, the complete body pattern, both split-animation body fragments, the TitleDemo 43-44-43-44 repeat assembly and typed SeqCommand decoder, the removed Japanese suffix pattern, the protected English badge pattern, palette-index reuse, partitioned source restoration, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
            }
            Self::BattleTexture(consumer) => {
                if matches!(
                    &consumer.texture_use,
                    StylizedArtworkBattleTextureUse::SourceOnlyRegion { .. }
                ) {
                    "The hash-bound ImageGen master is rendered into an exact source-stroke region of the character-face source sheet. The selected character and generic face-pattern tables do not reference this rectangle, so no visible consumer is claimed. Exact source loading, fixed face-texture assembly registration, palette-index reuse, source-mask clearing, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
                } else {
                    "The hash-bound ImageGen master is rendered into one member of a deterministic battle-callout set. Exact source loads, texture registration, pattern source rectangle, typed actor and face-action consumption, palette-index reuse, source-mask clearing, preserved base overlay, and protected pixels are statically verified. The preview is not runtime display evidence; art approval, independent review, runtime observation, and release eligibility remain pending."
                }
            }
        }
    }

    pub(super) fn resized_native_placements(
        &self,
        primary: PixelBounds,
    ) -> Result<Vec<PixelBounds>> {
        let placements = match self {
            Self::ModeSelectionStates(consumer) => consumer.state_artwork_bounds.clone(),
            _ => vec![primary],
        };
        ensure!(
            placements.first().copied() == Some(primary),
            "stylized artwork primary placement differs from its first consumer state"
        );
        Ok(placements)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StylizedArtworkScreenPoint {
    pub x: i16,
    pub y: i16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StylizedArtworkReviewStatus {
    PreHumanReview,
}

pub struct StylizedArtworkDraftInputs<'a> {
    pub graphics: &'a mut GraphicsTranslationBundle,
    pub catalog: &'a StaticRegionCatalog,
    pub previews: &'a TexturePreviewCorpus,
    pub executable: &'a [u8],
    pub image_container: &'a [u8],
    pub asset_path: &'a Path,
    pub asset: &'a StylizedArtworkAsset,
    pub native_preview_path: &'a Path,
    pub surface_preview_path: &'a Path,
    pub refresh_existing: bool,
}

pub struct StylizedArtworkSetDraftInputs<'a> {
    pub graphics: &'a mut GraphicsTranslationBundle,
    pub catalog: &'a StaticRegionCatalog,
    pub previews: &'a TexturePreviewCorpus,
    pub executable: &'a [u8],
    pub image_container: &'a [u8],
    pub asset_path: &'a Path,
    pub asset: &'a StylizedArtworkSetAsset,
    pub native_preview_directory: &'a Path,
    pub surface_preview_path: &'a Path,
    pub refresh_existing: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkDraftReport {
    pub schema: u32,
    pub candidate_id: String,
    pub source_bin_sha256: String,
    pub executable_sha256: String,
    pub source_surface_id: String,
    pub region_id: String,
    pub source_text: String,
    pub korean_text: String,
    pub source_background_restoration: GraphicsSourceBackgroundRestoration,
    pub source_preview: HashedAsset,
    pub transparent_master: HashedAsset,
    pub master_crop: PixelBounds,
    pub native_placement: PixelBounds,
    pub additional_native_placements: Vec<StylizedArtworkSignedPoint>,
    pub native_artwork: HashedAsset,
    pub replacement_overlay: HashedAsset,
    pub surface_preview: HashedAsset,
    pub palette_roles: Vec<StylizedArtworkPaletteRoleUsage>,
    pub demo_registration: Option<StylizedArtworkDemoRegistrationValidation>,
    pub consumer: StylizedArtworkConsumerValidation,
    pub source_japanese_pixels: usize,
    pub korean_overlay_pixels: usize,
    pub changed_palette_indices: usize,
    pub protected_pixels: usize,
    pub changed_protected_indices: usize,
    pub changed_protected_english_indices: usize,
    pub status: &'static str,
    pub release_candidate_eligible: bool,
    pub evidence_boundary: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkSetDraftReport {
    pub schema: u32,
    pub candidate_id: String,
    pub source_bin_sha256: String,
    pub executable_sha256: String,
    pub source_surface_id: String,
    pub artwork_method: GraphicsArtworkMethod,
    pub source_background_restoration: GraphicsSourceBackgroundRestoration,
    pub master_layout: StylizedArtworkSetMasterLayout,
    pub maximum_source_palette_colors_per_member: Option<usize>,
    pub source_preview: HashedAsset,
    pub base_replacement_overlay: Option<HashedAsset>,
    pub base_region_ids: Vec<String>,
    pub sequential_registration: Option<StylizedArtworkSequentialRegistrationValidation>,
    pub indexed_registration: Option<StylizedArtworkIndexedRegistrationValidation>,
    pub demo_registration: Option<StylizedArtworkDemoRegistrationValidation>,
    pub legacy_demo_registration: Option<StylizedArtworkLegacyDemoRegistrationValidation>,
    pub character_face_registration: Option<StylizedArtworkCharacterFaceRegistrationValidation>,
    pub members: Vec<StylizedArtworkSetMemberReport>,
    pub replacement_overlay: HashedAsset,
    pub surface_preview: HashedAsset,
    pub source_japanese_pixels: usize,
    pub korean_overlay_pixels: usize,
    pub changed_palette_indices: usize,
    pub protected_pixels: usize,
    pub changed_protected_indices: usize,
    pub changed_protected_english_indices: usize,
    pub status: &'static str,
    pub release_candidate_eligible: bool,
    pub evidence_boundary: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkSetMemberReport {
    pub region_id: String,
    pub source_text: String,
    pub korean_text: String,
    pub generation_master: HashedAsset,
    pub master_background_removal: Option<StylizedArtworkMasterBackgroundRemoval>,
    pub removed_master_background_pixels: usize,
    pub master_crop: PixelBounds,
    pub additional_variant_master_crops: Vec<PixelBounds>,
    pub native_placement: PixelBounds,
    pub artwork_composition_bounds: Option<PixelBounds>,
    pub render_clip_bounds: Option<PixelBounds>,
    pub additional_native_placements: Vec<StylizedArtworkSignedPoint>,
    pub native_artwork: HashedAsset,
    pub native_variants: Vec<StylizedArtworkNativeVariantReport>,
    pub palette_roles: Vec<StylizedArtworkPaletteRoleUsage>,
    pub consumer: StylizedArtworkConsumerValidation,
}

fn generated_or_hand_authored_method() -> GraphicsArtworkMethod {
    GraphicsArtworkMethod::GeneratedOrHandAuthored
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkNativeVariantReport {
    pub master_crop: PixelBounds,
    pub placement: PixelBounds,
    pub artwork: HashedAsset,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkPaletteRoleUsage {
    pub role: String,
    pub palette_index: u8,
    pub rgba: [u8; 4],
    pub native_pixels: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StylizedArtworkConsumerValidation {
    SequencePattern(StylizedArtworkSequenceConsumerValidation),
    AnimatedPattern(StylizedArtworkAnimatedPatternConsumerValidation),
    ScriptedAnimation(StylizedArtworkScriptedAnimationConsumerValidation),
    SourcePattern(StylizedArtworkSourcePatternConsumerValidation),
    UnreferencedSource(StylizedArtworkUnreferencedSourceConsumerValidation),
    DirectPatternDisplay(StylizedArtworkDirectPatternDisplayConsumerValidation),
    IndexedPatternDisplay(StylizedArtworkIndexedPatternDisplayConsumerValidation),
    ScrollingTile(StylizedArtworkScrollingTileConsumerValidation),
    MenuPanel(StylizedArtworkMenuPanelConsumerValidation),
    ModeSelectionStates(StylizedArtworkModeSelectionStatesConsumerValidation),
    TitleBranding(StylizedArtworkTitleBrandingConsumerValidation),
    BattleTexture(StylizedArtworkBattleTextureConsumerValidation),
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkSequentialRegistrationValidation {
    pub predecessor_load_table_symbol: String,
    pub predecessor_image_count: usize,
    pub pointer_table_symbol: String,
    pub load_table_symbols: Vec<String>,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub texture_slot: usize,
    pub loader_actor_symbol: String,
    pub registration_actor_symbol: String,
    pub typed_loader_instructions: Vec<TypedInstructionEvidence>,
    pub typed_registration_instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkIndexedRegistrationValidation {
    pub registration: StylizedArtworkRegistrationRoute,
    pub file_id: i16,
    pub load_actor_symbol: String,
    pub registration_actor_symbol: String,
    pub typed_load_instructions: Vec<TypedInstructionEvidence>,
    pub typed_registration_instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkDemoRegistrationValidation {
    pub route: StylizedArtworkDemoRegistrationRoute,
    pub selected_registration_pair: StylizedArtworkRegistrationRoute,
    pub typed_transition_decoder_instructions: Vec<TypedInstructionEvidence>,
    pub typed_runner_instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkLegacyDemoRegistrationValidation {
    pub route: StylizedArtworkLegacyDemoRegistrationRoute,
    pub selected_registration_pair: StylizedArtworkRegistrationRoute,
    pub typed_loader_instructions: Vec<TypedInstructionEvidence>,
    pub typed_runner_instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkCharacterFaceRegistrationValidation {
    pub route: StylizedArtworkCharacterFaceRegistrationRoute,
    pub frame_image_count: usize,
    pub game_image_count: usize,
    pub typed_load_instructions: Vec<TypedInstructionEvidence>,
    pub typed_registration_instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkBattleTextureConsumerValidation {
    pub load_route: StylizedArtworkLoadRoute,
    pub pattern_texture_slot: usize,
    pub actor_symbol: String,
    pub pattern_setter_symbol: String,
    pub texture_use: StylizedArtworkBattleTextureUse,
    pub typed_actor_instructions: Vec<TypedInstructionEvidence>,
    pub typed_creator_instructions: Vec<TypedInstructionEvidence>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkSequenceConsumerValidation {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub sequence_command_offset: usize,
    pub sequence_opcode: usize,
    pub sequence_actor_index: u16,
    pub sequence_operation: StylizedArtworkSequenceOperation,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: u16,
    pub derived_logical_screen_bounds: PixelBounds,
    pub derived_native_screen_bounds: PixelBounds,
    pub typed_decoder_instructions: Vec<TypedInstructionEvidence>,
    pub movesys: Option<StylizedArtworkMovesysValidation>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkAnimatedPatternConsumerValidation {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub create_command_offset: usize,
    pub animation_command_offset: usize,
    pub sequence_actor_index: u16,
    pub animation_index: usize,
    pub animation_pointer_table_symbol: String,
    pub animation_table_symbol: String,
    pub animation_frame_duration: u16,
    pub animation_frames: Vec<StylizedArtworkAnimationFrameValidation>,
    pub animation_terminator: u16,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: u16,
    pub derived_logical_screen_bounds: PixelBounds,
    pub derived_native_screen_bounds: PixelBounds,
    pub typed_create_decoder_instructions: Vec<TypedInstructionEvidence>,
    pub typed_animation_decoder_instructions: Vec<TypedInstructionEvidence>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkScriptedAnimationConsumerValidation {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub create_command_offset: usize,
    pub animation_command_offset: usize,
    pub sequence_actor_index: u16,
    pub animation_index: usize,
    pub animation_pointer_table_symbol: String,
    pub animation_table_symbol: String,
    pub animation_script_record_index: usize,
    pub animation_frame_duration: u16,
    pub animation_script_records: usize,
    pub animation_frame_records: usize,
    pub animation_control_records: usize,
    pub animation_terminator: u16,
    pub animation_terminator_operand: u16,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: u16,
    pub derived_logical_screen_bounds: PixelBounds,
    pub derived_native_screen_bounds: PixelBounds,
    pub typed_create_decoder_instructions: Vec<TypedInstructionEvidence>,
    pub typed_animation_decoder_instructions: Vec<TypedInstructionEvidence>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkAnimationFrameValidation {
    pub pattern_record_index: usize,
    pub pattern_source_bounds: PixelBounds,
    pub frame_duration: u16,
    pub derived_logical_screen_bounds: PixelBounds,
    pub derived_native_screen_bounds: PixelBounds,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkMovesysValidation {
    pub route: StylizedArtworkMovesysRoute,
    pub typed_runner_instructions: Vec<TypedInstructionEvidence>,
    pub typed_sequence_creator_instructions: Vec<TypedInstructionEvidence>,
    pub typed_sequence_decoder_instructions: Vec<TypedInstructionEvidence>,
    pub typed_initializer_instructions: Vec<TypedInstructionEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkSourcePatternConsumerValidation {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub animation_pointer_table_symbol: String,
    pub direct_sequence_pattern_references: Vec<usize>,
    pub animation_pattern_references: Vec<usize>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkUnreferencedSourceConsumerValidation {
    pub load_table_symbol: String,
    pub image_load_index: usize,
    pub image_file_record: usize,
    pub palette_slot: usize,
    pub palette_file_record: usize,
    pub pattern_table_symbol: String,
    pub pattern_texture_slot: usize,
    pub source_bounds: PixelBounds,
    pub sequence_symbol: String,
    pub animation_pointer_table_symbol: String,
    pub pattern_record_count: usize,
    pub direct_sequence_pattern_references: Vec<usize>,
    pub animation_pattern_references: Vec<usize>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkDirectPatternDisplayConsumerValidation {
    pub load_route: StylizedArtworkLoadRoute,
    pub registration_route: Option<StylizedArtworkRegistrationRoute>,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub display_constructor_symbol: String,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: i16,
    pub derived_logical_screen_bounds: PixelBounds,
    pub derived_native_screen_bounds: PixelBounds,
    pub protected_pattern_uses: Vec<StylizedArtworkProtectedPatternUse>,
    pub typed_actor_instructions: Vec<TypedInstructionEvidence>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkIndexedPatternDisplayConsumerValidation {
    pub load_route: StylizedArtworkLoadRoute,
    pub registration_route: StylizedArtworkRegistrationRoute,
    pub pattern_table_symbol: String,
    pub base_pattern_record_index: usize,
    pub selector_address: String,
    pub selector_value: u16,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub display_constructor_symbol: String,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub pattern_depth: i16,
    pub derived_logical_screen_bounds: PixelBounds,
    pub derived_native_screen_bounds: PixelBounds,
    pub typed_actor_instructions: Vec<TypedInstructionEvidence>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkScrollingTileConsumerValidation {
    pub load_routes: Vec<StylizedArtworkLoadRoute>,
    pub registration_routes: Vec<StylizedArtworkRegistrationRoute>,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub position_table_symbol: String,
    pub positions: Vec<StylizedArtworkScreenPoint>,
    pub horizontal_wrap_distance: u32,
    pub vertical_wrap_distance: u32,
    pub typed_actor_instructions: Vec<TypedInstructionEvidence>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkMenuPanelConsumerValidation {
    pub load_route: StylizedArtworkLoadRoute,
    pub registration_route: StylizedArtworkRegistrationRoute,
    pub pattern_table_symbol: String,
    pub pattern_record_index: usize,
    pub pattern_texture_slot: usize,
    pub pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub selector_state_offset: i16,
    pub column_state_offset: i16,
    pub column_index: u16,
    pub pattern_screen_origin: StylizedArtworkScreenPoint,
    pub derived_logical_screen_bounds: PixelBounds,
    pub derived_native_screen_bounds: PixelBounds,
    pub typed_actor_instructions: Vec<TypedInstructionEvidence>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkModeSelectionStatesConsumerValidation {
    pub load_route: StylizedArtworkLoadRoute,
    pub pattern_texture_slot: usize,
    pub registration_actor_symbol: String,
    pub registration_function_symbol: String,
    pub pattern_pointer_table_symbol: String,
    pub pattern_pointer_table_index: usize,
    pub pattern_table_symbol: String,
    pub state_pattern_record_indices: Vec<usize>,
    pub state_pattern_source_bounds: Vec<PixelBounds>,
    pub state_source_mask_bounds: Vec<PixelBounds>,
    pub state_artwork_bounds: Vec<PixelBounds>,
    pub blank_pattern_record_index: usize,
    pub blank_pattern_source_bounds: PixelBounds,
    pub actor_symbol: String,
    pub pattern_setter_symbol: String,
    pub mode_index_offset: i16,
    pub animation_table_symbol: String,
    pub animation_pattern_indices: Vec<u16>,
    pub animation_frame_duration: u16,
    pub animation_initializer_symbol: String,
    pub typed_registration_instructions: Vec<TypedInstructionEvidence>,
    pub typed_actor_instructions: Vec<TypedInstructionEvidence>,
    pub runtime_status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StylizedArtworkTitleBrandingConsumerValidation {
    pub load_route: StylizedArtworkLoadRoute,
    pub registration_routes: Vec<StylizedArtworkRegistrationRoute>,
    pub pattern_table_symbol: String,
    pub pattern_texture_slot: usize,
    pub body_pattern: StylizedArtworkPatternSlice,
    pub stored_body_text: String,
    pub assembled_body_text: String,
    pub body_fragment_patterns: Vec<StylizedArtworkTitleFragmentPattern>,
    pub sequence_symbol: String,
    pub sequence_opcode: usize,
    pub sequence_command_words: usize,
    pub sequence_case_address: u64,
    pub pattern_setter_symbol: String,
    pub assembly_commands: Vec<StylizedArtworkTitleAssemblyCommand>,
    pub suffix_pattern: StylizedArtworkPatternSlice,
    pub protected_english_pattern: StylizedArtworkPatternSlice,
    pub typed_decoder_instructions: Vec<TypedInstructionEvidence>,
    pub runtime_status: &'static str,
}
