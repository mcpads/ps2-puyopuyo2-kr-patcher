use super::background::clear_border_connected_light_neutral;
use super::composition::*;
use super::consumer::*;
use super::*;

pub fn render_stylized_artwork_draft(
    inputs: StylizedArtworkDraftInputs<'_>,
) -> Result<StylizedArtworkDraftReport> {
    validate_asset_header(inputs.asset)?;
    ensure!(
        inputs.graphics.source.source_bin_sha256 == inputs.catalog.source_bin_sha256
            && inputs.graphics.source.source_bin_sha256 == inputs.previews.source_bin_sha256
            && inputs.graphics.source.executable_sha256 == sha256(inputs.executable)
            && inputs.graphics.source.executable_sha256 == inputs.previews.executable_sha256,
        "stylized artwork inputs differ from the exact graphics source"
    );

    let target_index = inputs
        .graphics
        .entries
        .iter()
        .position(|entry| entry.source.source_surface_id == inputs.asset.source_surface_id)
        .with_context(|| {
            format!(
                "graphics ledger has no stylized artwork target: {}",
                inputs.asset.source_surface_id
            )
        })?;
    let target = &inputs.graphics.entries[target_index];
    ensure!(
        matches!(
            target.authoring.status,
            GraphicsTranslationStatus::InProgress | GraphicsTranslationStatus::NeedsHumanReview
        ) && target.authoring.replacement_image.is_none()
            && target.authoring.human_approval.is_none()
            && target.authoring.independent_review.is_none(),
        "stylized artwork target is not in a bounded pre-human-review state"
    );
    let region_index = target
        .authoring
        .regions
        .iter()
        .position(|region| region.region_id == inputs.asset.region_id)
        .with_context(|| {
            format!(
                "stylized artwork target has no region: {}",
                inputs.asset.region_id
            )
        })?;
    let region = target.authoring.regions[region_index].clone();
    ensure!(
        region.source_text == inputs.asset.expected_source_text
            && region.korean_text.as_deref() == Some(&inputs.asset.expected_korean_text)
            && region.source_background_restoration == inputs.asset.source_background_restoration
            && region.treatment_profile == GraphicsTreatmentProfile::StylizedDisplayLettering
            && region.artwork_plan.is_some_and(|plan| {
                plan.method == GraphicsArtworkMethod::GeneratedOrHandAuthored
            }),
        "stylized artwork wording or treatment differs from its graphics region"
    );
    ensure_bounds_contains(
        region.bounds,
        inputs.asset.native_placement,
        "native artwork placement exceeds its exact Japanese region",
    )?;

    let asset_bytes = fs::read(inputs.asset_path)
        .with_context(|| format!("failed to hash {}", inputs.asset_path.display()))?;
    let asset_path = inputs.asset_path.to_string_lossy();
    let asset_sha256 = sha256(&asset_bytes);
    ensure!(
        target
            .authoring
            .design_candidates
            .iter()
            .any(|candidate| { candidate.path == asset_path && candidate.sha256 == asset_sha256 }),
        "stylized artwork manifest is not hash-bound by the graphics ledger"
    );

    let declaration = inputs
        .catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == inputs.asset.source_surface_id)
        .context("static region catalog has no stylized artwork surface")?;
    let declared_region = declaration
        .regions
        .iter()
        .find(|candidate| candidate.region_id == inputs.asset.region_id)
        .context("static region catalog has no stylized artwork region")?;
    ensure!(
        declared_region.source_text == inputs.asset.expected_source_text
            && declared_region.korean_text.as_deref() == Some(&inputs.asset.expected_korean_text)
            && declared_region.bounds == region.bounds
            && declared_region.source_restoration_bounds == region.source_restoration_bounds
            && declared_region.source_background_restoration
                == region.source_background_restoration
            && declared_region.treatment_profile
                == GraphicsTreatmentProfile::StylizedDisplayLettering
            && declared_region.artwork_plan.is_some_and(|plan| {
                plan.method == GraphicsArtworkMethod::GeneratedOrHandAuthored
                    && plan.placement_status == inputs.asset.consumer.adopted_placement_status()
            }),
        "static region catalog does not declare the adopted artwork placement evidence"
    );

    let source_preview = find_source_preview(inputs.previews, target, declaration, inputs.asset)?;
    let container_identity = inputs
        .previews
        .containers
        .iter()
        .find(|container| container.iso_path == target.source.container_iso_path)
        .context("preview corpus has no stylized artwork image container")?;
    ensure!(
        sha256(inputs.image_container) == container_identity.sha256,
        "stylized artwork image container differs from the preview corpus"
    );

    let file = object::File::parse(inputs.executable)
        .context("failed to parse stylized artwork executable")?;
    ensure!(
        file.format() == BinaryFormat::Elf
            && file.architecture() == Architecture::Mips
            && file.endianness() == Endianness::Little,
        "stylized artwork executable is not the expected little-endian MIPS ELF"
    );
    let demo_registration = inputs
        .asset
        .demo_registration_route
        .as_ref()
        .map(|route| validate_demo_registration_route(&file, inputs.executable, route))
        .transpose()?;
    let consumer = validate_consumer(&file, inputs.executable, inputs.asset, &region)?;

    let primary_load_route = inputs.asset.consumer.primary_load_route()?;
    let file_list_symbol = match target.source.container_iso_path.as_str() {
        "/IMAGE_D.BIN;1" => "FileList_Image",
        "/IMAGE_G.BIN;1" => "FileList_Image2",
        "/IMAGE_MD.BIN;1" => "FileList_Image3",
        path => anyhow::bail!("unsupported stylized artwork image container: {path}"),
    };
    let (_, file_list_bytes) = read_symbol_bytes(&file, file_list_symbol)?;
    let records = parse_file_records(&file_list_bytes)?;
    let image_record = *records
        .get(target.source.image_file_record)
        .context("stylized artwork image file record is out of range")?;
    let palette_record = *records
        .get(primary_load_route.palette_file_record)
        .context("stylized artwork palette file record is out of range")?;
    ensure!(
        image_record.width == target.source.width
            && image_record.height == target.source.height
            && target.source.image_file_record == primary_load_route.image_file_record,
        "stylized artwork image dimensions or file record changed"
    );
    let width = usize::try_from(image_record.width)?;
    let height = usize::try_from(image_record.height)?;
    let decoded = read_decoded_image(inputs.image_container, image_record)?;
    let image_byte_length = width
        .checked_mul(height)
        .context("stylized artwork image byte length overflow")?;
    let swizzled = decoded
        .get(..image_byte_length)
        .context("stylized artwork decoded image is shorter than its dimensions")?;
    let source_indices = unswizzle_psmt8_uploaded_as_psmct32(swizzled, width, height)?;
    ensure!(
        sha256(&source_indices) == target.source.linear_indices_sha256
            && u64::try_from(decoded.len() - image_byte_length)?
                == source_preview.decoded_trailing_byte_length,
        "stylized artwork source indices or trailing data changed"
    );
    let palette = read_palette(
        inputs.image_container,
        palette_record,
        source_preview.clear_first_alpha,
    )?;
    let source_rgba = colorize_psmt8(&source_indices, &palette);
    ensure!(
        sha256(&source_rgba) == source_preview.rgba_sha256,
        "stylized artwork source palette render changed"
    );
    let resolved_palette_roles = resolve_source_palette_roles(SourcePaletteRoleInputs {
        palette_selection: inputs.asset.source_palette_selection,
        declared_palette_roles: &inputs.asset.source_palette_roles,
        source_mask_asset: &region.source_pixel_mask,
        artwork_composition_bounds: None,
        maximum_composition_colors: None,
        source_indices: &source_indices,
        palette: &palette,
        surface_dimensions: (width, height),
    })
    .with_context(|| {
        format!(
            "failed to resolve source palette roles for {}",
            inputs.asset.region_id
        )
    })?;
    let render_palette_roles = select_render_palette_roles(
        &resolved_palette_roles,
        &inputs.asset.render_palette_indices,
    )?;

    verify_hashed_asset(&inputs.asset.transparent_master)?;
    let master = read_rgba_surface(Path::new(&inputs.asset.transparent_master.path))?;
    let (native_pixels, palette_roles) = render_native_artwork(
        &master.pixels,
        master.width,
        master.height,
        inputs.asset.master_crop,
        inputs.asset.native_placement.width,
        inputs.asset.native_placement.height,
        inputs.asset.alpha_threshold,
        &render_palette_roles,
        inputs.asset.source_palette_selection
            == StylizedArtworkPaletteSelection::ExactDeclaredRoles,
        true,
    )?;
    let mut overlay = vec![0_u8; width * height * 4];
    let mut claimed_overlay = vec![false; width * height];
    let mut placements = vec![StylizedArtworkSignedPoint {
        x: i32::try_from(inputs.asset.native_placement.x)?,
        y: i32::try_from(inputs.asset.native_placement.y)?,
    }];
    placements.extend_from_slice(&inputs.asset.additional_native_placements);
    for placement in placements {
        paste_native_artwork_clipped(
            &mut overlay,
            &mut claimed_overlay,
            width,
            height,
            &native_pixels,
            inputs.asset.native_placement.width,
            inputs.asset.native_placement.height,
            placement,
            region.bounds,
        )?;
    }
    clip_overlay_to_consumer_patterns(
        &mut overlay,
        &mut claimed_overlay,
        width,
        height,
        &inputs.asset.consumer,
    )?;
    let composition = compose_static_ui_indices(StaticUiIndexCompositionInputs {
        source_indices: &source_indices,
        palette: &palette,
        width,
        height,
        regions: std::slice::from_ref(&region),
        protected_english_mask: target.authoring.protected_english_mask.as_ref(),
        overlay: &overlay,
    })?;
    ensure!(
        composition.changed_protected_indices == 0
            && composition.changed_protected_english_indices == 0,
        "stylized artwork composition changed protected pixels"
    );
    let replacement_rgba = colorize_psmt8(&composition.replacement_indices, &palette);

    write_rgba_png(
        inputs.native_preview_path,
        inputs.asset.native_placement.width,
        inputs.asset.native_placement.height,
        &native_pixels,
    )?;
    let native_artwork = hashed_output(inputs.native_preview_path)?;
    write_rgba_png(
        inputs.surface_preview_path,
        image_record.width,
        image_record.height,
        &replacement_rgba,
    )?;
    let surface_preview = hashed_output(inputs.surface_preview_path)?;

    let staging = tempfile::tempdir().context("failed to stage stylized artwork overlay")?;
    let staged_overlay = staging.path().join("overlay.png");
    write_rgba_png(
        &staged_overlay,
        image_record.width,
        image_record.height,
        &overlay,
    )?;
    let overlay_sha256 = sha256(
        &fs::read(&staged_overlay)
            .with_context(|| format!("failed to hash {}", staged_overlay.display()))?,
    );
    let overlay_path = Path::new(&inputs.asset.replacement_overlay_path);
    install_overlay(
        &staged_overlay,
        overlay_path,
        &overlay_sha256,
        target.authoring.replacement_overlay.as_ref(),
        inputs.refresh_existing,
    )?;
    let replacement_overlay = HashedAsset {
        path: inputs.asset.replacement_overlay_path.clone(),
        sha256: overlay_sha256,
    };

    let target = &mut inputs.graphics.entries[target_index];
    target.authoring.regions[region_index]
        .artwork_plan
        .as_mut()
        .expect("stylized artwork plan was validated")
        .placement_status = inputs.asset.consumer.adopted_placement_status();
    ensure!(
        target.authoring.regions.iter().all(|candidate| {
            candidate
                .artwork_plan
                .is_none_or(|plan| plan.placement_status.is_buildable())
        }),
        "stylized artwork surface still contains a rough region and cannot enter human review"
    );
    target.authoring.replacement_overlay = Some(replacement_overlay.clone());
    target.authoring.status = GraphicsTranslationStatus::NeedsHumanReview;

    Ok(StylizedArtworkDraftReport {
        schema: 1,
        candidate_id: inputs.asset.candidate_id.clone(),
        source_bin_sha256: inputs.graphics.source.source_bin_sha256.clone(),
        executable_sha256: inputs.graphics.source.executable_sha256.clone(),
        source_surface_id: inputs.asset.source_surface_id.clone(),
        region_id: inputs.asset.region_id.clone(),
        source_text: inputs.asset.expected_source_text.clone(),
        korean_text: inputs.asset.expected_korean_text.clone(),
        source_background_restoration: inputs.asset.source_background_restoration,
        source_preview: HashedAsset {
            path: source_preview.png_path.clone(),
            sha256: source_preview.png_sha256.clone(),
        },
        transparent_master: inputs.asset.transparent_master.clone(),
        master_crop: inputs.asset.master_crop,
        native_placement: inputs.asset.native_placement,
        additional_native_placements: inputs.asset.additional_native_placements.clone(),
        native_artwork,
        replacement_overlay,
        surface_preview,
        palette_roles,
        demo_registration,
        consumer,
        source_japanese_pixels: composition.source_japanese_pixels,
        korean_overlay_pixels: composition.korean_overlay_pixels,
        changed_palette_indices: composition.changed_indices,
        protected_pixels: composition.protected_pixels,
        changed_protected_indices: composition.changed_protected_indices,
        changed_protected_english_indices: composition.changed_protected_english_indices,
        status: "needs_human_review",
        release_candidate_eligible: false,
        evidence_boundary: inputs.asset.consumer.report_evidence_boundary(),
    })
}

