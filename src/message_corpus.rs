use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use object::{
    Architecture, BinaryFormat, Endianness, Object, ObjectSection, ObjectSymbol, SymbolKind,
};
use r5900::{GeneralRegister, InstructionForm, Operands};
use serde::Serialize;

use crate::cpx::{read_symbol_bytes, read_u32_at};
use crate::executable::{disassemble_symbol, read_executable_symbol, validate_r5900_instruction};
use crate::indexed_texture::sha256;

mod catalog;
mod instruction;
mod model;
mod nazo;
mod parsing;
mod survey;
mod text;

pub use model::*;
pub use nazo::{NazoMessageCorpusReport, survey_nazo_message_producers, survey_nazo_messages};
pub use survey::{
    survey_memory_card_messages, survey_memory_card_save_messages, survey_my_nazo_error_messages,
    survey_tutorial_messages,
};
use text::{decode_hex, display_message, hex, parse_message, reconstruct_message};

const RENDERER_SYMBOL: &str = "MsgPrint_Act";
const ACTOR_SETTER_SYMBOL: &str = "Act_SetBottom";
const MESSAGE_FIELD_OFFSET: i16 = 0x108;
const TUTORIAL_PRODUCER_SYMBOL: &str = "SeqCommand";
const TUTORIAL_RENDERER_SYMBOL: &str = "ExpMsgPrint_Act";
const TUTORIAL_LOOKUP_SYMBOL: &str = "GetFontParam_Exp";
const TUTORIAL_FONT_TABLE_SYMBOL: &str = "FontList_Exp";
const TUTORIAL_MESSAGE_TABLE_SYMBOL: &str = "ExpMsg_Data";
const TUTORIAL_REFERENCE_ID: &str = "tutorial/messages/indexed-table-reference";
const TUTORIAL_COMMAND_SIZE_SYMBOL: &str = "SeqComSize";
const TUTORIAL_SEQUENCE_SYMBOLS: [&str; 2] = ["ExpDemo_0", "ExpDemo_1"];
const TUTORIAL_MESSAGE_COMMAND_OPCODE: usize = 0x35;
const TUTORIAL_MESSAGE_COMMAND_WORDS: usize = 7;
const TUTORIAL_VIEWPORT_WIDTH_PIXELS: usize = 640;
const TUTORIAL_VIEWPORT_HEIGHT_PIXELS: usize = 480;
const TUTORIAL_GLYPH_WIDTH_PIXELS: usize = 32;
const TUTORIAL_GLYPH_HEIGHT_PIXELS: usize = 32;
const TUTORIAL_LINE_ADVANCE_PIXELS: usize = 48;

pub(crate) fn declared_message_corpus_memberships() -> BTreeMap<&'static str, Vec<&'static str>> {
    catalog::declared_message_corpus_memberships()
}

#[cfg(test)]
#[path = "message_corpus/message_corpus_tests.rs"]
mod message_corpus_tests;
