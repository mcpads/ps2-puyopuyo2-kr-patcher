use super::model::*;
use super::*;

pub(crate) fn declared_message_corpus_memberships() -> BTreeMap<&'static str, Vec<&'static str>> {
    let mut memberships = BTreeMap::<&'static str, BTreeSet<&'static str>>::new();
    let mut register_consumer = |corpus: &'static str, spec: MessageConsumerSpec| {
        for message in spec
            .direct_references
            .iter()
            .map(|reference| reference.message)
            .chain(
                spec.table_references
                    .iter()
                    .flat_map(|reference| reference.messages.iter().copied()),
            )
        {
            memberships
                .entry(message.symbol)
                .or_default()
                .insert(corpus);
        }
    };
    register_consumer("memory_card_load", LOAD_CONSUMER);
    register_consumer("memory_card_save", SAVE_CONSUMER);
    register_consumer("my_nazo_error", MY_NAZO_ERROR_CONSUMER);
    for message in TUTORIAL_MESSAGES {
        memberships
            .entry(message.symbol)
            .or_default()
            .insert("tutorial");
    }
    for symbol in nazo::declared_message_symbols() {
        memberships.entry(symbol).or_default().insert("nazo");
    }
    memberships
        .into_iter()
        .map(|(symbol, corpora)| (symbol, corpora.into_iter().collect()))
        .collect()
}

