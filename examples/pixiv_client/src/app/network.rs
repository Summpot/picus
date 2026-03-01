use super::*;

fn summarize_error(details: &str) -> String {
    let first = details
        .lines()
        .next()
        .unwrap_or("network request failed")
        .trim();
    let mut summary = first.to_string();
    if summary.len() > 140 {
        summary.truncate(140);
        summary.push('…');
    }
    summary
}

pub(super) fn spawn_network_tasks(world: &mut World) {
    let cmd_rx = world.resource::<NetworkBridge>().cmd_rx.clone();
    let result_tx = world.resource::<NetworkBridge>().result_tx.clone();
    let client = world.resource::<PixivApiClient>().clone();
    let auth = world.resource::<AuthState>().clone();

    while let Ok(cmd) = cmd_rx.try_recv() {
        let client = client.clone();
        let auth = auth.clone();
        let result_tx = result_tx.clone();

        AsyncComputeTaskPool::get()
            .spawn(async move {
                let result = match run_network_command(&client, &auth, cmd) {
                    Ok(r) => r,
                    Err(err) => {
                        let details = err.to_string();
                        let summary = summarize_error(&details);
                        NetworkResult::Error { summary, details }
                    }
                };
                let _ = result_tx.send(result);
            })
            .detach();
    }
}

fn run_network_command(
    client: &PixivApiClient,
    auth: &AuthState,
    cmd: NetworkCommand,
) -> Result<NetworkResult> {
    match cmd {
        NetworkCommand::DiscoverIdp => {
            let idp = client.discover_idp_urls()?;
            Ok(NetworkResult::IdpDiscovered(idp))
        }
        NetworkCommand::ExchangeCode {
            code,
            code_verifier,
        } => {
            let idp = auth.idp_urls.as_ref();
            let auth_token_url = idp
                .map(|value| value.auth_token_url.as_str())
                .unwrap_or(PIXIV_AUTH_TOKEN_FALLBACK);
            let redirect_uri = idp
                .map(|value| value.auth_token_redirect_url.as_str())
                .unwrap_or(PIXIV_WEB_REDIRECT_FALLBACK);
            let response = client.exchange_authorization_code(
                auth_token_url,
                &code_verifier,
                &code,
                redirect_uri,
            )?;
            Ok(NetworkResult::Authenticated(response.into()))
        }
        NetworkCommand::Refresh { refresh_token } => {
            let auth_token_url = auth
                .idp_urls
                .as_ref()
                .map(|value| value.auth_token_url.as_str())
                .unwrap_or(PIXIV_AUTH_TOKEN_FALLBACK);
            let response = client.refresh_access_token(auth_token_url, &refresh_token)?;
            Ok(NetworkResult::Authenticated(response.into()))
        }
        NetworkCommand::FetchHome => {
            let token = auth
                .session
                .as_ref()
                .map(|s| s.access_token.clone())
                .ok_or_else(|| anyhow::anyhow!("not authenticated"))?;
            let payload = client.recommended_illusts(&token)?;
            Ok(NetworkResult::FeedLoaded {
                source: NavTab::Home,
                payload,
            })
        }
        NetworkCommand::FetchRanking => {
            let token = auth
                .session
                .as_ref()
                .map(|s| s.access_token.clone())
                .ok_or_else(|| anyhow::anyhow!("not authenticated"))?;
            let payload = client.ranking_illusts(&token, "day")?;
            Ok(NetworkResult::FeedLoaded {
                source: NavTab::Rankings,
                payload,
            })
        }
        NetworkCommand::Search { word } => {
            let token = auth
                .session
                .as_ref()
                .map(|s| s.access_token.clone())
                .ok_or_else(|| anyhow::anyhow!("not authenticated"))?;
            let payload = client.search_illusts(&token, &word)?;
            Ok(NetworkResult::FeedLoaded {
                source: NavTab::Search,
                payload,
            })
        }
        NetworkCommand::Bookmark { illust_id } => {
            let token = auth
                .session
                .as_ref()
                .map(|s| s.access_token.clone())
                .ok_or_else(|| anyhow::anyhow!("not authenticated"))?;
            client.bookmark_illust(&token, illust_id)?;
            Ok(NetworkResult::BookmarkDone { illust_id })
        }
    }
}

