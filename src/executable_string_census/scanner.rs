use anyhow::{Context, Result};
use encoding_rs::SHIFT_JIS;
use object::{Object, ObjectSection, ObjectSymbol};

pub(super) struct StringScan {
    pub exact_string_symbols: usize,
    pub japanese_strings: Vec<ScannedJapaneseString>,
}

pub(super) struct ScannedJapaneseString {
    pub symbol: String,
    pub address: u32,
    pub file_offset: u64,
    pub bytes: Vec<u8>,
    pub display_text: String,
    pub japanese_characters: usize,
}

pub(super) fn read_japanese_string_symbols(file: &object::File<'_>) -> Result<StringScan> {
    let mut exact_string_symbols = 0;
    let mut japanese_strings = Vec::new();
    for symbol in file.symbols() {
        let Some((file_offset, bytes)) = read_symbol_bytes(file, &symbol)? else {
            continue;
        };
        let Some((display_text, japanese_characters)) = decode_exact_string(&bytes) else {
            continue;
        };
        exact_string_symbols += 1;
        if japanese_characters == 0 {
            continue;
        }
        let Ok(symbol_name) = symbol.name() else {
            continue;
        };
        japanese_strings.push(ScannedJapaneseString {
            symbol: symbol_name.to_string(),
            address: u32::try_from(symbol.address())
                .with_context(|| format!("string address exceeds 32 bits: {symbol_name}"))?,
            file_offset,
            bytes,
            display_text,
            japanese_characters,
        });
    }
    japanese_strings.sort_by_key(|string| string.address);
    Ok(StringScan {
        exact_string_symbols,
        japanese_strings,
    })
}

pub(crate) fn symbol_file_range(
    file: &object::File<'_>,
    symbol: &object::Symbol<'_, '_>,
) -> Result<Option<(u64, usize)>> {
    if !symbol.is_definition() || symbol.size() == 0 {
        return Ok(None);
    }
    let Some(section_index) = symbol.section_index() else {
        return Ok(None);
    };
    let section = file.section_by_index(section_index)?;
    let Some(relative) = symbol.address().checked_sub(section.address()) else {
        return Ok(None);
    };
    let Some((section_file_offset, section_file_size)) = section.file_range() else {
        return Ok(None);
    };
    if relative + symbol.size() > section_file_size {
        return Ok(None);
    }
    Ok(Some((
        section_file_offset + relative,
        usize::try_from(symbol.size())?,
    )))
}

fn read_symbol_bytes(
    file: &object::File<'_>,
    symbol: &object::Symbol<'_, '_>,
) -> Result<Option<(u64, Vec<u8>)>> {
    let Some((file_offset, length)) = symbol_file_range(file, symbol)? else {
        return Ok(None);
    };
    let section_index = symbol
        .section_index()
        .context("file-backed symbol has no section")?;
    let section = file.section_by_index(section_index)?;
    let relative = usize::try_from(symbol.address() - section.address())?;
    let end = relative
        .checked_add(length)
        .context("symbol range overflow")?;
    Ok(section
        .data()?
        .get(relative..end)
        .map(|bytes| (file_offset, bytes.to_vec())))
}

pub(crate) fn decode_exact_string(source: &[u8]) -> Option<(String, usize)> {
    if source.len() < 2 || source.last() != Some(&0) || source[..source.len() - 1].contains(&0) {
        return None;
    }
    let payload = &source[..source.len() - 1];
    let decoded = SHIFT_JIS
        .decode_without_bom_handling_and_without_replacement(payload)?
        .into_owned();
    if decoded
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return None;
    }
    let (encoded, encoding, had_errors) = SHIFT_JIS.encode(&decoded);
    if had_errors || encoding != SHIFT_JIS || encoded.as_ref() != payload {
        return None;
    }
    let japanese_characters = decoded
        .chars()
        .filter(|character| is_japanese(*character))
        .count();
    Some((decoded, japanese_characters))
}

fn is_japanese(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
            | '\u{ff61}'..='\u{ff9f}'
    )
}

#[cfg(test)]
#[path = "scanner_tests.rs"]
mod scanner_tests;
