use super::catalog::*;
use super::*;

pub(super) fn read_message_entry(
    file: &object::File<'_>,
    spec: MessageSpec,
    reference_ids: Vec<String>,
) -> Result<MessageEntry> {
    let symbol = file
        .symbols()
        .find(|symbol| symbol.name().is_ok_and(|name| name == spec.symbol))
        .with_context(|| format!("ELF message symbol not found: {}", spec.symbol))?;
    ensure!(
        symbol.is_definition() && symbol.size() > 0,
        "ELF message symbol is not a nonempty definition: {}",
        spec.symbol
    );
    let section_index = symbol
        .section_index()
        .with_context(|| format!("ELF message symbol has no section: {}", spec.symbol))?;
    let section = file.section_by_index(section_index)?;
    let relative = symbol
        .address()
        .checked_sub(section.address())
        .with_context(|| format!("message symbol precedes its section: {}", spec.symbol))?;
    let (section_file_offset, _) = section
        .file_range()
        .with_context(|| format!("message section has no file range: {}", spec.symbol))?;
    let source_file_offset = section_file_offset
        .checked_add(relative)
        .context("message file offset overflow")?;
    let (_, source) = read_symbol_bytes(file, spec.symbol)?;
    let tokens = parse_message(&source)
        .with_context(|| format!("failed to parse message symbol {}", spec.symbol))?;
    let reconstructed = reconstruct_message(&tokens);
    ensure!(
        reconstructed == source,
        "message token roundtrip changed source bytes: {}",
        spec.symbol
    );
    let display_text = display_message(&tokens);
    let protected_english_runs = tokens
        .iter()
        .filter(|token| {
            token.kind == "ascii-text"
                && token
                    .value
                    .chars()
                    .any(|character| character.is_ascii_alphabetic())
        })
        .map(|token| ProtectedEnglishRun {
            byte_offset: token.byte_offset,
            raw_hex: token.raw_hex.clone(),
            text: token.value.clone(),
        })
        .collect();
    Ok(MessageEntry {
        message_id: spec.id.to_string(),
        symbol: spec.symbol.to_string(),
        source_address: format!("{:#010x}", symbol.address()),
        source_file_offset: format!("{source_file_offset:#010x}"),
        storage_byte_length: source.len(),
        payload_byte_length: source.len() - 1,
        source_sha256: sha256(&source),
        source_hex: hex(&source),
        terminator: "00",
        display_text,
        tokens,
        protected_english_runs,
        reference_ids,
        roundtrip: "exact source bytes, including final NUL",
    })
}

pub(super) fn parse_tutorial_font_table(source: &[u8]) -> Result<TutorialFontTable> {
    ensure!(
        source.last() == Some(&0),
        "tutorial font table has no lead-byte terminator"
    );
    let payload = &source[..source.len() - 1];
    ensure!(
        payload.len().is_multiple_of(2),
        "tutorial font table is not a sequence of two-byte codes"
    );
    let code_slots: Vec<[u8; 2]> = payload
        .chunks_exact(2)
        .map(|bytes| [bytes[0], bytes[1]])
        .collect();
    ensure!(
        code_slots.iter().all(|code| code[0] != 0),
        "tutorial font table terminates before its declared end"
    );
    let unique_code_units: BTreeSet<[u8; 2]> = code_slots.iter().copied().collect();
    let mut occurrences = BTreeMap::new();
    for (slot, code) in code_slots.iter().copied().enumerate() {
        let entry = occurrences.entry(code).or_insert((0_usize, slot));
        entry.0 += 1;
    }
    let repeated_code_units = occurrences
        .into_iter()
        .filter(|(_, (count, _))| *count > 1)
        .map(|(code, (occurrences, first_slot))| RepeatedFontCodeUnit {
            raw_hex: format!("{:02x}{:02x}", code[0], code[1]),
            occurrences,
            first_slot,
        })
        .collect();
    Ok(TutorialFontTable {
        code_slots,
        unique_code_units,
        repeated_code_units,
    })
}

pub(super) fn validate_tutorial_message_tokens(
    message: &MessageEntry,
    font_code_units: &BTreeSet<[u8; 2]>,
) -> Result<()> {
    for token in &message.tokens {
        ensure!(
            matches!(token.kind, "shift-jis-text" | "line-break"),
            "tutorial message {} contains an unsupported token kind: {}",
            message.message_id,
            token.kind
        );
        if token.kind != "shift-jis-text" {
            continue;
        }
        let raw = decode_hex(&token.raw_hex);
        ensure!(
            raw.len().is_multiple_of(2),
            "tutorial message {} contains a one-byte text code",
            message.message_id
        );
        for code in raw.chunks_exact(2) {
            ensure!(
                font_code_units.contains(&[code[0], code[1]]),
                "tutorial message {} uses a code absent from FontList_Exp: {:02x}{:02x}",
                message.message_id,
                code[0],
                code[1]
            );
        }
    }
    Ok(())
}

pub(super) struct ControlAccumulator {
    kind: String,
    raw_values: BTreeSet<String>,
    occurrences: usize,
    byte_width: usize,
    policy: String,
    effect: String,
}

impl ControlAccumulator {
    pub(super) fn from_token(token: &MessageToken) -> Self {
        Self {
            kind: token.kind.to_string(),
            raw_values: BTreeSet::new(),
            occurrences: 0,
            byte_width: token.byte_length,
            policy: token.translation_policy.to_string(),
            effect: match token.kind {
                "line-break" => {
                    "advance the input by one byte, reset x, and advance y by 24 pixels"
                }
                "button-glyph" => "render FontPrintBtn_Act index from the A-Z argument",
                "dynamic-port" => {
                    "select the current memory-card port glyph and render it through FontPrint_Act"
                }
                "color-control" => "set the message actor alpha to 0x80 and RGB channels to 0x40",
                _ => unreachable!("text token entered the control accumulator"),
            }
            .to_string(),
        }
    }

    pub(super) fn record(&mut self, token: &MessageToken) {
        self.raw_values.insert(token.raw_hex.clone());
        self.occurrences += 1;
        assert_eq!(self.kind, token.kind);
        assert_eq!(self.byte_width, token.byte_length);
        assert_eq!(self.policy, token.translation_policy);
    }

    pub(super) fn finish(self) -> ControlCodeUsage {
        ControlCodeUsage {
            kind: self.kind,
            raw_values: self.raw_values.into_iter().collect(),
            occurrences: self.occurrences,
            byte_width: self.byte_width,
            policy: self.policy,
            confirmed_effect: self.effect,
        }
    }
}
