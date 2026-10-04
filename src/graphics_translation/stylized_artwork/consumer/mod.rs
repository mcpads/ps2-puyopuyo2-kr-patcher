use super::composition::{derive_screen_bounds, ensure_bounds_contains};
use super::*;

mod battle;
mod demo;
mod layout;
mod pattern;
mod registration;
mod scripted_animation;
mod sequence;

pub(super) use battle::*;
pub(super) use demo::*;
pub(super) use layout::*;
pub(super) use pattern::*;
pub(super) use registration::*;
pub(super) use scripted_animation::*;
pub(super) use sequence::*;

pub(super) fn validate_consumer(
    file: &object::File<'_>,
    executable: &[u8],
    asset: &StylizedArtworkAsset,
    region: &super::GraphicsTextRegion,
) -> Result<StylizedArtworkConsumerValidation> {
    validate_consumer_for_placement(
        file,
        executable,
        &asset.consumer,
        asset.native_placement,
        region,
        StylizedArtworkRegistrationRoutes {
            demo: asset.demo_registration_route.as_ref(),
            ..StylizedArtworkRegistrationRoutes::default()
        },
    )
}

#[derive(Clone, Copy, Default)]
pub(super) struct StylizedArtworkRegistrationRoutes<'a> {
    pub(super) sequential: Option<&'a StylizedArtworkSequentialRegistrationRoute>,
    pub(super) indexed: Option<&'a StylizedArtworkIndexedRegistrationRoute>,
    pub(super) demo: Option<&'a StylizedArtworkDemoRegistrationRoute>,
    pub(super) legacy_demo: Option<&'a StylizedArtworkLegacyDemoRegistrationRoute>,
    pub(super) character_face: Option<&'a StylizedArtworkCharacterFaceRegistrationRoute>,
}

pub(super) fn validate_consumer_for_placement(
    file: &object::File<'_>,
    executable: &[u8],
    consumer: &StylizedArtworkConsumer,
    native_placement: PixelBounds,
    region: &super::GraphicsTextRegion,
    registration_routes: StylizedArtworkRegistrationRoutes<'_>,
) -> Result<StylizedArtworkConsumerValidation> {
    match consumer {
        StylizedArtworkConsumer::SequencePattern(consumer) => Ok(
            StylizedArtworkConsumerValidation::SequencePattern(validate_sequence_consumer(
                file,
                executable,
                native_placement,
                consumer,
                region,
                registration_routes.demo,
                registration_routes.legacy_demo,
            )?),
        ),
        StylizedArtworkConsumer::AnimatedPattern(consumer) => {
            let legacy_demo_route = registration_routes.legacy_demo.context(
                "animated pattern consumer has no shared legacy demo registration route",
            )?;
            Ok(StylizedArtworkConsumerValidation::AnimatedPattern(
                validate_animated_pattern_consumer(
                    file,
                    executable,
                    native_placement,
                    consumer,
                    region,
                    legacy_demo_route,
                )?,
            ))
        }
        StylizedArtworkConsumer::ScriptedAnimation(consumer) => {
            let legacy_demo_route = registration_routes.legacy_demo.context(
                "scripted-animation consumer has no shared legacy demo registration route",
            )?;
            Ok(StylizedArtworkConsumerValidation::ScriptedAnimation(
                validate_scripted_animation_consumer(
                    file,
                    executable,
                    native_placement,
                    consumer,
                    region,
                    legacy_demo_route,
                )?,
            ))
        }
        StylizedArtworkConsumer::SourcePattern(consumer) => {
            let demo_route = registration_routes
                .demo
                .context("source pattern consumer has no shared demo registration route")?;
            Ok(StylizedArtworkConsumerValidation::SourcePattern(
                validate_source_pattern_consumer(file, consumer, region, demo_route)?,
            ))
        }
        StylizedArtworkConsumer::UnreferencedSource(consumer) => {
            let legacy_demo_route = registration_routes.legacy_demo.context(
                "unreferenced source consumer has no shared legacy demo registration route",
            )?;
            Ok(StylizedArtworkConsumerValidation::UnreferencedSource(
                validate_unreferenced_source_consumer(file, consumer, region, legacy_demo_route)?,
            ))
        }
        StylizedArtworkConsumer::DirectPatternDisplay(consumer) => {
            Ok(StylizedArtworkConsumerValidation::DirectPatternDisplay(
                validate_direct_pattern_display_consumer(
                    file,
                    executable,
                    native_placement,
                    consumer,
                    region,
                )?,
            ))
        }
        StylizedArtworkConsumer::IndexedPatternDisplay(consumer) => {
            Ok(StylizedArtworkConsumerValidation::IndexedPatternDisplay(
                validate_indexed_pattern_display_consumer(
                    file,
                    executable,
                    native_placement,
                    consumer,
                    region,
                )?,
            ))
        }
        StylizedArtworkConsumer::ScrollingTile(consumer) => {
            Ok(StylizedArtworkConsumerValidation::ScrollingTile(
                validate_scrolling_tile_consumer(file, executable, consumer, region)?,
            ))
        }
        StylizedArtworkConsumer::MenuPanel(consumer) => {
            Ok(StylizedArtworkConsumerValidation::MenuPanel(
                validate_menu_panel_consumer(file, executable, native_placement, consumer, region)?,
            ))
        }
        StylizedArtworkConsumer::ModeSelectionStates(consumer) => {
            Ok(StylizedArtworkConsumerValidation::ModeSelectionStates(
                validate_mode_selection_states_consumer(
                    file,
                    executable,
                    native_placement,
                    consumer,
                    region,
                )?,
            ))
        }
        StylizedArtworkConsumer::TitleBranding(consumer) => Ok(
            StylizedArtworkConsumerValidation::TitleBranding(validate_title_branding_consumer(
                file,
                executable,
                native_placement,
                consumer,
                region,
            )?),
        ),
        StylizedArtworkConsumer::BattleTexture(consumer) => {
            let registration_route = match (
                registration_routes.sequential,
                registration_routes.indexed,
                registration_routes.character_face,
            ) {
                (Some(route), None, None) => {
                    StylizedArtworkBattleRegistrationRouteRef::Sequential(route)
                }
                (None, Some(route), None) => {
                    StylizedArtworkBattleRegistrationRouteRef::Indexed(route)
                }
                (None, None, Some(route)) => {
                    StylizedArtworkBattleRegistrationRouteRef::CharacterFace(route)
                }
                _ => anyhow::bail!(
                    "battle texture consumer does not have exactly one shared registration route"
                ),
            };
            Ok(StylizedArtworkConsumerValidation::BattleTexture(
                validate_battle_texture_consumer(
                    file,
                    executable,
                    consumer,
                    region,
                    registration_route,
                )?,
            ))
        }
    }
}