pub fn render_stylized_artwork_set_draft(
    inputs: StylizedArtworkSetDraftInputs<'_>,
) -> Result<StylizedArtworkSetDraftReport> {
    validate_set_asset_header(inputs.asset)?;
    ensure!(
        inputs.graphics.source.source_bin_sha256 == inputs.catalog.source_bin_sha256
            && inputs.graphics.source.source_bin_sha256 == inputs.previews.source_bin_sha256
            && inputs.graphics.source.executable_sha256 == sha256(inputs.executable)
            && inputs.graphics.source.executable_sha256 == inputs.previews.executable_sha256,
        "stylized artwork set inputs differ from the exact graphics source"
    );

    let target_index = inputs
        .graphics
        .entries
        .iter()
        .position(|entry| entry.source.source_surface_id == inputs.asset.source_surface_id)
        .with_context(|| {
            format!(
                "graphics ledger has no stylized artwork set target: {}",
                inputs.asset.source_surface_id
            )
        })?;
    let target = &inputs.graphics.entries[target_index];
    ensure!(
        matches!(
            target.authoring.status,
            GraphicsTranslationStatus::InProgress | GraphicsTranslationStatus::NeedsHumanReview
        ) && target.authoring.replacement_image.is_none()
            && target.authoring.human_approval.is_none()
            && target.authoring.independent_review.is_none(),
        "stylized artwork set target is not in a bounded pre-human-review state"
    );
    let member_region_ids = inputs
        .asset
        .members
        .iter()
        .map(|member| member.region_id.as_str())
        .collect::<BTreeSet<_>>();
    let target_region_ids = target
        .authoring
        .regions
        .iter()
        .filter(|region| {
            region
                .artwork_plan
                .is_some_and(|plan| plan.method == inputs.asset.artwork_method)
        })
        .map(|region| region.region_id.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        member_region_ids == target_region_ids,
        "stylized artwork set does not cover the exact authored region set"
    );

    let asset_bytes = fs::read(inputs.asset_path)
        .with_context(|| format!("failed to hash {}", inputs.asset_path.display()))?;
    let asset_path = inputs.asset_path.to_string_lossy();
    let asset_sha256 = sha256(&asset_bytes);
    ensure!(
        target
            .authoring
            .design_candidates
            .iter()
            .any(|candidate| candidate.path == asset_path && candidate.sha256 == asset_sha256),
        "stylized artwork set manifest is not hash-bound by the graphics ledger"
    );

    let declaration = inputs
        .catalog
        .surfaces
        .iter()
        .find(|surface| surface.source_surface_id == inputs.asset.source_surface_id)
        .context("static region catalog has no stylized artwork set surface")?;
    let first_member = inputs
        .asset
        .members
        .first()
        .context("stylized artwork set has no members")?;
    let primary_load_route = first_member.consumer.primary_load_route()?;
    ensure!(
        inputs.asset.members.iter().all(|member| {
            member
                .consumer
                .primary_load_route()
                .is_ok_and(|route| route == primary_load_route)
        }),
        "stylized artwork set members disagree on the physical source and palette"
    );
    let source_preview = find_source_preview_for_consumer(
        inputs.previews,
        target,
        declaration,
        &first_member.consumer,
    )?;
    let container_identity = inputs
        .previews
        .containers
        .iter()
        .find(|container| container.iso_path == target.source.container_iso_path)
        .context("preview corpus has no stylized artwork set image container")?;
    ensure!(
        sha256(inputs.image_container) == container_identity.sha256,
        "stylized artwork set image container differs from the preview corpus"
    );

    let file = object::File::parse(inputs.executable)
        .context("failed to parse stylized artwork set executable")?;
    ensure!(
        file.format() == BinaryFormat::Elf
            && file.architecture() == Architecture::Mips
            && file.endianness() == Endianness::Little,
        "stylized artwork set executable is not the expected little-endian MIPS ELF"
    );
    let sequential_registration = inputs
        .asset
        .sequential_registration_route
        .as_ref()
        .map(|route| validate_sequential_registration_route(&file, inputs.executable, route))
        .transpose()?;
    let indexed_registration = inputs
        .asset
        .indexed_registration_route
        .as_ref()
        .map(|route| validate_indexed_registration_route(&file, inputs.executable, route))
        .transpose()?;
    let demo_registration = inputs
        .asset
        .demo_registration_route
        .as_ref()
        .map(|route| validate_demo_registration_route(&file, inputs.executable, route))
        .transpose()?;
    let legacy_demo_registration = inputs
        .asset
        .legacy_demo_registration_route
        .as_ref()
        .map(|route| validate_legacy_demo_registration_route(&file, inputs.executable, route))
        .transpose()?;
    let character_face_registration = inputs
        .asset
        .character_face_registration_route
        .as_ref()
        .map(|route| validate_character_face_registration_route(&file, inputs.executable, route))
        .transpose()?;
    let file_list_symbol = match target.source.container_iso_path.as_str() {
        "/IMAGE_D.BIN;1" => "FileList_Image",
        "/IMAGE_G.BIN;1" => "FileList_Image2",
        "/IMAGE_MD.BIN;1" => "FileList_Image3",
        path => anyhow::bail!("unsupported stylized artwork set image container: {path}"),
    };
    let (_, file_list_bytes) = read_symbol_bytes(&file, file_list_symbol)?;
    let records = parse_file_records(&file_list_bytes)?;
    let image_record = *records
        .get(target.source.image_file_record)
        .context("stylized artwork set image file record is out of range")?;
    let palette_record = *records
        .get(primary_load_route.palette_file_record)
        .context("stylized artwork set palette file record is out of range")?;
    ensure!(
        image_record.width == target.source.width
            && image_record.height == target.source.height
            && target.source.image_file_record == primary_load_route.image_file_record,
        "stylized artwork set image dimensions or file record changed"
    );
    let width = usize::try_from(image_record.width)?;
    let height = usize::try_from(image_record.height)?;
    let decoded = read_decoded_image(inputs.image_container, image_record)?;
    let image_byte_length = width
        .checked_mul(height)
        .context("stylized artwork set image byte length overflow")?;
    let swizzled = decoded
        .get(..image_byte_length)
        .context("stylized artwork set decoded image is shorter than its dimensions")?;
    let source_indices = unswizzle_psmt8_uploaded_as_psmct32(swizzled, width, height)?;
    ensure!(
        sha256(&source_indices) == target.source.linear_indices_sha256
            && u64::try_from(decoded.len() - image_byte_length)?
                == source_preview.decoded_trailing_byte_length,
        "stylized artwork set source indices or trailing data changed"
    );
    let palette = read_palette(
        inputs.image_container,
        palette_record,
        source_preview.clear_first_alpha,
    )?;
    let source_rgba = colorize_psmt8(&source_indices, &palette);
    ensure!(
        sha256(&source_rgba) == source_preview.rgba_sha256,
        "stylized artwork set source palette render changed"
    );

    fs::create_dir_all(inputs.native_preview_directory).with_context(|| {
        format!(
            "failed to create {}",
            inputs.native_preview_directory.display()
        )
    })?;
    let (mut overlay, mut claimed_overlay, base_region_ids, mut selected_regions) =
        initialize_stylized_set_overlay(inputs.asset, target, width, height)?;
    let mut member_reports = Vec::with_capacity(inputs.asset.members.len());
    let mut pattern_records = BTreeSet::new();
    let mut mode_state_pattern_tables = BTreeSet::new();
    for member in &inputs.asset.members {
        let region = target
            .authoring
            .regions
            .iter()
            .find(|region| region.region_id == member.region_id)
            .with_context(|| {
                format!(
                    "stylized artwork set target has no region: {}",
                    member.region_id
                )
            })?
            .clone();
        ensure!(
            region.source_text == member.expected_source_text
                && region.korean_text.as_deref() == Some(&member.expected_korean_text)
                && region.source_background_restoration
                    == inputs.asset.source_background_restoration
                && region.treatment_profile == GraphicsTreatmentProfile::StylizedDisplayLettering
                && region
                    .artwork_plan
                    .is_some_and(|plan| { plan.method == inputs.asset.artwork_method }),
            "stylized artwork set wording or treatment differs from its graphics region"
        );
        let composition_bounds = region.artwork_composition_bounds.unwrap_or(region.bounds);
        let render_clip_bounds = member.render_clip_bounds.unwrap_or(composition_bounds);
        ensure_bounds_contains(
            composition_bounds,
            render_clip_bounds,
            "set-member render clip exceeds its artwork composition",
        )?;
        let resized_native_placements = member
            .consumer
            .resized_native_placements(member.native_placement)?;
        if region.artwork_composition_bounds.is_some() {
            let full_mode_selection_states = matches!(
                &member.consumer,
                StylizedArtworkConsumer::ModeSelectionStates(consumer)
                    if consumer.state_artwork_bounds == consumer.state_pattern_source_bounds
                        && enclosing_pixel_bounds(&resized_native_placements)?
                            == composition_bounds
            );
            ensure!(
                ((resized_native_placements == [composition_bounds]
                    && member.native_placement == composition_bounds)
                    || full_mode_selection_states)
                    && member.render_clip_bounds.is_none()
                    && member.additional_native_placements.is_empty(),
                "full artwork composition must replace one exact consumer rectangle or one exact mode-state set"
            );
        }
        for placement in &resized_native_placements {
            ensure_bounds_contains(
                PixelBounds {
                    x: 0,
                    y: 0,
                    width: u32::try_from(width)?,
                    height: u32::try_from(height)?,
                },
                *placement,
                "resized native set-member placement exceeds its source surface",
            )?;
            if member.render_clip_bounds.is_some() {
                ensure!(
                    pixel_bounds_overlap(*placement, render_clip_bounds),
                    "clipped native set-member placement does not overlap its render clip"
                );
            } else {
                ensure_bounds_contains(
                    composition_bounds,
                    *placement,
                    "resized native set-member placement exceeds its artwork composition",
                )?;
            }
        }
        if resized_native_placements.len() > 1 {
            ensure!(
                member.additional_native_placements.is_empty()
                    && member.render_clip_bounds.is_none(),
                "resized native state variants cannot also declare point copies or one shared clip"
            );
        }
        let declared_region = declaration
            .regions
            .iter()
            .find(|candidate| candidate.region_id == member.region_id)
            .with_context(|| {
                format!(
                    "static region catalog has no stylized artwork set region: {}",
                    member.region_id
                )
            })?;
        ensure!(
            declared_region.source_text == member.expected_source_text
                && declared_region.korean_text.as_deref() == Some(&member.expected_korean_text)
                && declared_region.bounds == region.bounds
                && declared_region.artwork_composition_bounds == region.artwork_composition_bounds
                && declared_region.artwork_uses_overlay_alpha == region.artwork_uses_overlay_alpha
                && declared_region.source_restoration_bounds == region.source_restoration_bounds
                && declared_region.source_background_restoration
                    == region.source_background_restoration
                && declared_region.treatment_profile
                    == GraphicsTreatmentProfile::StylizedDisplayLettering
                && declared_region.artwork_plan.is_some_and(|plan| {
                    plan.method == inputs.asset.artwork_method
                        && plan.placement_status == member.consumer.adopted_placement_status()
                }),
            "static region catalog does not declare the adopted set-member placement evidence"
        );
        let consumer = validate_consumer_for_placement(
            &file,
            inputs.executable,
            &member.consumer,
            member.native_placement,
            &region,
            StylizedArtworkRegistrationRoutes {
                sequential: inputs.asset.sequential_registration_route.as_ref(),
                indexed: inputs.asset.indexed_registration_route.as_ref(),
                demo: inputs.asset.demo_registration_route.as_ref(),
                legacy_demo: inputs.asset.legacy_demo_registration_route.as_ref(),
                character_face: inputs.asset.character_face_registration_route.as_ref(),
            },
        )?;
        if let StylizedArtworkConsumerValidation::MenuPanel(panel) = &consumer {
            ensure!(
                pattern_records.insert(panel.pattern_record_index),
                "stylized artwork set reuses a menu-panel pattern record"
            );
        }
        if let StylizedArtworkConsumerValidation::ModeSelectionStates(states) = &consumer {
            ensure!(
                mode_state_pattern_tables.insert((
                    states.pattern_pointer_table_index,
                    states.pattern_table_symbol.clone(),
                )),
                "stylized artwork set reuses a mode-selection state pattern table"
            );
            validate_source_mask_footprints(
                &region.source_pixel_mask,
                width,
                height,
                &states.state_source_mask_bounds,
            )?;
        }
        let resolved_palette_roles = resolve_source_palette_roles(SourcePaletteRoleInputs {
            palette_selection: member.source_palette_selection,
            declared_palette_roles: &member.source_palette_roles,
            source_mask_asset: &region.source_pixel_mask,
            artwork_composition_bounds: region.artwork_composition_bounds,
            maximum_composition_colors: inputs.asset.maximum_source_palette_colors_per_member,
            source_indices: &source_indices,
            palette: &palette,
            surface_dimensions: (width, height),
        })
        .with_context(|| {
            format!(
                "failed to resolve source palette roles for {}",
                member.region_id
            )
        })?;
        let master_asset = inputs
            .asset
            .shared_artwork_sheet
            .as_ref()
            .or(member.transparent_master.as_ref())
            .context("stylized artwork set member has no generation master")?;
        verify_hashed_asset(master_asset)?;
        let mut master = read_rgba_surface(Path::new(&master_asset.path))?;
        let removed_master_background_pixels =
            if let Some(removal) = member.master_background_removal {
                let removed = match removal.method {
                    StylizedArtworkMasterBackgroundRemovalMethod::BorderConnectedLightNeutral => {
                        clear_border_connected_light_neutral(
                            &mut master.pixels,
                            master.width,
                            master.height,
                            removal.minimum_channel,
                            removal.maximum_channel_spread,
                        )?
                    }
                };
                ensure!(
                    removed > 0,
                    "stylized artwork master background removal selected no pixels"
                );
                removed
            } else {
                0
            };
        let variant_master_crops = if member.additional_variant_master_crops.is_empty() {
            vec![member.master_crop; resized_native_placements.len()]
        } else {
            ensure!(
                member.additional_variant_master_crops.len() + 1 == resized_native_placements.len(),
                "stylized artwork member master crops do not match its native variants"
            );
            std::iter::once(member.master_crop)
                .chain(member.additional_variant_master_crops.iter().copied())
                .collect::<Vec<_>>()
        };
        let mut native_variants = Vec::with_capacity(resized_native_placements.len());
        let mut native_artwork = None;
        let mut palette_roles = Vec::<StylizedArtworkPaletteRoleUsage>::new();
        for (variant_index, (placement_bounds, master_crop)) in resized_native_placements
            .iter()
            .copied()
            .zip(variant_master_crops)
            .enumerate()
        {
            let (mut native_pixels, variant_palette_roles) = render_native_artwork(
                &master.pixels,
                master.width,
                master.height,
                master_crop,
                placement_bounds.width,
                placement_bounds.height,
                member.alpha_threshold,
                &resolved_palette_roles,
                member.source_palette_selection
                    == StylizedArtworkPaletteSelection::ExactDeclaredRoles,
                inputs.asset.master_layout == StylizedArtworkSetMasterLayout::IsolatedMemberArtwork
                    && member.additional_variant_master_crops.is_empty()
                    && region.artwork_composition_bounds.is_none(),
            )
            .with_context(|| {
                format!(
                    "failed to render stylized artwork set member {} state {}",
                    member.region_id, variant_index
                )
            })?;
            if region.artwork_composition_bounds.is_some() && !region.artwork_uses_overlay_alpha {
                clear_native_pixels_where_source_is_transparent(
                    &mut native_pixels,
                    placement_bounds,
                    &source_indices,
                    &palette,
                    width,
                    height,
                )?;
            }
            paste_native_artwork_clipped(
                &mut overlay,
                &mut claimed_overlay,
                width,
                height,
                &native_pixels,
                placement_bounds.width,
                placement_bounds.height,
                StylizedArtworkSignedPoint {
                    x: i32::try_from(placement_bounds.x)?,
                    y: i32::try_from(placement_bounds.y)?,
                },
                render_clip_bounds,
            )?;
            if variant_index == 0 {
                for placement in &member.additional_native_placements {
                    paste_native_artwork_clipped(
                        &mut overlay,
                        &mut claimed_overlay,
                        width,
                        height,
                        &native_pixels,
                        placement_bounds.width,
                        placement_bounds.height,
                        *placement,
                        render_clip_bounds,
                    )?;
                }
            }
            for usage in variant_palette_roles {
                if let Some(existing) = palette_roles.iter_mut().find(|existing| {
                    existing.role == usage.role
                        && existing.palette_index == usage.palette_index
                        && existing.rgba == usage.rgba
                }) {
                    existing.native_pixels += usage.native_pixels;
                } else {
                    palette_roles.push(usage);
                }
            }
            let native_preview_path = if variant_index == 0 {
                inputs
                    .native_preview_directory
                    .join(format!("{}.png", member.region_id))
            } else {
                inputs
                    .native_preview_directory
                    .join(format!("{}-state-{variant_index}.png", member.region_id))
            };
            write_rgba_png(
                &native_preview_path,
                placement_bounds.width,
                placement_bounds.height,
                &native_pixels,
            )?;
            let artwork = hashed_output(&native_preview_path)?;
            native_artwork.get_or_insert_with(|| artwork.clone());
            native_variants.push(StylizedArtworkNativeVariantReport {
                master_crop,
                placement: placement_bounds,
                artwork,
            });
        }
        member_reports.push(StylizedArtworkSetMemberReport {
            region_id: member.region_id.clone(),
            source_text: member.expected_source_text.clone(),
            korean_text: member.expected_korean_text.clone(),
            generation_master: master_asset.clone(),
            master_background_removal: member.master_background_removal,
            removed_master_background_pixels,
            master_crop: member.master_crop,
            additional_variant_master_crops: member.additional_variant_master_crops.clone(),
            native_placement: member.native_placement,
            artwork_composition_bounds: region.artwork_composition_bounds,
            render_clip_bounds: member.render_clip_bounds,
            additional_native_placements: member.additional_native_placements.clone(),
            native_artwork: native_artwork
                .context("stylized artwork member has no native state")?,
            native_variants,
            palette_roles,
            consumer,
        });
        selected_regions.push(region);
    }
    ensure!(
        pattern_records.is_empty() || pattern_records.len() == inputs.asset.members.len(),
        "stylized artwork menu-panel set has incomplete pattern coverage"
    );
    ensure!(
        mode_state_pattern_tables.len()
            == inputs
                .asset
                .members
                .iter()
                .filter(|member| {
                    matches!(
                        member.consumer,
                        StylizedArtworkConsumer::ModeSelectionStates(_)
                    )
                })
                .count(),
        "stylized artwork mode-selection set has incomplete pattern-table coverage"
    );

    let composition = compose_static_ui_indices(StaticUiIndexCompositionInputs {
        source_indices: &source_indices,
        palette: &palette,
        width,
        height,
        regions: &selected_regions,
        protected_english_mask: target.authoring.protected_english_mask.as_ref(),
        overlay: &overlay,
    })?;
    ensure!(
        composition.changed_protected_indices == 0
            && composition.changed_protected_english_indices == 0,
        "stylized artwork set composition changed protected pixels"
    );
    let replacement_rgba = colorize_psmt8(&composition.replacement_indices, &palette);
    write_rgba_png(
        inputs.surface_preview_path,
        image_record.width,
        image_record.height,
        &replacement_rgba,
    )?;
    let surface_preview = hashed_output(inputs.surface_preview_path)?;

    let staging = tempfile::tempdir().context("failed to stage stylized artwork set overlay")?;
    let staged_overlay = staging.path().join("overlay.png");
    write_rgba_png(
        &staged_overlay,
        image_record.width,
        image_record.height,
        &overlay,
    )?;
    let overlay_sha256 = sha256(
        &fs::read(&staged_overlay)
            .with_context(|| format!("failed to hash {}", staged_overlay.display()))?,
    );
    let overlay_path = Path::new(&inputs.asset.replacement_overlay_path);
    let existing_combined_overlay = target
        .authoring
        .replacement_overlay
        .as_ref()
        .filter(|existing| existing.path == inputs.asset.replacement_overlay_path);
    install_overlay(
        &staged_overlay,
        overlay_path,
        &overlay_sha256,
        existing_combined_overlay,
        inputs.refresh_existing,
    )?;
    let replacement_overlay = HashedAsset {
        path: inputs.asset.replacement_overlay_path.clone(),
        sha256: overlay_sha256,
    };

    let target = &mut inputs.graphics.entries[target_index];
    for member in &inputs.asset.members {
        target
            .authoring
            .regions
            .iter_mut()
            .find(|region| region.region_id == member.region_id)
            .expect("stylized artwork set region was validated")
            .artwork_plan
            .as_mut()
            .expect("stylized artwork set plan was validated")
            .placement_status = member.consumer.adopted_placement_status();
    }
    ensure!(
        target.authoring.regions.iter().all(|region| {
            region
                .artwork_plan
                .is_none_or(|plan| plan.placement_status.is_buildable())
        }),
        "stylized artwork surface still contains a rough region and cannot enter human review"
    );
    target.authoring.replacement_overlay = Some(replacement_overlay.clone());
    target.authoring.status = GraphicsTranslationStatus::NeedsHumanReview;

    Ok(StylizedArtworkSetDraftReport {
        schema: 1,
        candidate_id: inputs.asset.candidate_id.clone(),
        source_bin_sha256: inputs.graphics.source.source_bin_sha256.clone(),
        executable_sha256: inputs.graphics.source.executable_sha256.clone(),
        source_surface_id: inputs.asset.source_surface_id.clone(),
        artwork_method: inputs.asset.artwork_method,
        source_background_restoration: inputs.asset.source_background_restoration,
        master_layout: inputs.asset.master_layout,
        maximum_source_palette_colors_per_member: inputs
            .asset
            .maximum_source_palette_colors_per_member,
        source_preview: HashedAsset {
            path: source_preview.png_path.clone(),
            sha256: source_preview.png_sha256.clone(),
        },
        base_replacement_overlay: inputs.asset.base_replacement_overlay.clone(),
        base_region_ids,
        sequential_registration,
        indexed_registration,
        demo_registration,
        legacy_demo_registration,
        character_face_registration,
        members: member_reports,
        replacement_overlay,
        surface_preview,
        source_japanese_pixels: composition.source_japanese_pixels,
        korean_overlay_pixels: composition.korean_overlay_pixels,
        changed_palette_indices: composition.changed_indices,
        protected_pixels: composition.protected_pixels,
        changed_protected_indices: composition.changed_protected_indices,
        changed_protected_english_indices: composition.changed_protected_english_indices,
        status: "needs_human_review",
        release_candidate_eligible: false,
        evidence_boundary: inputs.asset.evidence_boundary.clone(),
    })
}

