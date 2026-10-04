use r5900::GeneralRegister;

use super::super::super::catalog::{
    ActorCreationSpec, MessageSpec, TableReferenceSpec, actor, message,
};

pub(super) const START_MENU_TABLE_SYMBOL: &str = "Str_Nazo_Menu";
pub(super) const QUESTION_TABLE_SYMBOL: &str = "NazoQes_Str";
pub(super) const WINDOW_TITLE_TABLE_SYMBOL: &str = "WindowTitle";
pub(super) const EDIT_QUESTION_TABLE_SYMBOL: &str = "NazoEditQes_Str";

pub(super) const START_MENU_MESSAGES: [MessageSpec; 4] = [
    message("nazo/start-menu/start", "Str_Start_Game"),
    message("nazo/start-menu/delete", "Str_Delete_Game"),
    message("nazo/start-menu/copy-from", "Str_Copy_From"),
    message("nazo/start-menu/copy-to", "Str_Copy_To"),
];

pub(super) const QUESTION_MESSAGES: [MessageSpec; 8] = [
    message("nazo/question/01", "NazoQes_Str_1"),
    message("nazo/question/02", "NazoQes_Str_2"),
    message("nazo/question/03", "NazoQes_Str_3"),
    message("nazo/question/04", "NazoQes_Str_4"),
    message("nazo/question/05", "NazoQes_Str_5"),
    message("nazo/question/06", "NazoQes_Str_6"),
    message("nazo/question/07", "NazoQes_Str_7"),
    message("nazo/question/08", "NazoQes_Str_8"),
];

pub(super) const WINDOW_TITLE_MESSAGES: [MessageSpec; 15] = [
    message("nazo/window-title/00", "WindowTitle_0"),
    message("nazo/window-title/01", "WindowTitle_1"),
    message("nazo/window-title/02", "WindowTitle_2"),
    message("nazo/window-title/03", "WindowTitle_3"),
    message("nazo/window-title/04", "WindowTitle_4"),
    message("nazo/window-title/05", "WindowTitle_5"),
    message("nazo/window-title/06", "WindowTitle_6"),
    message("nazo/window-title/07", "WindowTitle_7"),
    message("nazo/window-title/08", "WindowTitle_8"),
    message("nazo/window-title/09", "WindowTitle_9"),
    message("nazo/window-title/10", "WindowTitle_10"),
    message("nazo/window-title/11", "WindowTitle_11"),
    message("nazo/window-title/12", "WindowTitle_12"),
    message("nazo/window-title/13", "WindowTitle_13"),
    message("nazo/window-title/14", "WindowTitle_14"),
];

pub(super) const EDIT_QUESTION_MESSAGES: [MessageSpec; 9] = [
    message("nazo/edit-question/01", "NazoEditQes_Str_1"),
    message("nazo/edit-question/02", "NazoEditQes_Str_2"),
    message("nazo/edit-question/03", "NazoEditQes_Str_3"),
    message("nazo/edit-question/04", "NazoEditQes_Str_4"),
    message("nazo/edit-question/05", "NazoEditQes_Str_5"),
    message("nazo/edit-question/06", "NazoEditQes_Str_6"),
    message("nazo/edit-question/07", "NazoEditQes_Str_7"),
    message("nazo/edit-question/08", "NazoEditQes_Str_8"),
    message("nazo/edit-question/09", "NazoEditQes_Str_9"),
];

pub(super) const ALL_MESSAGE_TABLES: [(&str, &[MessageSpec]); 4] = [
    (START_MENU_TABLE_SYMBOL, &START_MENU_MESSAGES),
    (QUESTION_TABLE_SYMBOL, &QUESTION_MESSAGES),
    (WINDOW_TITLE_TABLE_SYMBOL, &WINDOW_TITLE_MESSAGES),
    (EDIT_QUESTION_TABLE_SYMBOL, &EDIT_QUESTION_MESSAGES),
];

#[derive(Clone, Copy)]
pub(super) struct IndexedMessageReferenceSpec {
    pub(super) producer_symbol: &'static str,
    pub(super) table_indices: &'static [usize],
    pub(super) table: TableReferenceSpec,
}

