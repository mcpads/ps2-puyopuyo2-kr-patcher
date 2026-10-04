use std::collections::{BTreeMap, VecDeque};

use anyhow::{Result, ensure};

use super::super::{GraphicsSourceBackgroundRestoration, PixelBounds};
use super::model::RegionRestorationInputs;
use crate::indexed_texture::colorize_psmt8;

pub(super) fn restore_source_indices(
    replacement: &mut [u8],
    source_indices: &[u8],
    palette: &[u8],
    inputs: RegionRestorationInputs<'_>,
    restoration: GraphicsSourceBackgroundRestoration,
) -> Result<()> {
    match restoration {
        GraphicsSourceBackgroundRestoration::NearestSourceIndex => {
            restore_source_index_region(replacement, source_indices, palette, inputs)
        }
        GraphicsSourceBackgroundRestoration::DominantSourceIndexPerRow => {
            restore_source_index_rows(replacement, source_indices, inputs)
        }
        GraphicsSourceBackgroundRestoration::TransparentSourceIndex => {
            restore_transparent_source_index(replacement, source_indices, palette, inputs)
        }
    }
}

fn restore_transparent_source_index(
    replacement: &mut [u8],
    source_indices: &[u8],
    palette: &[u8],
    inputs: RegionRestorationInputs<'_>,
) -> Result<()> {
    ensure!(
        palette.len() == 1024
            && replacement.len() == source_indices.len()
            && source_indices.len() == inputs.surface_width * inputs.surface_height
            && inputs.source_mask.len() == source_indices.len()
            && inputs.all_source_masks.len() == source_indices.len()
            && inputs.protected_english.len() == source_indices.len(),
        "transparent source restoration dimensions changed"
    );
    let bounds = inputs.bounds;
    let right = usize::try_from(bounds.x + bounds.width)?;
    let bottom = usize::try_from(bounds.y + bounds.height)?;
    ensure!(
        right <= inputs.surface_width && bottom <= inputs.surface_height,
        "transparent source restoration exceeds the surface"
    );
    let mut transparent = std::collections::BTreeSet::new();
    for y in usize::try_from(bounds.y)?..bottom {
        for x in usize::try_from(bounds.x)?..right {
            let pixel = y * inputs.surface_width + x;
            let index = source_indices[pixel];
            if !inputs.all_source_masks[pixel]
                && !inputs.protected_english[pixel]
                && colorize_psmt8(&[index], palette)[3] == 0
            {
                transparent.insert(index);
            }
        }
    }
    ensure!(
        transparent.len() == 1,
        "transparent source restoration requires one unambiguous source background index"
    );
    let index = *transparent.first().expect("one checked source index");
    for y in usize::try_from(bounds.y)?..bottom {
        for x in usize::try_from(bounds.x)?..right {
            let pixel = y * inputs.surface_width + x;
            if inputs.source_mask[pixel] {
                ensure!(
                    !inputs.protected_english[pixel],
                    "transparent restoration overlaps protected English"
                );
                replacement[pixel] = index;
            }
        }
    }
    Ok(())
}

