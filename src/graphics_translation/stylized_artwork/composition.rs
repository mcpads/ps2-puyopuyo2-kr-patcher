use super::*;

pub(super) fn derive_screen_bounds(
    screen_origin: StylizedArtworkScreenPoint,
    pattern_bounds: PixelBounds,
    source_bounds: PixelBounds,
) -> Result<PixelBounds> {
    ensure_bounds_contains(
        pattern_bounds,
        source_bounds,
        "source bounds exceed their pattern rectangle",
    )?;
    let x = i64::from(screen_origin.x) + i64::from(source_bounds.x - pattern_bounds.x);
    let y = i64::from(screen_origin.y) + i64::from(source_bounds.y - pattern_bounds.y);
    ensure!(x >= 0 && y >= 0, "derived screen bounds are negative");
    Ok(PixelBounds {
        x: u32::try_from(x)?,
        y: u32::try_from(y)?,
        width: source_bounds.width,
        height: source_bounds.height,
    })
}

pub(super) struct SourcePaletteRoleInputs<'a> {
    pub palette_selection: StylizedArtworkPaletteSelection,
    pub declared_palette_roles: &'a [StylizedArtworkPaletteRole],
    pub source_mask_asset: &'a HashedAsset,
    pub artwork_composition_bounds: Option<PixelBounds>,
    pub maximum_composition_colors: Option<usize>,
    pub source_indices: &'a [u8],
    pub palette: &'a [u8],
    pub surface_dimensions: (usize, usize),
}

pub(super) fn resolve_source_palette_roles(
    inputs: SourcePaletteRoleInputs<'_>,
) -> Result<Vec<StylizedArtworkPaletteRole>> {
    let SourcePaletteRoleInputs {
        palette_selection,
        declared_palette_roles,
        source_mask_asset,
        artwork_composition_bounds,
        maximum_composition_colors,
        source_indices,
        palette,
        surface_dimensions: (width, height),
    } = inputs;
    if let Some(bounds) = artwork_composition_bounds {
        ensure!(
            palette_selection == StylizedArtworkPaletteSelection::OpaqueSourceMask
                && declared_palette_roles.is_empty(),
            "full artwork composition must select its source palette automatically"
        );
        let colors = select_composition_color_indices(
            source_indices,
            palette,
            width,
            height,
            bounds,
            maximum_composition_colors,
        )?;
        ensure!(
            colors.len() >= 2,
            "stylized artwork composition has too few visible source colors"
        );
        return Ok(colors
            .into_iter()
            .map(|(rgba, palette_index)| StylizedArtworkPaletteRole {
                role: format!("composition_palette_index_{palette_index:03}"),
                palette_index,
                rgba,
            })
            .collect());
    }
    ensure!(
        maximum_composition_colors.is_none(),
        "source palette color limits require a full artwork composition"
    );
    verify_hashed_asset(source_mask_asset)?;
    let source_mask = read_binary_mask(
        Path::new(&source_mask_asset.path),
        width,
        height,
        "stylized artwork source Japanese mask",
    )?;
    let opaque_source_indices = source_indices
        .iter()
        .zip(&source_mask)
        .filter_map(|(index, selected)| {
            (*selected && palette[usize::from(*index) * 4 + 3] != 0).then_some(*index)
        })
        .collect::<BTreeSet<_>>();
    if palette_selection == StylizedArtworkPaletteSelection::OpaqueSourceMask {
        ensure!(
            opaque_source_indices.len() >= 2,
            "stylized artwork source Japanese mask has too few opaque palette colors for automatic selection"
        );
        ensure!(
            declared_palette_roles.is_empty(),
            "opaque-source-mask palette selection cannot also declare named roles"
        );
        let mut colors = BTreeSet::new();
        let mut roles = Vec::with_capacity(opaque_source_indices.len());
        for palette_index in opaque_source_indices {
            let rgba: [u8; 4] = colorize_psmt8(&[palette_index], palette)
                .try_into()
                .expect("one palette index produces one RGBA pixel");
            ensure!(
                rgba[3] == u8::MAX && colors.insert(rgba),
                "stylized artwork source Japanese mask contains transparent or duplicate visible palette colors"
            );
            roles.push(StylizedArtworkPaletteRole {
                role: format!("source_palette_index_{palette_index:03}"),
                palette_index,
                rgba,
            });
        }
        return Ok(roles);
    }

    ensure!(
        !declared_palette_roles.is_empty(),
        "stylized artwork declares no source palette roles"
    );
    let mut declared_indices = BTreeSet::new();
    let mut declared_colors = BTreeSet::new();
    let mut declared_roles = BTreeSet::new();
    for role in declared_palette_roles {
        ensure!(
            !role.role.trim().is_empty()
                && role.rgba[3] == u8::MAX
                && declared_roles.insert(role.role.as_str())
                && declared_indices.insert(role.palette_index)
                && declared_colors.insert(role.rgba),
            "stylized artwork palette roles are empty, transparent, or duplicated"
        );
        let actual: [u8; 4] = colorize_psmt8(&[role.palette_index], palette)
            .try_into()
            .expect("one palette index produces one RGBA pixel");
        ensure!(
            actual == role.rgba,
            "stylized artwork source palette role changed: {}",
            role.role
        );
    }
    ensure!(
        opaque_source_indices == declared_indices,
        "stylized artwork manifest does not cover exactly the opaque palette indices in its Japanese source mask"
    );
    Ok(declared_palette_roles.to_vec())
}

