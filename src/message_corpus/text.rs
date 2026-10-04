use anyhow::{Context, Result, ensure};
use encoding_rs::SHIFT_JIS;

use super::MessageToken;

pub(super) fn parse_message(source: &[u8]) -> Result<Vec<MessageToken>> {
    ensure!(source.last() == Some(&0), "message has no final NUL");
    ensure!(
        !source[..source.len() - 1].contains(&0),
        "message has an interior NUL"
    );
    let payload = &source[..source.len() - 1];
    let mut cursor = 0_usize;
    let mut tokens = Vec::new();
    while cursor < payload.len() {
        let start = cursor;
        match payload[cursor] {
            b'\n' => {
                cursor += 1;
                tokens.push(token(
                    start,
                    &payload[start..cursor],
                    "line-break",
                    "\\n",
                    "movable-layout",
                ));
            }
            b'@' => {
                let argument = *payload.get(cursor + 1).context("truncated @ control")?;
                cursor += 2;
                match argument {
                    b'P' => tokens.push(token(
                        start,
                        &payload[start..cursor],
                        "dynamic-port",
                        "current memory-card port glyph",
                        "preserve-runtime-control",
                    )),
                    b'0' => tokens.push(token(
                        start,
                        &payload[start..cursor],
                        "color-control",
                        "message color state 0",
                        "preserve-runtime-control",
                    )),
                    b'A'..=b'Z' => tokens.push(token(
                        start,
                        &payload[start..cursor],
                        "button-glyph",
                        &char::from(argument).to_string(),
                        "preserve-runtime-control",
                    )),
                    _ => anyhow::bail!("unsupported @ control argument: {argument:#04x}"),
                }
            }
            byte if byte.is_ascii() && !byte.is_ascii_control() => {
                cursor += 1;
                while cursor < payload.len()
                    && payload[cursor].is_ascii()
                    && !payload[cursor].is_ascii_control()
                    && payload[cursor] != b'@'
                {
                    cursor += 1;
                }
                let raw = &payload[start..cursor];
                let text = std::str::from_utf8(raw).context("ASCII message run is not UTF-8")?;
                tokens.push(token(
                    start,
                    raw,
                    "ascii-text",
                    text,
                    "preserve-ascii-in-japanese-only-scope",
                ));
            }
            byte if shift_jis_unit_length(byte).is_some() => {
                cursor += shift_jis_unit_length(byte).unwrap();
                ensure!(cursor <= payload.len(), "truncated Shift-JIS code unit");
                while cursor < payload.len() {
                    let Some(length) = shift_jis_unit_length(payload[cursor]) else {
                        break;
                    };
                    ensure!(
                        cursor + length <= payload.len(),
                        "truncated Shift-JIS code unit"
                    );
                    cursor += length;
                }
                let raw = &payload[start..cursor];
                let text = decode_shift_jis_exact(raw)?;
                tokens.push(token(
                    start,
                    raw,
                    "shift-jis-text",
                    &text,
                    "translate-japanese",
                ));
            }
            byte => anyhow::bail!("unsupported message byte at {cursor:#x}: {byte:#04x}"),
        }
    }
    Ok(tokens)
}

fn shift_jis_unit_length(first: u8) -> Option<usize> {
    match first {
        0x81..=0x9f | 0xe0..=0xfc => Some(2),
        0xa1..=0xdf => Some(1),
        _ => None,
    }
}

fn decode_shift_jis_exact(source: &[u8]) -> Result<String> {
    let decoded = SHIFT_JIS
        .decode_without_bom_handling_and_without_replacement(source)
        .map(|text| text.into_owned())
        .context("message contains invalid Shift-JIS")?;
    let (encoded, encoding, had_errors) = SHIFT_JIS.encode(&decoded);
    ensure!(
        !had_errors && encoding == SHIFT_JIS && encoded.as_ref() == source,
        "Shift-JIS text run is not byte-reversible"
    );
    Ok(decoded)
}

fn token(
    byte_offset: usize,
    raw: &[u8],
    kind: &'static str,
    value: &str,
    translation_policy: &'static str,
) -> MessageToken {
    MessageToken {
        byte_offset,
        byte_length: raw.len(),
        raw_hex: hex(raw),
        kind,
        value: value.to_string(),
        translation_policy,
    }
}

pub(super) fn reconstruct_message(tokens: &[MessageToken]) -> Vec<u8> {
    let mut output = Vec::new();
    for token in tokens {
        output.extend(decode_hex(&token.raw_hex));
    }
    output.push(0);
    output
}

pub(super) fn display_message(tokens: &[MessageToken]) -> String {
    let mut output = String::new();
    for token in tokens {
        match token.kind {
            "ascii-text" | "shift-jis-text" => output.push_str(&token.value),
            "line-break" => output.push('\n'),
            "button-glyph" => output.push_str(&format!("{{button:{}}}", token.value)),
            "dynamic-port" => output.push_str("{dynamic-port}"),
            "color-control" => output.push_str("{color:0}"),
            _ => unreachable!("parser emitted an unknown message token"),
        }
    }
    output
}

pub(super) fn decode_hex(source: &str) -> Vec<u8> {
    source
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("generated hex is ASCII");
            u8::from_str_radix(text, 16).expect("generated hex is valid")
        })
        .collect()
}

pub(super) fn hex(source: &[u8]) -> String {
    source.iter().map(|byte| format!("{byte:02x}")).collect()
}