pub(crate) fn restore_source_region(
    preview: &mut [u8],
    source_rgba: &[u8],
    inputs: RegionRestorationInputs<'_>,
) -> Result<bool> {
    let RegionRestorationInputs {
        surface_width,
        surface_height,
        bounds,
        source_mask,
        all_source_masks,
        protected_english,
    } = inputs;
    ensure!(
        preview.len() == source_rgba.len()
            && source_rgba.len() == surface_width * surface_height * 4
            && source_mask.len() == surface_width * surface_height
            && all_source_masks.len() == source_mask.len()
            && protected_english.len() == source_mask.len(),
        "functional UI background reconstruction dimensions changed"
    );
    let left = usize::try_from(bounds.x)?;
    let top = usize::try_from(bounds.y)?;
    let region_width = usize::try_from(bounds.width)?;
    let region_height = usize::try_from(bounds.height)?;
    ensure!(
        left + region_width <= surface_width && top + region_height <= surface_height,
        "functional UI background reconstruction bounds exceed the source"
    );
    let region_len = region_width * region_height;
    let mut backgrounds = vec![None::<[u8; 4]>; region_len];
    let mut queue = VecDeque::new();
    for local_y in 0..region_height {
        for local_x in 0..region_width {
            let pixel = (top + local_y) * surface_width + left + local_x;
            if !all_source_masks[pixel] && !protected_english[pixel] {
                backgrounds[local_y * region_width + local_x] = Some(
                    source_rgba[pixel * 4..pixel * 4 + 4]
                        .try_into()
                        .expect("RGBA pixel has four bytes"),
                );
                queue.push_back((local_x, local_y));
            }
        }
    }
    if queue.is_empty() {
        return Ok(false);
    }
    while let Some((x, y)) = queue.pop_front() {
        let color = backgrounds[y * region_width + x].expect("queued background has a color");
        let neighbors = [
            x.checked_sub(1).map(|next| (next, y)),
            (x + 1 < region_width).then_some((x + 1, y)),
            y.checked_sub(1).map(|next| (x, next)),
            (y + 1 < region_height).then_some((x, y + 1)),
        ];
        for (next_x, next_y) in neighbors.into_iter().flatten() {
            let next = next_y * region_width + next_x;
            if backgrounds[next].is_none() {
                backgrounds[next] = Some(color);
                queue.push_back((next_x, next_y));
            }
        }
    }
    for local_y in 0..region_height {
        for local_x in 0..region_width {
            let pixel = (top + local_y) * surface_width + left + local_x;
            if source_mask[pixel] {
                preview[pixel * 4..pixel * 4 + 4].copy_from_slice(
                    &backgrounds[local_y * region_width + local_x]
                        .expect("background propagation covers the declared bounds"),
                );
            }
        }
    }
    Ok(true)
}

fn restore_source_index_region(
    replacement: &mut [u8],
    source_indices: &[u8],
    palette: &[u8],
    inputs: RegionRestorationInputs<'_>,
) -> Result<()> {
    let RegionRestorationInputs {
        surface_width,
        surface_height,
        bounds,
        source_mask,
        all_source_masks,
        protected_english,
    } = inputs;
    ensure!(
        replacement.len() == source_indices.len()
            && source_indices.len() == surface_width * surface_height
            && palette.len() == 256 * 4
            && source_mask.len() == source_indices.len()
            && all_source_masks.len() == source_indices.len()
            && protected_english.len() == source_indices.len(),
        "static UI index background reconstruction dimensions changed"
    );
    let left = usize::try_from(bounds.x)?;
    let top = usize::try_from(bounds.y)?;
    let region_width = usize::try_from(bounds.width)?;
    let region_height = usize::try_from(bounds.height)?;
    ensure!(
        left + region_width <= surface_width && top + region_height <= surface_height,
        "static UI index background reconstruction bounds exceed the source"
    );
    if let Some(background_index) = select_uniform_transparent_background_index(
        source_indices,
        palette,
        surface_width,
        bounds,
        all_source_masks,
        protected_english,
    )? {
        for local_y in 0..region_height {
            for local_x in 0..region_width {
                let pixel = (top + local_y) * surface_width + left + local_x;
                if source_mask[pixel] {
                    replacement[pixel] = background_index;
                }
            }
        }
        return Ok(());
    }
    let mut backgrounds = vec![None::<u8>; region_width * region_height];
    let mut queue = VecDeque::new();
    for local_y in 0..region_height {
        for local_x in 0..region_width {
            let pixel = (top + local_y) * surface_width + left + local_x;
            if !all_source_masks[pixel] && !protected_english[pixel] {
                backgrounds[local_y * region_width + local_x] = Some(source_indices[pixel]);
                queue.push_back((local_x, local_y));
            }
        }
    }
    ensure!(
        !queue.is_empty(),
        "static UI region has no background palette index inside its declared bounds"
    );
    while let Some((x, y)) = queue.pop_front() {
        let index = backgrounds[y * region_width + x].expect("queued background has an index");
        let neighbors = [
            x.checked_sub(1).map(|next| (next, y)),
            (x + 1 < region_width).then_some((x + 1, y)),
            y.checked_sub(1).map(|next| (x, next)),
            (y + 1 < region_height).then_some((x, y + 1)),
        ];
        for (next_x, next_y) in neighbors.into_iter().flatten() {
            let next = next_y * region_width + next_x;
            if backgrounds[next].is_none() {
                backgrounds[next] = Some(index);
                queue.push_back((next_x, next_y));
            }
        }
    }
    for local_y in 0..region_height {
        for local_x in 0..region_width {
            let pixel = (top + local_y) * surface_width + left + local_x;
            if source_mask[pixel] {
                replacement[pixel] = backgrounds[local_y * region_width + local_x]
                    .expect("background index propagation covers the declared bounds");
            }
        }
    }
    Ok(())
}