pub(super) fn clear_native_pixels_where_source_is_transparent(
    native: &mut [u8],
    placement: PixelBounds,
    source_indices: &[u8],
    palette: &[u8],
    surface_width: usize,
    surface_height: usize,
) -> Result<()> {
    ensure!(
        native.len() == usize::try_from(placement.width * placement.height)? * 4
            && source_indices.len() == surface_width * surface_height
            && palette.len() == 256 * 4
            && usize::try_from(placement.x + placement.width)? <= surface_width
            && usize::try_from(placement.y + placement.height)? <= surface_height,
        "source-transparency composition dimensions changed"
    );
    let logical_indices = (0..=u8::MAX).collect::<Vec<_>>();
    let palette_rgba = colorize_psmt8(&logical_indices, palette);
    let left = usize::try_from(placement.x)?;
    let top = usize::try_from(placement.y)?;
    let native_width = usize::try_from(placement.width)?;
    let native_height = usize::try_from(placement.height)?;
    for y in 0..native_height {
        for x in 0..native_width {
            let source_index = usize::from(source_indices[(top + y) * surface_width + left + x]);
            if palette_rgba[source_index * 4 + 3] == 0 {
                native[(y * native_width + x) * 4..(y * native_width + x + 1) * 4]
                    .copy_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Ok(())
}

pub(super) fn select_render_palette_roles(
    source_palette_roles: &[StylizedArtworkPaletteRole],
    render_palette_indices: &[u8],
) -> Result<Vec<StylizedArtworkPaletteRole>> {
    if render_palette_indices.is_empty() {
        return Ok(source_palette_roles.to_vec());
    }
    ensure!(
        render_palette_indices.len() >= 2,
        "stylized artwork render palette must contain at least two indices"
    );
    let selected_indices = render_palette_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    ensure!(
        selected_indices.len() == render_palette_indices.len(),
        "stylized artwork render palette contains duplicate indices"
    );
    let render_palette_roles = source_palette_roles
        .iter()
        .filter(|role| selected_indices.contains(&role.palette_index))
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        render_palette_roles.len() == selected_indices.len(),
        "stylized artwork render palette contains an index outside the verified source mask"
    );
    Ok(render_palette_roles)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_native_artwork(
    master: &[u8],
    master_width: usize,
    master_height: usize,
    crop: PixelBounds,
    output_width: u32,
    output_height: u32,
    alpha_threshold: u8,
    palette_roles: &[StylizedArtworkPaletteRole],
    require_every_palette_role: bool,
    require_transparent_pixels_outside_crop: bool,
) -> Result<(Vec<u8>, Vec<StylizedArtworkPaletteRoleUsage>)> {
    ensure!(
        master.len() == master_width * master_height * 4,
        "stylized artwork master dimensions changed"
    );
    ensure_bounds_contains(
        PixelBounds {
            x: 0,
            y: 0,
            width: u32::try_from(master_width)?,
            height: u32::try_from(master_height)?,
        },
        crop,
        "stylized artwork crop exceeds its master",
    )?;
    let crop_left = usize::try_from(crop.x)?;
    let crop_top = usize::try_from(crop.y)?;
    let crop_right = usize::try_from(crop.x + crop.width)?;
    let crop_bottom = usize::try_from(crop.y + crop.height)?;
    if require_transparent_pixels_outside_crop {
        for y in 0..master_height {
            for x in 0..master_width {
                if x >= crop_left && x < crop_right && y >= crop_top && y < crop_bottom {
                    continue;
                }
                ensure!(
                    master[(y * master_width + x) * 4 + 3] < alpha_threshold,
                    "stylized artwork crop excludes a visible master pixel"
                );
            }
        }
    }

    let output_width = usize::try_from(output_width)?;
    let output_height = usize::try_from(output_height)?;
    let crop_width = usize::try_from(crop.width)?;
    let crop_height = usize::try_from(crop.height)?;
    let mut output = vec![0_u8; output_width * output_height * 4];
    let mut counts = BTreeMap::<u8, usize>::new();
    for output_y in 0..output_height {
        for output_x in 0..output_width {
            let x_start = output_x * crop_width;
            let x_end = (output_x + 1) * crop_width;
            let y_start = output_y * crop_height;
            let y_end = (output_y + 1) * crop_height;
            let first_x = x_start / output_width;
            let last_x = (x_end - 1) / output_width;
            let first_y = y_start / output_height;
            let last_y = (y_end - 1) / output_height;
            let mut total_weight = 0_u128;
            let mut alpha_sum = 0_u128;
            let mut premultiplied = [0_u128; 3];
            for source_y in first_y..=last_y {
                let source_y_start = source_y * output_height;
                let source_y_end = (source_y + 1) * output_height;
                let y_weight = y_end.min(source_y_end) - y_start.max(source_y_start);
                for source_x in first_x..=last_x {
                    let source_x_start = source_x * output_width;
                    let source_x_end = (source_x + 1) * output_width;
                    let x_weight = x_end.min(source_x_end) - x_start.max(source_x_start);
                    let weight = u128::try_from(x_weight * y_weight)?;
                    let source_pixel =
                        ((crop_top + source_y) * master_width + crop_left + source_x) * 4;
                    let alpha = u128::from(master[source_pixel + 3]);
                    total_weight += weight;
                    alpha_sum += alpha * weight;
                    for channel in 0..3 {
                        premultiplied[channel] +=
                            u128::from(master[source_pixel + channel]) * alpha * weight;
                    }
                }
            }
            let alpha = ((alpha_sum + total_weight / 2) / total_weight) as u8;
            if alpha < alpha_threshold {
                continue;
            }
            let rgb = premultiplied.map(|sum| ((sum + alpha_sum / 2) / alpha_sum) as u8);
            let role = nearest_palette_role(rgb, palette_roles, require_every_palette_role)?;
            let output_pixel = (output_y * output_width + output_x) * 4;
            output[output_pixel..output_pixel + 4].copy_from_slice(&role.rgba);
            *counts.entry(role.palette_index).or_default() += 1;
        }
    }
    ensure!(!counts.is_empty(), "stylized native artwork is empty");
    let usage = palette_roles
        .iter()
        .filter_map(|role| {
            let native_pixels = counts.get(&role.palette_index).copied().unwrap_or_default();
            if native_pixels == 0 && !require_every_palette_role {
                return None;
            }
            Some(Ok(StylizedArtworkPaletteRoleUsage {
                role: role.role.clone(),
                palette_index: role.palette_index,
                rgba: role.rgba,
                native_pixels,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        !require_every_palette_role || usage.iter().all(|role| role.native_pixels > 0),
        "stylized native artwork does not use every declared palette role"
    );
    Ok((output, usage))
}

pub(super) fn nearest_palette_role(
    rgb: [u8; 3],
    roles: &[StylizedArtworkPaletteRole],
    reject_ambiguous_color: bool,
) -> Result<&StylizedArtworkPaletteRole> {
    let mut best = None;
    for role in roles {
        let distance = rgb
            .iter()
            .zip(role.rgba)
            .take(3)
            .map(|(left, right)| {
                let delta = i32::from(*left) - i32::from(right);
                u32::try_from(delta * delta).expect("squared color distance is nonnegative")
            })
            .sum::<u32>();
        match best {
            None => best = Some((distance, role)),
            Some((best_distance, _)) if distance < best_distance => {
                best = Some((distance, role));
            }
            Some((best_distance, _)) if distance == best_distance && reject_ambiguous_color => {
                anyhow::bail!("stylized artwork color is equidistant between palette roles")
            }
            Some((best_distance, best_role))
                if distance == best_distance && role.palette_index < best_role.palette_index =>
            {
                best = Some((distance, role));
            }
            _ => {}
        }
    }
    best.map(|(_, role)| role)
        .context("stylized artwork has no palette roles")
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paste_native_artwork_clipped(
    surface: &mut [u8],
    claimed: &mut [bool],
    surface_width: usize,
    surface_height: usize,
    native: &[u8],
    native_width: u32,
    native_height: u32,
    placement: StylizedArtworkSignedPoint,
    clip: PixelBounds,
) -> Result<()> {
    let native_width = usize::try_from(native_width)?;
    let native_height = usize::try_from(native_height)?;
    ensure!(
        surface.len() == surface_width * surface_height * 4
            && claimed.len() == surface_width * surface_height
            && native.len() == native_width * native_height * 4,
        "stylized artwork paste dimensions changed"
    );
    ensure_bounds_contains(
        PixelBounds {
            x: 0,
            y: 0,
            width: u32::try_from(surface_width)?,
            height: u32::try_from(surface_height)?,
        },
        clip,
        "stylized artwork clip exceeds its surface",
    )?;
    let clip_left = i64::from(clip.x);
    let clip_top = i64::from(clip.y);
    let clip_right = i64::from(clip.x) + i64::from(clip.width);
    let clip_bottom = i64::from(clip.y) + i64::from(clip.height);
    let mut pasted_pixels = 0_usize;
    for y in 0..native_height {
        for x in 0..native_width {
            let source_pixel = (y * native_width + x) * 4;
            if native[source_pixel + 3] == 0 {
                continue;
            }
            let destination_x = i64::from(placement.x) + i64::try_from(x)?;
            let destination_y = i64::from(placement.y) + i64::try_from(y)?;
            if destination_x < clip_left
                || destination_x >= clip_right
                || destination_y < clip_top
                || destination_y >= clip_bottom
            {
                continue;
            }
            let destination_x = usize::try_from(destination_x)?;
            let destination_y = usize::try_from(destination_y)?;
            let destination_pixel = destination_y * surface_width + destination_x;
            ensure!(
                !claimed[destination_pixel],
                "stylized artwork native placements overlap"
            );
            claimed[destination_pixel] = true;
            surface[destination_pixel * 4..destination_pixel * 4 + 4]
                .copy_from_slice(&native[source_pixel..source_pixel + 4]);
            pasted_pixels += 1;
        }
    }
    ensure!(
        pasted_pixels > 0,
        "stylized artwork native placement contributes no visible pixel inside its clip"
    );
    Ok(())
}

pub(super) fn install_overlay(
    staged_path: &Path,
    output_path: &Path,
    expected_sha256: &str,
    existing_ledger_asset: Option<&HashedAsset>,
    refresh_existing: bool,
) -> Result<()> {
    if let Some(existing) = existing_ledger_asset {
        ensure!(
            existing.path == output_path.to_string_lossy(),
            "stylized artwork ledger points to a different overlay path"
        );
        ensure!(
            existing.sha256 == expected_sha256 || refresh_existing,
            "stylized artwork overlay changed; pass --refresh-existing only after reviewing the new deterministic output"
        );
    }
    if output_path.exists() {
        let existing_sha256 = sha256(
            &fs::read(output_path)
                .with_context(|| format!("failed to hash {}", output_path.display()))?,
        );
        if existing_sha256 == expected_sha256 {
            return Ok(());
        }
        ensure!(
            refresh_existing,
            "stylized artwork output exists with different pixels; pass --refresh-existing only after reviewing the new deterministic output"
        );
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(staged_path, output_path).with_context(|| {
        format!(
            "failed to install stylized artwork overlay {}",
            output_path.display()
        )
    })?;
    let installed_sha256 = sha256(
        &fs::read(output_path)
            .with_context(|| format!("failed to hash {}", output_path.display()))?,
    );
    ensure!(
        installed_sha256 == expected_sha256,
        "installed stylized artwork overlay hash changed"
    );
    Ok(())
}

pub(super) fn hashed_output(path: &Path) -> Result<HashedAsset> {
    Ok(HashedAsset {
        path: path.to_string_lossy().into_owned(),
        sha256: sha256(
            &fs::read(path).with_context(|| format!("failed to hash {}", path.display()))?,
        ),
    })
}

pub(super) fn ensure_bounds_contains(
    outer: PixelBounds,
    inner: PixelBounds,
    message: &str,
) -> Result<()> {
    let outer_right = outer
        .x
        .checked_add(outer.width)
        .context("outer horizontal bounds overflow")?;
    let outer_bottom = outer
        .y
        .checked_add(outer.height)
        .context("outer vertical bounds overflow")?;
    let inner_right = inner
        .x
        .checked_add(inner.width)
        .context("inner horizontal bounds overflow")?;
    let inner_bottom = inner
        .y
        .checked_add(inner.height)
        .context("inner vertical bounds overflow")?;
    ensure!(
        inner.width > 0
            && inner.height > 0
            && inner.x >= outer.x
            && inner.y >= outer.y
            && inner_right <= outer_right
            && inner_bottom <= outer_bottom,
        "{message}"
    );
    Ok(())
}