type StylizedSetOverlayState = (
    Vec<u8>,
    Vec<bool>,
    Vec<String>,
    Vec<super::GraphicsTextRegion>,
);

pub(super) fn enclosing_pixel_bounds(bounds: &[PixelBounds]) -> Result<PixelBounds> {
    let first = bounds
        .first()
        .copied()
        .context("cannot enclose an empty pixel-bounds set")?;
    let left = bounds
        .iter()
        .map(|bounds| bounds.x)
        .min()
        .unwrap_or(first.x);
    let top = bounds
        .iter()
        .map(|bounds| bounds.y)
        .min()
        .unwrap_or(first.y);
    let right = bounds.iter().try_fold(
        first
            .x
            .checked_add(first.width)
            .context("pixel-bounds right edge overflow")?,
        |right, bounds| {
            Ok::<_, anyhow::Error>(
                right.max(
                    bounds
                        .x
                        .checked_add(bounds.width)
                        .context("pixel-bounds right edge overflow")?,
                ),
            )
        },
    )?;
    let bottom = bounds.iter().try_fold(
        first
            .y
            .checked_add(first.height)
            .context("pixel-bounds bottom edge overflow")?,
        |bottom, bounds| {
            Ok::<_, anyhow::Error>(
                bottom.max(
                    bounds
                        .y
                        .checked_add(bounds.height)
                        .context("pixel-bounds bottom edge overflow")?,
                ),
            )
        },
    )?;
    Ok(PixelBounds {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

pub(super) fn initialize_stylized_set_overlay(
    asset: &StylizedArtworkSetAsset,
    target: &super::GraphicsTranslationEntry,
    width: usize,
    height: usize,
) -> Result<StylizedSetOverlayState> {
    let Some(base_asset) = &asset.base_replacement_overlay else {
        return Ok((
            vec![0_u8; width * height * 4],
            vec![false; width * height],
            Vec::new(),
            Vec::new(),
        ));
    };
    ensure!(
        base_asset.path != asset.replacement_overlay_path,
        "stylized artwork base and combined overlay paths must differ"
    );
    verify_hashed_asset(base_asset)?;
    let base = read_rgba_surface(Path::new(&base_asset.path))?;
    ensure!(
        base.width == width && base.height == height,
        "stylized artwork base overlay dimensions differ from the source surface"
    );
    ensure!(
        target
            .authoring
            .replacement_overlay
            .as_ref()
            .is_some_and(|current| {
                current == base_asset || current.path == asset.replacement_overlay_path
            }),
        "stylized artwork base overlay is neither the current ledger input nor an adopted combined output"
    );

    let base_regions = target
        .authoring
        .regions
        .iter()
        .filter(|region| {
            region.korean_text.is_some()
                && region.treatment_profile != GraphicsTreatmentProfile::StylizedDisplayLettering
        })
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        !base_regions.is_empty(),
        "stylized artwork declares a base overlay without translated base regions"
    );
    let mut claimed = vec![false; width * height];
    for (index, pixel) in base.pixels.chunks_exact(4).enumerate() {
        if pixel[3] == 0 {
            continue;
        }
        let x = u32::try_from(index % width)?;
        let y = u32::try_from(index / width)?;
        ensure!(
            base_regions.iter().any(|region| {
                x >= region.bounds.x
                    && x < region.bounds.x + region.bounds.width
                    && y >= region.bounds.y
                    && y < region.bounds.y + region.bounds.height
            }),
            "stylized artwork base overlay has an opaque pixel outside its translated base regions"
        );
        claimed[index] = true;
    }
    ensure!(
        claimed.iter().any(|claimed| *claimed),
        "stylized artwork base overlay has no opaque pixels"
    );
    let base_region_ids = base_regions
        .iter()
        .map(|region| region.region_id.clone())
        .collect();
    Ok((base.pixels, claimed, base_region_ids, base_regions))
}

pub(super) fn validate_set_asset_header(asset: &StylizedArtworkSetAsset) -> Result<()> {
    ensure!(
        asset.schema == 1
            && !asset.candidate_id.trim().is_empty()
            && !asset.source_surface_id.trim().is_empty()
            && !asset.tool_path.trim().is_empty()
            && !asset.members.is_empty()
            && !asset.evidence_boundary.trim().is_empty()
            && asset.review_status == StylizedArtworkReviewStatus::PreHumanReview,
        "stylized artwork set manifest is incomplete"
    );
    let mut region_ids = BTreeSet::new();
    ensure!(
        asset
            .maximum_source_palette_colors_per_member
            .is_none_or(|maximum| (2..=256).contains(&maximum)),
        "stylized artwork set palette limit is outside 2..=256"
    );
    for member in &asset.members {
        ensure!(
            !member.region_id.trim().is_empty()
                && region_ids.insert(member.region_id.as_str())
                && !member.expected_source_text.trim().is_empty()
                && !member.expected_korean_text.trim().is_empty()
                && member.alpha_threshold > 0
                && member.alpha_threshold < u8::MAX
                && member.native_placement.width > 0
                && member.native_placement.height > 0
                && member.master_crop.width > 0
                && member.master_crop.height > 0
                && member
                    .additional_variant_master_crops
                    .iter()
                    .all(|crop| crop.width > 0 && crop.height > 0),
            "stylized artwork set member is incomplete or duplicated"
        );
        if let Some(removal) = member.master_background_removal {
            ensure!(
                removal.minimum_channel >= 128 && removal.maximum_channel_spread <= 64,
                "stylized artwork light-neutral background threshold is not bounded"
            );
        }
    }
    match asset.master_layout {
        StylizedArtworkSetMasterLayout::IsolatedMemberArtwork => ensure!(
            asset.shared_generation_prompt.is_none()
                && asset.shared_artwork_sheet.is_none()
                && asset.members.iter().all(|member| {
                    member
                        .generation_prompt
                        .as_deref()
                        .is_some_and(|prompt| !prompt.trim().is_empty())
                        && member.transparent_master.is_some()
                }),
            "isolated artwork members must each own one prompt and transparent master"
        ),
        StylizedArtworkSetMasterLayout::SharedArtworkSheet => ensure!(
            asset.members.len() > 1
                && asset
                    .shared_generation_prompt
                    .as_deref()
                    .is_some_and(|prompt| !prompt.trim().is_empty())
                && asset.shared_artwork_sheet.is_some()
                && asset.members.iter().all(|member| {
                    member.generation_prompt.is_none() && member.transparent_master.is_none()
                }),
            "shared artwork sheet must own the set prompt and exact master"
        ),
    }
    let battle_member_count = asset
        .members
        .iter()
        .filter(|member| matches!(member.consumer, StylizedArtworkConsumer::BattleTexture(_)))
        .count();
    let battle_registration_route_count =
        usize::from(asset.sequential_registration_route.is_some())
            + usize::from(asset.indexed_registration_route.is_some())
            + usize::from(asset.character_face_registration_route.is_some());
    ensure!(
        (battle_member_count == 0 && battle_registration_route_count == 0)
            || (battle_member_count == asset.members.len() && battle_registration_route_count == 1),
        "stylized artwork set must apply one registration route to every battle texture member"
    );
    let demo_member_count = asset
        .members
        .iter()
        .filter(|member| {
            matches!(
                member.consumer,
                StylizedArtworkConsumer::SequencePattern(_)
                    | StylizedArtworkConsumer::SourcePattern(_)
            )
        })
        .count();
    let source_pattern_member_count = asset
        .members
        .iter()
        .filter(|member| matches!(member.consumer, StylizedArtworkConsumer::SourcePattern(_)))
        .count();
    ensure!(
        asset.demo_registration_route.as_ref().map_or_else(
            || source_pattern_member_count == 0,
            |_| {
                demo_member_count == asset.members.len() && battle_registration_route_count == 0
            }
        ),
        "stylized artwork demo registration route does not cover the exact demo member set"
    );
    let legacy_demo_member_count = asset
        .members
        .iter()
        .filter(|member| {
            matches!(
                member.consumer,
                StylizedArtworkConsumer::SequencePattern(_)
                    | StylizedArtworkConsumer::AnimatedPattern(_)
                    | StylizedArtworkConsumer::ScriptedAnimation(_)
                    | StylizedArtworkConsumer::UnreferencedSource(_)
            )
        })
        .count();
    ensure!(
        asset.legacy_demo_registration_route.as_ref().map_or_else(
            || true,
            |_| {
                asset.demo_registration_route.is_none()
                    && legacy_demo_member_count == asset.members.len()
                    && battle_registration_route_count == 0
            }
        ),
        "stylized artwork legacy demo registration route does not cover the exact legacy demo member set"
    );
    let output = Path::new(&asset.replacement_overlay_path);
    ensure!(
        asset
            .replacement_overlay_path
            .starts_with("assets/translation/graphics/")
            && output
                .components()
                .all(|component| component != Component::ParentDir),
        "stylized artwork set overlay path leaves the tracked graphics asset boundary"
    );
    Ok(())
}

pub(super) fn validate_asset_header(asset: &StylizedArtworkAsset) -> Result<()> {
    ensure!(
        asset.schema == 1
            && !asset.candidate_id.trim().is_empty()
            && !asset.expected_source_text.trim().is_empty()
            && !asset.expected_korean_text.trim().is_empty()
            && !asset.tool_path.trim().is_empty()
            && !asset.generation_prompt.trim().is_empty()
            && !asset.evidence_boundary.trim().is_empty()
            && asset.alpha_threshold > 0
            && asset.alpha_threshold < u8::MAX
            && asset.review_status == StylizedArtworkReviewStatus::PreHumanReview,
        "stylized artwork manifest is incomplete"
    );
    let output = Path::new(&asset.replacement_overlay_path);
    ensure!(
        asset
            .replacement_overlay_path
            .starts_with("assets/translation/graphics/")
            && output
                .components()
                .all(|component| component != Component::ParentDir),
        "stylized artwork overlay path leaves the tracked graphics asset boundary"
    );
    ensure!(
        asset.native_placement.width > 0
            && asset.native_placement.height > 0
            && asset.master_crop.width > 0
            && asset.master_crop.height > 0,
        "stylized artwork crop or placement is empty"
    );
    ensure!(
        asset.demo_registration_route.is_none()
            || matches!(
                asset.consumer,
                StylizedArtworkConsumer::SequencePattern(_)
                    | StylizedArtworkConsumer::SourcePattern(_)
            ),
        "single stylized artwork demo route does not cover a demo consumer"
    );
    Ok(())
}

pub(super) fn find_source_preview<'a>(
    previews: &'a TexturePreviewCorpus,
    target: &super::GraphicsTranslationEntry,
    declaration: &super::static_regions::StaticSurfaceDeclaration,
    asset: &StylizedArtworkAsset,
) -> Result<&'a TexturePreview> {
    find_source_preview_for_consumer(previews, target, declaration, &asset.consumer)
}