pub(super) fn apply_network_results(world: &mut World) {
    let result_rx = world.resource::<NetworkBridge>().result_rx.clone();
    let image_cmd_tx = world.resource::<ImageBridge>().cmd_tx.clone();

    while let Ok(result) = result_rx.try_recv() {
        match result {
            NetworkResult::IdpDiscovered(idp) => {
                world.resource_mut::<AuthState>().idp_urls = Some(idp);
                set_status_key(
                    world,
                    "pixiv.status.idp_discovered",
                    "IdP endpoint discovered. Enter auth_code or refresh token.",
                );
            }
            NetworkResult::Authenticated(session) => {
                world.resource_mut::<AuthState>().session = Some(session.clone());
                set_status_key(
                    world,
                    "pixiv.status.authenticated_loading_home",
                    "Authenticated. Loading home feed…",
                );
                *world.resource_mut::<ResponsePanelState>() = ResponsePanelState::default();

                if world.resource::<AuthState>().refresh_token_input.is_empty() {
                    world.resource_mut::<AuthState>().refresh_token_input =
                        session.refresh_token.clone();
                }

                let _ = world
                    .resource::<NetworkBridge>()
                    .cmd_tx
                    .send(NetworkCommand::FetchHome);
            }
            NetworkResult::FeedLoaded { source, payload } => {
                let home_feed = world.resource::<PixivUiTree>().home_feed;
                world.resource_mut::<UiState>().active_tab = source;
                let message = format!(
                    "{} {} ({source:?})",
                    tr(
                        world,
                        "pixiv.status.loaded_illustrations",
                        "Loaded illustrations",
                    ),
                    payload.illusts.len()
                );
                set_status(world, message);

                for entity in std::mem::take(&mut world.resource_mut::<FeedOrder>().0) {
                    if world.get_entity(entity).is_ok() {
                        world.entity_mut(entity).despawn();
                    }
                }

                let mut new_order = Vec::new();
                for illust in payload.illusts {
                    let open_thumbnail = world.spawn_empty().id();
                    let bookmark = world.spawn_empty().id();
                    let entity = world
                        .spawn((
                            PixivIllustCard,
                            illust.clone(),
                            IllustVisual::default(),
                            CardAnimState::default(),
                            IllustActionEntities {
                                open_thumbnail,
                                bookmark,
                            },
                            StyleClass(vec!["pixiv.card".to_string()]),
                            ChildOf(home_feed),
                        ))
                        .id();

                    let _ = image_cmd_tx.send(ImageCommand::Download {
                        entity,
                        kind: ImageKind::Thumb,
                        url: illust.image_urls.square_medium.clone(),
                    });
                    let _ = image_cmd_tx.send(ImageCommand::Download {
                        entity,
                        kind: ImageKind::Avatar,
                        url: illust.user.profile_image_urls.medium.clone(),
                    });

                    new_order.push(entity);
                }

                world.resource_mut::<FeedOrder>().0 = new_order;
            }
            NetworkResult::BookmarkDone { illust_id } => {
                set_status(
                    world,
                    format!(
                        "{} #{illust_id}",
                        tr(
                            world,
                            "pixiv.status.bookmark_synced",
                            "Bookmark synced for illust",
                        )
                    ),
                );
            }
            NetworkResult::Error { summary, details } => {
                let status_message = format!(
                    "{}: {summary}",
                    tr(world, "pixiv.status.network_error", "Network error")
                );
                set_status(world, status_message);
                *world.resource_mut::<ResponsePanelState>() = ResponsePanelState {
                    title: tr(
                        world,
                        "pixiv.status.response_detail_title",
                        "Last network response body / detail",
                    ),
                    content: details,
                };
            }
        }
    }
}

pub(super) fn spawn_image_tasks(world: &mut World) {
    let cmd_rx = world.resource::<ImageBridge>().cmd_rx.clone();
    let result_tx = world.resource::<ImageBridge>().result_tx.clone();
    let client = world.resource::<PixivApiClient>().clone();

    while let Ok(cmd) = cmd_rx.try_recv() {
        let client = client.clone();
        let result_tx = result_tx.clone();

        AsyncComputeTaskPool::get()
            .spawn(async move {
                let result = match cmd {
                    ImageCommand::Download { entity, kind, url } => {
                        match client.download_image_rgba8(&url) {
                            Ok(decoded) => ImageResult::Loaded {
                                entity,
                                kind,
                                decoded,
                            },
                            Err(err) => ImageResult::Failed {
                                entity,
                                kind,
                                error: err.to_string(),
                            },
                        }
                    }
                };

                let _ = result_tx.send(result);
            })
            .detach();
    }
}

pub(super) fn apply_image_results(world: &mut World) {
    let result_rx = world.resource::<ImageBridge>().result_rx.clone();

    while let Ok(result) = result_rx.try_recv() {
        match result {
            ImageResult::Loaded {
                entity,
                kind,
                decoded,
            } => {
                if world.get_entity(entity).is_err() {
                    continue;
                }

                let DecodedImageRgba {
                    width,
                    height,
                    rgba8,
                } = decoded;

                let ui_data = ImageData {
                    data: Blob::new(Arc::new(rgba8.clone())),
                    format: ImageFormat::Rgba8,
                    alpha_type: ImageAlphaType::Alpha,
                    width,
                    height,
                };

                let Some(rgba_image) = image::RgbaImage::from_raw(width, height, rgba8) else {
                    set_status(
                        world,
                        format!(
                            "{} {entity:?}",
                            tr(
                                world,
                                "pixiv.status.image_decode_buffer_mismatch",
                                "Image decode buffer size mismatch for entity",
                            )
                        ),
                    );
                    continue;
                };
                let bevy_image = BevyImage::from_dynamic(
                    image::DynamicImage::ImageRgba8(rgba_image),
                    true,
                    RenderAssetUsages::default(),
                );

                let handle = world.resource_mut::<Assets<BevyImage>>().add(bevy_image);

                let mut visual = world
                    .get::<IllustVisual>(entity)
                    .cloned()
                    .unwrap_or_default();
                match kind {
                    ImageKind::Thumb => {
                        visual.thumb_ui = Some(ui_data);
                        visual.thumb_handle = Some(handle);
                    }
                    ImageKind::Avatar => {
                        visual.avatar_ui = Some(ui_data);
                        visual.avatar_handle = Some(handle);
                    }
                    ImageKind::HighRes => {
                        visual.high_res_ui = Some(ui_data);
                        visual.high_res_handle = Some(handle);
                    }
                }

                world.entity_mut(entity).insert(visual);
            }
            ImageResult::Failed {
                entity,
                kind,
                error,
            } => {
                let which = match kind {
                    ImageKind::Thumb => "thumb",
                    ImageKind::Avatar => "avatar",
                    ImageKind::HighRes => "high-res",
                };
                if world.get_entity(entity).is_ok() {
                    set_status(
                        world,
                        format!(
                            "{} ({which}): {error}",
                            tr(world, "pixiv.status.image_load_failed", "Image load failed")
                        ),
                    );
                }
            }
        }
    }
}