pub(super) fn restore_source_index_rows(
    replacement: &mut [u8],
    source_indices: &[u8],
    inputs: RegionRestorationInputs<'_>,
) -> Result<()> {
    let RegionRestorationInputs {
        surface_width,
        surface_height,
        bounds,
        source_mask,
        all_source_masks,
        protected_english,
    } = inputs;
    ensure!(
        replacement.len() == source_indices.len()
            && source_indices.len() == surface_width * surface_height
            && source_mask.len() == source_indices.len()
            && all_source_masks.len() == source_indices.len()
            && protected_english.len() == source_indices.len(),
        "row-dominant index background reconstruction dimensions changed"
    );
    let left = usize::try_from(bounds.x)?;
    let top = usize::try_from(bounds.y)?;
    let right = usize::try_from(bounds.x + bounds.width)?;
    let bottom = usize::try_from(bounds.y + bounds.height)?;
    ensure!(
        left < right && top < bottom && right <= surface_width && bottom <= surface_height,
        "row-dominant index background reconstruction bounds exceed the source"
    );

    let mut row_indices = Vec::with_capacity(bottom - top);
    for y in top..bottom {
        let mut usage = BTreeMap::<u8, usize>::new();
        for x in left..right {
            let pixel = y * surface_width + x;
            if !all_source_masks[pixel] && !protected_english[pixel] {
                *usage.entry(source_indices[pixel]).or_default() += 1;
            }
        }
        row_indices.push(
            usage
                .into_iter()
                .max_by_key(|(index, count)| (*count, std::cmp::Reverse(*index)))
                .map(|(index, _)| index),
        );
    }
    ensure!(
        row_indices.iter().any(Option::is_some),
        "row-dominant index region has no background palette index"
    );
    for row in 0..row_indices.len() {
        if row_indices[row].is_some() {
            continue;
        }
        row_indices[row] = (1..row_indices.len()).find_map(|distance| {
            row.checked_sub(distance)
                .and_then(|candidate| row_indices[candidate])
                .or_else(|| row_indices.get(row + distance).copied().flatten())
        });
    }
    ensure!(
        row_indices.iter().all(Option::is_some),
        "row-dominant index background did not reach every selected row"
    );

    for (local_y, background_index) in row_indices.into_iter().enumerate() {
        let y = top + local_y;
        let background_index = background_index.expect("every row has a background index");
        for x in left..right {
            let pixel = y * surface_width + x;
            if source_mask[pixel] {
                replacement[pixel] = background_index;
            }
        }
    }
    Ok(())
}

pub(super) fn select_uniform_transparent_background_index(
    source_indices: &[u8],
    palette: &[u8],
    surface_width: usize,
    bounds: PixelBounds,
    all_source_masks: &[bool],
    protected_english: &[bool],
) -> Result<Option<u8>> {
    let left = usize::try_from(bounds.x)?;
    let top = usize::try_from(bounds.y)?;
    let right = usize::try_from(bounds.x + bounds.width)?;
    let bottom = usize::try_from(bounds.y + bounds.height)?;
    let mut usage = BTreeMap::<u8, usize>::new();
    for y in top..bottom {
        for x in left..right {
            let pixel = y * surface_width + x;
            if !all_source_masks[pixel] && !protected_english[pixel] {
                *usage.entry(source_indices[pixel]).or_default() += 1;
            }
        }
    }
    if usage.is_empty() {
        return Ok(None);
    }
    let all_transparent = usage.keys().all(|index| {
        colorize_psmt8(&[*index], palette)
            .get(3)
            .copied()
            .is_some_and(|alpha| alpha == 0)
    });
    if !all_transparent {
        return Ok(None);
    }
    Ok(usage
        .into_iter()
        .max_by_key(|(index, count)| (*count, std::cmp::Reverse(*index)))
        .map(|(index, _)| index))
}