pub(super) fn find_source_preview_for_consumer<'a>(
    previews: &'a TexturePreviewCorpus,
    target: &super::GraphicsTranslationEntry,
    declaration: &super::static_regions::StaticSurfaceDeclaration,
    consumer: &StylizedArtworkConsumer,
) -> Result<&'a TexturePreview> {
    let primary_load_route = consumer.primary_load_route()?;
    let matches = previews
        .previews
        .iter()
        .filter(|preview| {
            preview.image_container_iso_path == target.source.container_iso_path
                && preview.image_file_record == target.source.image_file_record
                && preview.palette_file_record == primary_load_route.palette_file_record
                && preview.width == target.source.width
                && preview.height == target.source.height
                && preview.linear_indices_sha256 == target.source.linear_indices_sha256
                && preview.png_sha256 == declaration.source_png_sha256
                && preview
                    .owner_tables
                    .iter()
                    .any(|owner| consumer.owns_load_table(owner))
        })
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "stylized artwork has no unique exact source preview"
    );
    let preview = matches[0];
    ensure!(
        sha256(
            &fs::read(&preview.png_path)
                .with_context(|| format!("failed to hash {}", preview.png_path))?
        ) == preview.png_sha256,
        "stylized artwork source preview PNG hash changed"
    );
    Ok(preview)
}