#[derive(Clone, Copy)]
pub(super) struct MessageSpec {
    pub(super) id: &'static str,
    pub(super) symbol: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ActorCreationSpec {
    pub(super) upper_offset: usize,
    pub(super) lower_offset: usize,
    pub(super) call_offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RendererActorCreationSite {
    pub(super) producer_symbol: String,
    pub(super) producer_address: u64,
    pub(super) actor: ActorCreationSpec,
}

pub(super) struct TutorialFontTable {
    pub(super) code_slots: Vec<[u8; 2]>,
    pub(super) unique_code_units: BTreeSet<[u8; 2]>,
    pub(super) repeated_code_units: Vec<RepeatedFontCodeUnit>,
}

#[derive(Clone, Copy)]
pub(super) struct DirectReferenceSpec {
    pub(super) reference_id: &'static str,
    pub(super) message: MessageSpec,
    pub(super) actor: ActorCreationSpec,
    pub(super) upper_offset: usize,
    pub(super) lower_offset: usize,
    pub(super) value_register: GeneralRegister,
    pub(super) store_offset: usize,
    pub(super) store_base: GeneralRegister,
}

#[derive(Clone, Copy)]
pub(super) struct TableReferenceSpec {
    pub(super) reference_id: &'static str,
    pub(super) table_symbol: &'static str,
    pub(super) messages: &'static [MessageSpec],
    pub(super) actor: ActorCreationSpec,
    pub(super) upper_offset: usize,
    pub(super) lower_offset: usize,
    pub(super) shift_offset: usize,
    pub(super) add_offset: usize,
    pub(super) load_offset: usize,
    pub(super) store_offset: usize,
    pub(super) table_register: GeneralRegister,
    pub(super) index_source_register: GeneralRegister,
    pub(super) index_register: GeneralRegister,
    pub(super) message_register: GeneralRegister,
    pub(super) store_base: GeneralRegister,
}

#[derive(Clone, Copy)]
pub(super) struct MessageConsumerSpec {
    pub(super) consumer_symbol: &'static str,
    pub(super) direct_references: &'static [DirectReferenceSpec],
    pub(super) table_references: &'static [TableReferenceSpec],
    pub(super) expected_null_stores: usize,
    pub(super) included: &'static str,
    pub(super) excluded: &'static str,
    pub(super) population_status: &'static str,
}

pub(super) const LOAD_RANK_EXEC: MessageSpec =
    message("memory-card/load/rank/execute", "MemLoadMsg_RankLoadExec");
pub(super) const LOAD_TUU_EXEC: MessageSpec =
    message("memory-card/load/tuu/execute", "MemLoadMsg_TuuLoadExec");
pub(super) const LOAD_RULE_EXEC: MessageSpec =
    message("memory-card/load/rule/execute", "MemLoadMsg_RuleLoadExec");
pub(super) const LOAD_NAZO1_EXEC: MessageSpec =
    message("memory-card/load/nazo1/execute", "MemLoadMsg_Nazo1LoadExec");
pub(super) const LOAD_NAZO2_EXEC: MessageSpec =
    message("memory-card/load/nazo2/execute", "MemLoadMsg_Nazo2LoadExec");
pub(super) const LOAD_MY_NAZO_EXEC: MessageSpec = message(
    "memory-card/load/my-nazo/execute",
    "MemLoadMsg_MyNazoLoadExec",
);
pub(super) const LOAD_EXEC_MESSAGES: [MessageSpec; 6] = [
    LOAD_RANK_EXEC,
    LOAD_TUU_EXEC,
    LOAD_RULE_EXEC,
    LOAD_NAZO1_EXEC,
    LOAD_NAZO2_EXEC,
    LOAD_MY_NAZO_EXEC,
];

pub(super) const LOAD_RANK_FAIL: MessageSpec =
    message("memory-card/load/rank/fail", "MemLoadMsg_RankLoadFail");
pub(super) const LOAD_TUU_FAIL: MessageSpec =
    message("memory-card/load/tuu/fail", "MemLoadMsg_TuuLoadFail");
pub(super) const LOAD_RULE_FAIL: MessageSpec =
    message("memory-card/load/rule/fail", "MemLoadMsg_RuleLoadFail");
pub(super) const LOAD_NAZO1_FAIL: MessageSpec =
    message("memory-card/load/nazo1/fail", "MemLoadMsg_Nazo1LoadFail");
pub(super) const LOAD_NAZO2_FAIL: MessageSpec =
    message("memory-card/load/nazo2/fail", "MemLoadMsg_Nazo2LoadFail");
pub(super) const LOAD_MY_NAZO_FAIL: MessageSpec =
    message("memory-card/load/my-nazo/fail", "MemLoadMsg_MyNazoLoadFail");
pub(super) const LOAD_FAIL_MESSAGES: [MessageSpec; 6] = [
    LOAD_RANK_FAIL,
    LOAD_TUU_FAIL,
    LOAD_RULE_FAIL,
    LOAD_NAZO1_FAIL,
    LOAD_NAZO2_FAIL,
    LOAD_MY_NAZO_FAIL,
];

pub(super) const LOAD_RANK_CRUSH: MessageSpec =
    message("memory-card/load/rank/corrupt", "MemLoadMsg_RankLoadCrush");
pub(super) const LOAD_TUU_CRUSH: MessageSpec =
    message("memory-card/load/tuu/corrupt", "MemLoadMsg_TuuLoadCrush");
pub(super) const LOAD_RULE_CRUSH: MessageSpec =
    message("memory-card/load/rule/corrupt", "MemLoadMsg_RuleLoadCrush");
pub(super) const LOAD_NAZO1_CRUSH: MessageSpec = message(
    "memory-card/load/nazo1/corrupt",
    "MemLoadMsg_Nazo1LoadCrush",
);
pub(super) const LOAD_NAZO2_CRUSH: MessageSpec = message(
    "memory-card/load/nazo2/corrupt",
    "MemLoadMsg_Nazo2LoadCrush",
);
pub(super) const LOAD_MY_NAZO_CRUSH: MessageSpec = message(
    "memory-card/load/my-nazo/corrupt",
    "MemLoadMsg_MyNazoLoadCrush",
);
pub(super) const LOAD_CRUSH_MESSAGES: [MessageSpec; 6] = [
    LOAD_RANK_CRUSH,
    LOAD_TUU_CRUSH,
    LOAD_RULE_CRUSH,
    LOAD_NAZO1_CRUSH,
    LOAD_NAZO2_CRUSH,
    LOAD_MY_NAZO_CRUSH,
];

pub(super) const LOAD_DIRECT_REFERENCES: [DirectReferenceSpec; 12] = [
    direct_reference(
        "memory-card/load/check/reference",
        message("memory-card/load/check", "MemLoadMsg_Check"),
        actor(0x1b4, 0x1c0, 0x1bc),
        0x1c8,
        0x1d0,
        GeneralRegister::A3,
        0x1e0,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/load/corrupt-file/reference",
        message("memory-card/load/corrupt-file", "MemLoadMsg_CrushFile"),
        actor(0x414, 0x420, 0x41c),
        0x428,
        0x430,
        GeneralRegister::A3,
        0x440,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/load/no-card/reference",
        message("memory-card/load/no-card", "MemLoadMsg_NoCard"),
        actor(0x5e4, 0x5f0, 0x5ec),
        0x5f8,
        0x600,
        GeneralRegister::A3,
        0x610,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/load/unformatted/reference",
        message("memory-card/load/unformatted", "MemLoadMsg_NoFormat"),
        actor(0x694, 0x6a0, 0x69c),
        0x6a8,
        0x6b0,
        GeneralRegister::A3,
        0x6c0,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/load/no-data/reference",
        message("memory-card/load/no-data", "MemLoadMsg_NoData"),
        actor(0x73c, 0x748, 0x744),
        0x750,
        0x758,
        GeneralRegister::A3,
        0x768,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/load/no-space/primary-reference",
        message("memory-card/load/no-space", "MemSaveMsg_NoArea"),
        actor(0x7e4, 0x7f0, 0x7ec),
        0x7f8,
        0x800,
        GeneralRegister::A3,
        0x810,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/load/no-space/secondary-reference",
        message("memory-card/load/no-space", "MemSaveMsg_NoArea"),
        actor(0x87c, 0x888, 0x884),
        0x890,
        0x898,
        GeneralRegister::A3,
        0x8a8,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/load/all/execute-reference",
        message("memory-card/load/all/execute", "MemLoadMsg_AllLoadExec"),
        actor(0xabc, 0xac8, 0xac4),
        0xad0,
        0xad8,
        GeneralRegister::A3,
        0xae8,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/load/success/reference",
        message("memory-card/load/success", "MemLoadMsg_LoadSuccess"),
        actor(0xbfc, 0xc08, 0xc04),
        0xc10,
        0xc18,
        GeneralRegister::A3,
        0xc28,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/load/copy-start-confirm/reference",
        message(
            "memory-card/load/copy-start-confirm",
            "Mem_YesNoCopyStartMsg",
        ),
        actor(0xdd4, 0xde0, 0xddc),
        0xde8,
        0xdf0,
        GeneralRegister::A2,
        0xe00,
        GeneralRegister::V0,
    ),
    direct_reference(
        "memory-card/load/yes-no/reference",
        message("memory-card/load/yes-no", "Mem_YesNoMsg"),
        actor(0xe4c, 0xe58, 0xe54),
        0xe60,
        0xe68,
        GeneralRegister::A2,
        0xe78,
        GeneralRegister::V0,
    ),
    direct_reference(
        "memory-card/load/button-ok/reference",
        message("memory-card/load/button-ok", "MemLoadMsg_BtnOK"),
        actor(0x105c, 0x1068, 0x1064),
        0x1070,
        0x1078,
        GeneralRegister::A3,
        0x1088,
        GeneralRegister::V1,
    ),
];

pub(super) const LOAD_TABLE_REFERENCES: [TableReferenceSpec; 3] = [
    table_reference(
        "memory-card/load/result/corrupt-table-reference",
        "MemLoadMsg_Crush",
        &LOAD_CRUSH_MESSAGES,
        actor(0x924, 0x930, 0x92c),
        0x8ec,
        0x8f4,
        0x8f8,
        0x8fc,
        0x904,
        0x948,
    ),
    table_reference(
        "memory-card/load/result/fail-table-reference",
        "MemLoadMsg_Fail",
        &LOAD_FAIL_MESSAGES,
        actor(0x9ec, 0x9f8, 0x9f4),
        0x9b4,
        0x9bc,
        0x9c0,
        0x9c4,
        0x9cc,
        0xa10,
    ),
    table_reference(
        "memory-card/load/result/execute-table-reference",
        "MemLoadMsg_Exec",
        &LOAD_EXEC_MESSAGES,
        actor(0xb54, 0xb60, 0xb5c),
        0xb1c,
        0xb24,
        0xb20,
        0xb28,
        0xb34,
        0xb78,
    ),
];

pub(super) const SAVE_EXEC_MESSAGES: [MessageSpec; 6] = [
    message("memory-card/save/rank/execute", "MemSaveMsg_RankSaveExec"),
    message("memory-card/save/tuu/execute", "MemSaveMsg_TuuSaveExec"),
    message("memory-card/save/rule/execute", "MemSaveMsg_RuleSaveExec"),
    message("memory-card/save/nazo1/execute", "MemSaveMsg_Nazo1SaveExec"),
    message("memory-card/save/nazo2/execute", "MemSaveMsg_Nazo2SaveExec"),
    message(
        "memory-card/save/my-nazo/execute",
        "MemSaveMsg_MyNazoSaveExec",
    ),
];

pub(super) const SAVE_FAIL_MESSAGES: [MessageSpec; 6] = [
    message("memory-card/save/rank/fail", "MemSaveMsg_RankSaveFail"),
    message("memory-card/save/tuu/fail", "MemSaveMsg_TuuSaveFail"),
    message("memory-card/save/rule/fail", "MemSaveMsg_RuleSaveFail"),
    message("memory-card/save/nazo1/fail", "MemSaveMsg_Nazo1SaveFail"),
    message("memory-card/save/nazo2/fail", "MemSaveMsg_Nazo2SaveFail"),
    message("memory-card/save/my-nazo/fail", "MemSaveMsg_MyNazoSaveFail"),
];

pub(super) const SAVE_END_MESSAGES: [MessageSpec; 6] = [
    message("memory-card/save/rank/end", "MemSaveMsg_RankSaveEnd"),
    message("memory-card/save/tuu/end", "MemSaveMsg_TuuSaveEnd"),
    message("memory-card/save/rule/end", "MemSaveMsg_RuleSaveEnd"),
    message("memory-card/save/nazo1/end", "MemSaveMsg_Nazo1SaveEnd"),
    message("memory-card/save/nazo2/end", "MemSaveMsg_Nazo2SaveEnd"),
    message("memory-card/save/my-nazo/end", "MemSaveMsg_MyNazoSaveEnd"),
];

pub(super) const OVERWRITE_MESSAGES: [MessageSpec; 7] = [
    message("memory-card/save/overwrite/rank", "OverWrite_RankMsg"),
    message("memory-card/save/overwrite/tuu", "OverWrite_TuuMsg"),
    message("memory-card/save/overwrite/rule", "OverWrite_RuleMsg"),
    message("memory-card/save/overwrite/nazo1", "OverWrite_Nazo1Msg"),
    message("memory-card/save/overwrite/nazo2", "OverWrite_Nazo2Msg"),
    message("memory-card/save/overwrite/my-nazo", "OverWrite_MyNazoMsg"),
    message("memory-card/save/overwrite/my-nazo", "OverWrite_MyNazoMsg"),
];

pub(super) const OVERWRITE_CANCEL_MESSAGES: [MessageSpec; 7] = [
    message(
        "memory-card/save/overwrite/rank/cancel",
        "OverWrite_RankCancelMsg",
    ),
    message(
        "memory-card/save/overwrite/tuu/cancel",
        "OverWrite_TuuCancelMsg",
    ),
    message(
        "memory-card/save/overwrite/rule/cancel",
        "OverWrite_RuleCancelMsg",
    ),
    message(
        "memory-card/save/overwrite/nazo1/cancel",
        "OverWrite_Nazo1CancelMsg",
    ),
    message(
        "memory-card/save/overwrite/nazo2/cancel",
        "OverWrite_Nazo2CancelMsg",
    ),
    message(
        "memory-card/save/overwrite/my-nazo/cancel",
        "OverWrite_MyNazoCancelMsg",
    ),
    message(
        "memory-card/save/overwrite/my-nazo/cancel",
        "OverWrite_MyNazoCancelMsg",
    ),
];

pub(super) const SAVE_DIRECT_REFERENCES: [DirectReferenceSpec; 27] = [
    direct_reference_before_store(
        "memory-card/save/check/reference",
        message("memory-card/load/check", "MemLoadMsg_Check"),
        0x250,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/corrupt-file/reference",
        message("memory-card/load/corrupt-file", "MemLoadMsg_CrushFile"),
        0x500,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/new-select/reference",
        message("memory-card/save/new-select", "MemSaveMsg_NewSel"),
        0x5a0,
        GeneralRegister::A2,
        GeneralRegister::V0,
    ),
    direct_reference_before_store(
        "memory-card/save/no-card/primary-reference",
        message("memory-card/load/no-card", "MemLoadMsg_NoCard"),
        0x6e0,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/new-cancel/reference",
        message("memory-card/save/new-cancel", "MemSaveMsg_NewCancel"),
        0x840,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/overwrite-select/reference",
        message("memory-card/save/overwrite-select", "OverWriteSelMsg"),
        0x8c0,
        GeneralRegister::A2,
        GeneralRegister::V0,
    ),
    direct_reference_before_store(
        "memory-card/save/no-card/secondary-reference",
        message("memory-card/load/no-card", "MemLoadMsg_NoCard"),
        0xa00,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/fail/primary-reference",
        message("memory-card/save/fail", "MemSaveMsg_SaveFail"),
        0xbc8,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/no-card/tertiary-reference",
        message("memory-card/load/no-card", "MemLoadMsg_NoCard"),
        0xd28,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/format-select/reference",
        message("memory-card/save/format-select", "MemSaveMsg_FormatSel"),
        0xdb8,
        GeneralRegister::A2,
        GeneralRegister::V0,
    ),
    direct_reference_before_store(
        "memory-card/save/unformatted/reference",
        message(
            "memory-card/save/unformatted",
            "MemSaveMsg_SaveNoCardFormat",
        ),
        0xef8,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/format-execute/reference",
        message("memory-card/save/format-execute", "MemSaveMsg_FormatExec"),
        0x1048,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/format-cancel/reference",
        message("memory-card/save/format-cancel", "MemSaveMsg_FormatCancel"),
        0x10d8,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/format-success/reference",
        message("memory-card/save/format-success", "MemSaveMsg_FormatOk"),
        0x11a8,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/format-fail/reference",
        message("memory-card/save/format-fail", "MemSaveMsg_FormatFail"),
        0x1238,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/execute/reference",
        message("memory-card/save/execute", "MemSaveMsg_SaveExec"),
        0x12d8,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/no-space/reference",
        message("memory-card/load/no-space", "MemSaveMsg_NoArea"),
        0x1370,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/fail/secondary-reference",
        message("memory-card/save/fail", "MemSaveMsg_SaveFail"),
        0x1468,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/all/execute/primary-reference",
        message("memory-card/save/all/execute", "MemSaveMsg_AllSaveExec"),
        0x15a8,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/fail/tertiary-reference",
        message("memory-card/save/fail", "MemSaveMsg_SaveFail"),
        0x1638,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/success/primary-reference",
        message("memory-card/save/success", "MemSaveMsg_SaveSuccess"),
        0x16e8,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/fail/quaternary-reference",
        message("memory-card/save/fail", "MemSaveMsg_SaveFail"),
        0x1810,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/no-card/quaternary-reference",
        message("memory-card/load/no-card", "MemLoadMsg_NoCard"),
        0x1a00,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/all/execute/secondary-reference",
        message("memory-card/save/all/execute", "MemSaveMsg_AllSaveExec"),
        0x1c08,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/success/secondary-reference",
        message("memory-card/save/success", "MemSaveMsg_SaveSuccess"),
        0x1d48,
        GeneralRegister::A3,
        GeneralRegister::V1,
    ),
    direct_reference_before_store(
        "memory-card/save/cancel/reference",
        message("memory-card/save/cancel", "MemSaveCancel"),
        0x2020,
        GeneralRegister::A2,
        GeneralRegister::V0,
    ),
    direct_reference_before_store(
        "memory-card/save/copy-confirm/reference",
        message("memory-card/save/copy-confirm", "Mem_CopyYesNoMsg"),
        0x2098,
        GeneralRegister::A2,
        GeneralRegister::V0,
    ),
];

pub(super) const SAVE_TABLE_REFERENCES: [TableReferenceSpec; 5] = [
    table_reference_with_registers(
        "memory-card/save/overwrite/table-reference",
        "OverWrite_Msg",
        &OVERWRITE_MESSAGES,
        actor(0x1894, 0x18a0, 0x189c),
        0x185c,
        0x1860,
        0x1868,
        0x186c,
        0x1874,
        0x18b8,
        GeneralRegister::V0,
        GeneralRegister::V1,
        GeneralRegister::S1,
        GeneralRegister::V0,
    ),
    table_reference_with_registers(
        "memory-card/save/overwrite/cancel-table-reference",
        "OverWrite_CancelMsg",
        &OVERWRITE_CANCEL_MESSAGES,
        actor(0x1b3c, 0x1b48, 0x1b44),
        0x1b04,
        0x1b0c,
        0x1b10,
        0x1b14,
        0x1b1c,
        0x1b60,
        GeneralRegister::V1,
        GeneralRegister::A0,
        GeneralRegister::S1,
        GeneralRegister::V1,
    ),
    table_reference_with_registers(
        "memory-card/save/result/execute-table-reference",
        "MemSaveMsg_Exec",
        &SAVE_EXEC_MESSAGES,
        actor(0x1c74, 0x1c80, 0x1c7c),
        0x1c3c,
        0x1c44,
        0x1c40,
        0x1c48,
        0x1c54,
        0x1c98,
        GeneralRegister::V1,
        GeneralRegister::A0,
        GeneralRegister::S1,
        GeneralRegister::V1,
    ),
    table_reference_with_registers(
        "memory-card/save/result/end-table-reference",
        "MemSaveMsg_End",
        &SAVE_END_MESSAGES,
        actor(0x1e5c, 0x1e68, 0x1e64),
        0x1e24,
        0x1e2c,
        0x1e30,
        0x1e34,
        0x1e3c,
        0x1e80,
        GeneralRegister::V1,
        GeneralRegister::A0,
        GeneralRegister::S1,
        GeneralRegister::V1,
    ),
    table_reference_with_registers(
        "memory-card/save/result/fail-table-reference",
        "MemSaveMsg_Fail",
        &SAVE_FAIL_MESSAGES,
        actor(0x1efc, 0x1f08, 0x1f04),
        0x1ebc,
        0x1ec0,
        0x1ec4,
        0x1ec8,
        0x1ed0,
        0x1f20,
        GeneralRegister::V0,
        GeneralRegister::V1,
        GeneralRegister::S1,
        GeneralRegister::V1,
    ),
];

pub(super) const SAVE_CONSUMER: MessageConsumerSpec = MessageConsumerSpec {
    consumer_symbol: "MemSave_Act",
    direct_references: &SAVE_DIRECT_REFERENCES,
    table_references: &SAVE_TABLE_REFERENCES,
    expected_null_stores: 5,
    included: "all non-null message pointers stored to child +0x108 by MemSave_Act",
    excluded: "MemLoad_Act and every other text consumer in the executable",
    population_status: "complete for the declared MemSave_Act consumer population",
};

pub(super) const MY_NAZO_ERROR_DIRECT_REFERENCES: [DirectReferenceSpec; 2] = [
    direct_reference(
        "memory-card/my-nazo/error/reference",
        message("memory-card/my-nazo/error", "Mem_MyNazoErrMsg"),
        actor(0x54, 0x60, 0x5c),
        0x68,
        0x70,
        GeneralRegister::A3,
        0x80,
        GeneralRegister::V1,
    ),
    direct_reference(
        "memory-card/my-nazo/button-ok/reference",
        message("memory-card/load/button-ok", "MemLoadMsg_BtnOK"),
        actor(0xc4, 0xd0, 0xcc),
        0xd8,
        0xe0,
        GeneralRegister::A3,
        0xf0,
        GeneralRegister::V1,
    ),
];

pub(super) const MY_NAZO_ERROR_CONSUMER: MessageConsumerSpec = MessageConsumerSpec {
    consumer_symbol: "MyNazo_ErrMsg",
    direct_references: &MY_NAZO_ERROR_DIRECT_REFERENCES,
    table_references: &[],
    expected_null_stores: 0,
    included: "all non-null message pointers stored to child +0x108 by MyNazo_ErrMsg",
    excluded: "every other text consumer in the executable",
    population_status: "complete for the declared MyNazo_ErrMsg consumer population",
};

pub(super) const TUTORIAL_MESSAGES: [MessageSpec; 12] = [
    message("tutorial/message/00", "ExpMsg_Data_0"),
    message("tutorial/message/01", "ExpMsg_Data_1"),
    message("tutorial/message/02", "ExpMsg_Data_2"),
    message("tutorial/message/03", "ExpMsg_Data_3"),
    message("tutorial/message/04", "ExpMsg_Data_4"),
    message("tutorial/message/05", "ExpMsg_Data_5"),
    message("tutorial/message/06", "ExpMsg_Data_6"),
    message("tutorial/message/07", "ExpMsg_Data_7"),
    message("tutorial/message/08", "ExpMsg_Data_8"),
    message("tutorial/message/09", "ExpMsg_Data_9"),
    message("tutorial/message/10", "ExpMsg_Data_10"),
    message("tutorial/message/11", "ExpMsg_Data_11"),
];

pub(super) const TUTORIAL_TABLE_REFERENCE: TableReferenceSpec = table_reference_with_registers(
    TUTORIAL_REFERENCE_ID,
    TUTORIAL_MESSAGE_TABLE_SYMBOL,
    &TUTORIAL_MESSAGES,
    actor(0x1648, 0x164c, 0x1650),
    0x165c,
    0x1664,
    0x1688,
    0x168c,
    0x1690,
    0x1694,
    GeneralRegister::A1,
    GeneralRegister::A2,
    GeneralRegister::A1,
    GeneralRegister::V0,
);

pub(super) const LOAD_CONSUMER: MessageConsumerSpec = MessageConsumerSpec {
    consumer_symbol: "MemLoad_Act",
    direct_references: &LOAD_DIRECT_REFERENCES,
    table_references: &LOAD_TABLE_REFERENCES,
    expected_null_stores: 1,
    included: "all non-null message pointers stored to child +0x108 by MemLoad_Act",
    excluded: "MemSave_Act and every other text consumer in the executable",
    population_status: "complete for the declared MemLoad_Act consumer population",
};

pub(super) const fn message(id: &'static str, symbol: &'static str) -> MessageSpec {
    MessageSpec { id, symbol }
}

pub(super) const fn actor(
    upper_offset: usize,
    lower_offset: usize,
    call_offset: usize,
) -> ActorCreationSpec {
    ActorCreationSpec {
        upper_offset,
        lower_offset,
        call_offset,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) const fn direct_reference(
    reference_id: &'static str,
    message: MessageSpec,
    actor: ActorCreationSpec,
    upper_offset: usize,
    lower_offset: usize,
    value_register: GeneralRegister,
    store_offset: usize,
    store_base: GeneralRegister,
) -> DirectReferenceSpec {
    DirectReferenceSpec {
        reference_id,
        message,
        actor,
        upper_offset,
        lower_offset,
        value_register,
        store_offset,
        store_base,
    }
}

pub(super) const fn direct_reference_before_store(
    reference_id: &'static str,
    message: MessageSpec,
    store_offset: usize,
    value_register: GeneralRegister,
    store_base: GeneralRegister,
) -> DirectReferenceSpec {
    direct_reference(
        reference_id,
        message,
        actor(
            store_offset - 0x2c,
            store_offset - 0x20,
            store_offset - 0x24,
        ),
        store_offset - 0x18,
        store_offset - 0x10,
        value_register,
        store_offset,
        store_base,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) const fn table_reference(
    reference_id: &'static str,
    table_symbol: &'static str,
    messages: &'static [MessageSpec],
    actor: ActorCreationSpec,
    upper_offset: usize,
    lower_offset: usize,
    shift_offset: usize,
    add_offset: usize,
    load_offset: usize,
    store_offset: usize,
) -> TableReferenceSpec {
    TableReferenceSpec {
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
        table_register: GeneralRegister::V1,
        index_source_register: GeneralRegister::A0,
        index_register: GeneralRegister::A0,
        message_register: GeneralRegister::S0,
        store_base: GeneralRegister::V1,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) const fn table_reference_with_registers(
    reference_id: &'static str,
    table_symbol: &'static str,
    messages: &'static [MessageSpec],
    actor: ActorCreationSpec,
    upper_offset: usize,
    lower_offset: usize,
    shift_offset: usize,
    add_offset: usize,
    load_offset: usize,
    store_offset: usize,
    table_register: GeneralRegister,
    index_register: GeneralRegister,
    message_register: GeneralRegister,
    store_base: GeneralRegister,
) -> TableReferenceSpec {
    TableReferenceSpec {
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
        index_source_register: index_register,
        index_register,
        message_register,
        store_base,
    }
}
