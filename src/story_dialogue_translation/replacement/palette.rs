use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::indexed_texture::colorize_psmt8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StoryDialoguePaletteIndices {
    pub(super) transparent: u8,
    pub(super) outline: u8,
    pub(super) fill: u8,
}

pub(super) fn select_story_dialogue_palette_indices(
    source_indices: &[u8],
    palette: &[u8],
) -> Result<StoryDialoguePaletteIndices> {
    let mut usage = BTreeMap::<u8, usize>::new();
    for &index in source_indices {
        *usage.entry(index).or_default() += 1;
    }
    let colors = usage
        .keys()
        .map(|index| {
            let rgba: [u8; 4] = colorize_psmt8(&[*index], palette)
                .try_into()
                .expect("one palette index produces one RGBA pixel");
            (*index, rgba)
        })
        .collect::<BTreeMap<_, _>>();

    let transparent = usage
        .iter()
        .filter(|(index, _)| colors[index][3] == 0)
        .max_by_key(|(index, count)| (**count, std::cmp::Reverse(**index)))
        .map(|(index, _)| *index)
        .context("story dialogue texture uses no transparent palette index")?;
    let opaque = usage
        .keys()
        .copied()
        .filter(|index| colors[index][3] == u8::MAX)
        .collect::<Vec<_>>();
    let outline = opaque
        .iter()
        .copied()
        .min_by_key(|index| (brightness(colors[index]), *index))
        .context("story dialogue texture uses no opaque outline color")?;
    let fill = opaque
        .iter()
        .copied()
        .max_by_key(|index| (brightness(colors[index]), std::cmp::Reverse(*index)))
        .context("story dialogue texture uses no opaque fill color")?;
    ensure!(
        outline != fill && brightness(colors[&outline]) < brightness(colors[&fill]),
        "story dialogue palette does not distinguish outline and fill colors"
    );
    Ok(StoryDialoguePaletteIndices {
        transparent,
        outline,
        fill,
    })
}

fn brightness(rgba: [u8; 4]) -> u16 {
    u16::from(rgba[0]) + u16::from(rgba[1]) + u16::from(rgba[2])
}
