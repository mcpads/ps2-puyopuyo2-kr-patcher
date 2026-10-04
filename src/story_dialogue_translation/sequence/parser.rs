use anyhow::{Context, Result, ensure};

pub(super) const MESSAGE_WINDOW_OPCODE: usize = 0x34;
pub(super) const MESSAGE_WINDOW_COMMAND_WORDS: usize = 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MessageWindowCommand {
    pub(super) dialogue_texture_index: usize,
    pub(super) window_texture_index: usize,
    pub(super) source_row: usize,
    pub(super) width_cells: usize,
    pub(super) height_cells: usize,
}

pub(super) fn parse_command_sizes(source: &[u8]) -> Result<Vec<usize>> {
    ensure!(
        source.len().is_multiple_of(2),
        "SeqComSize is not a sequence of 16-bit sizes"
    );
    source
        .chunks_exact(2)
        .map(|bytes| Ok(usize::from(u16::from_le_bytes(bytes.try_into()?))))
        .collect()
}

pub(super) fn parse_message_window_commands(
    symbol: &str,
    source: &[u8],
    command_sizes: &[usize],
) -> Result<Vec<MessageWindowCommand>> {
    let mut offset = 0;
    let mut commands = Vec::new();
    while offset < source.len() {
        let opcode_bytes = source
            .get(offset..offset + 2)
            .with_context(|| format!("{symbol} ends inside a sequence opcode"))?;
        let opcode = usize::from(u16::from_le_bytes(opcode_bytes.try_into()?));
        let command_words = *command_sizes
            .get(opcode)
            .with_context(|| format!("{symbol} uses opcode {opcode:#x} outside SeqComSize"))?;
        ensure!(
            command_words > 0,
            "{symbol} uses zero-sized opcode {opcode:#x}"
        );
        let command_bytes = command_words
            .checked_mul(2)
            .context("story sequence command length overflow")?;
        let command = source
            .get(offset..offset + command_bytes)
            .with_context(|| format!("{symbol} ends inside opcode {opcode:#x}"))?;
        if opcode == MESSAGE_WINDOW_OPCODE {
            ensure!(
                command_words == MESSAGE_WINDOW_COMMAND_WORDS,
                "{symbol} message-window command size changed"
            );
            commands.push(MessageWindowCommand {
                dialogue_texture_index: usize::from(command[2]),
                window_texture_index: usize::from(command[3]),
                source_row: usize::from(command[8]),
                width_cells: usize::from(command[13]),
                height_cells: usize::from(command[12]),
            });
        }
        offset += command_bytes;
    }
    Ok(commands)
}