#[derive(Clone, Copy)]
pub(super) struct FixedTableEntryReferenceSpec {
    pub(super) reference_id: &'static str,
    pub(super) producer_symbol: &'static str,
    pub(super) actor: ActorCreationSpec,
    pub(super) table_symbol: &'static str,
    pub(super) messages: &'static [MessageSpec],
    pub(super) table_index: usize,
    pub(super) upper_offset: usize,
    pub(super) load_offset: usize,
    pub(super) store_offset: usize,
    pub(super) table_base_register: GeneralRegister,
    pub(super) message_register: GeneralRegister,
    pub(super) store_base: GeneralRegister,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DirectCallSite {
    pub(super) caller_address: u64,
    pub(super) call_offset: usize,
}

#[derive(Clone, Copy)]
pub(super) struct StartMenuCallerSpec {
    pub(super) call_offset: usize,
    pub(super) argument_offset: usize,
    pub(super) mode: u8,
    pub(super) zero_move: bool,
}

#[allow(clippy::too_many_arguments)]
const fn indexed_reference(
    reference_id: &'static str,
    producer_symbol: &'static str,
    table_symbol: &'static str,
    messages: &'static [MessageSpec],
    table_indices: &'static [usize],
    actor: ActorCreationSpec,
    upper_offset: usize,
    lower_offset: usize,
    shift_offset: usize,
    add_offset: usize,
    load_offset: usize,
    store_offset: usize,
    table_register: GeneralRegister,
    index_source_register: GeneralRegister,
    index_register: GeneralRegister,
    message_register: GeneralRegister,
    store_base: GeneralRegister,
) -> IndexedMessageReferenceSpec {
    IndexedMessageReferenceSpec {
        producer_symbol,
        table_indices,
        table: TableReferenceSpec {
            reference_id,
            table_symbol,
            messages,
            actor,
            upper_offset,
            lower_offset,
            shift_offset,
            add_offset,
            load_offset,
            store_offset,
            table_register,
            index_source_register,
            index_register,
            message_register,
            store_base,
        },
    }
}

pub(super) const INDEXED_REFERENCES: [IndexedMessageReferenceSpec; 5] = [
    indexed_reference(
        "nazo/start-menu/mode-0-or-1-table-reference",
        "Make_Nazo_StartMenu",
        START_MENU_TABLE_SYMBOL,
        &START_MENU_MESSAGES,
        &[0, 1],
        actor(0x34, 0x40, 0x3c),
        0x44,
        0x4c,
        0x48,
        0x54,
        0x5c,
        0x70,
        GeneralRegister::V1,
        GeneralRegister::S0,
        GeneralRegister::A0,
        GeneralRegister::A0,
        GeneralRegister::V0,
    ),
    indexed_reference(
        "nazo/start-menu/mode-2-table-reference",
        "Make_Nazo_StartMenu",
        START_MENU_TABLE_SYMBOL,
        &START_MENU_MESSAGES,
        &[2],
        actor(0x238, 0x23c, 0x240),
        0x248,
        0x250,
        0x24c,
        0x258,
        0x260,
        0x27c,
        GeneralRegister::V1,
        GeneralRegister::S0,
        GeneralRegister::A0,
        GeneralRegister::A0,
        GeneralRegister::V0,
    ),
    indexed_reference(
        "nazo/window-title/yes-no-table-reference",
        "Make_YesNo_Window",
        WINDOW_TITLE_TABLE_SYMBOL,
        &WINDOW_TITLE_MESSAGES,
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
        actor(0x4a8, 0x4b4, 0x4b0),
        0x4b8,
        0x4c4,
        0x4c0,
        0x4c8,
        0x4d0,
        0x4e4,
        GeneralRegister::V1,
        GeneralRegister::S4,
        GeneralRegister::A2,
        GeneralRegister::V1,
        GeneralRegister::V0,
    ),
    indexed_reference(
        "nazo/edit-question/table-reference",
        "Nazo_Edit_DataDisp",
        EDIT_QUESTION_TABLE_SYMBOL,
        &EDIT_QUESTION_MESSAGES,
        &[0, 1, 2, 3, 4, 5, 6, 7, 8],
        actor(0x58, 0x60, 0x6c),
        0x78,
        0x7c,
        0x84,
        0x88,
        0x8c,
        0x90,
        GeneralRegister::A0,
        GeneralRegister::A1,
        GeneralRegister::A1,
        GeneralRegister::A0,
        GeneralRegister::V0,
    ),
    indexed_reference(
        "nazo/question/info-table-reference",
        "Make_Nazo_InfoAct",
        QUESTION_TABLE_SYMBOL,
        &QUESTION_MESSAGES,
        &[0, 1, 2, 3, 4, 5, 6, 7],
        actor(0x90, 0x9c, 0x98),
        0xa4,
        0xac,
        0xc0,
        0xc4,
        0xc8,
        0xcc,
        GeneralRegister::A1,
        GeneralRegister::T1,
        GeneralRegister::T0,
        GeneralRegister::A1,
        GeneralRegister::V0,
    ),
];

pub(super) const FIXED_REFERENCES: [FixedTableEntryReferenceSpec; 4] = [
    FixedTableEntryReferenceSpec {
        reference_id: "nazo/start-menu/copy-to/fixed-table-reference",
        producer_symbol: "Make_Nazo_StartMenu",
        actor: actor(0x3bc, 0x3c8, 0x3c4),
        table_symbol: START_MENU_TABLE_SYMBOL,
        messages: &START_MENU_MESSAGES,
        table_index: 3,
        upper_offset: 0x3cc,
        load_offset: 0x3d4,
        store_offset: 0x3f8,
        table_base_register: GeneralRegister::AT,
        message_register: GeneralRegister::A0,
        store_base: GeneralRegister::V0,
    },
    FixedTableEntryReferenceSpec {
        reference_id: "nazo/window-title/08/menu-fixed-table-reference",
        producer_symbol: "Nazo_Menu_Run",
        actor: actor(0x1a70, 0x1a7c, 0x1a78),
        table_symbol: WINDOW_TITLE_TABLE_SYMBOL,
        messages: &WINDOW_TITLE_MESSAGES,
        table_index: 8,
        upper_offset: 0x1a80,
        load_offset: 0x1a88,
        store_offset: 0x1aa4,
        table_base_register: GeneralRegister::AT,
        message_register: GeneralRegister::A2,
        store_base: GeneralRegister::V0,
    },
    FixedTableEntryReferenceSpec {
        reference_id: "nazo/window-title/11/menu-fixed-table-reference",
        producer_symbol: "Nazo_Menu_Run",
        actor: actor(0x1e2c, 0x1e38, 0x1e34),
        table_symbol: WINDOW_TITLE_TABLE_SYMBOL,
        messages: &WINDOW_TITLE_MESSAGES,
        table_index: 11,
        upper_offset: 0x1e3c,
        load_offset: 0x1e44,
        store_offset: 0x1e60,
        table_base_register: GeneralRegister::AT,
        message_register: GeneralRegister::V1,
        store_base: GeneralRegister::V0,
    },
    FixedTableEntryReferenceSpec {
        reference_id: "nazo/window-title/12/menu-fixed-table-reference",
        producer_symbol: "Nazo_Menu_Run",
        actor: actor(0x2188, 0x2194, 0x2190),
        table_symbol: WINDOW_TITLE_TABLE_SYMBOL,
        messages: &WINDOW_TITLE_MESSAGES,
        table_index: 12,
        upper_offset: 0x2198,
        load_offset: 0x21a0,
        store_offset: 0x21bc,
        table_base_register: GeneralRegister::AT,
        message_register: GeneralRegister::V1,
        store_base: GeneralRegister::V0,
    },
];

pub(super) const START_MENU_CALLERS: [StartMenuCallerSpec; 3] = [
    StartMenuCallerSpec {
        call_offset: 0x8e8,
        argument_offset: 0x8ec,
        mode: 0,
        zero_move: true,
    },
    StartMenuCallerSpec {
        call_offset: 0xb98,
        argument_offset: 0xb9c,
        mode: 1,
        zero_move: false,
    },
    StartMenuCallerSpec {
        call_offset: 0xf54,
        argument_offset: 0xf4c,
        mode: 2,
        zero_move: false,
    },
];
