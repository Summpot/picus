use super::*;
use picus_core::{UiScrollView, bevy_math::Vec2};

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

fn is_downloadable_image_url(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("https://") || trimmed.starts_with("http://")
}

fn feed_generation_for_command(cmd: &NetworkCommand) -> Option<u64> {
    match cmd {
        NetworkCommand::FetchHome { generation }
        | NetworkCommand::FetchRanking { generation }
        | NetworkCommand::FetchManga { generation }
        | NetworkCommand::FetchNovels { generation } => Some(*generation),
        NetworkCommand::Search { generation, .. }
        | NetworkCommand::FetchNext { generation, .. } => Some(*generation),
        NetworkCommand::DiscoverIdp
        | NetworkCommand::ExchangeCode { .. }
        | NetworkCommand::Refresh { .. }
        | NetworkCommand::Bookmark { .. } => None,
    }
}

fn access_token(auth: &AuthState) -> Result<String> {
    auth.session
        .as_ref()
        .map(|session| session.access_token.clone())
        .ok_or_else(|| anyhow::anyhow!("not authenticated"))
}

fn fetch_next_payload(
    client: &PixivApiClient,
    token: &str,
    source: NavTab,
    url: &str,
) -> Result<PixivResponse> {
    match source {
        NavTab::Novels => client.fetch_novel_page(token, url),
        _ => client.fetch_page_json::<PixivResponse>(token, url),
    }
}

fn preferred_thumbnail_url(illust: &Illust) -> Option<String> {
    [
        illust.image_urls.medium.as_str(),
        illust.image_urls.large.as_str(),
        illust.image_urls.square_medium.as_str(),
    ]
    .into_iter()
    .find(|url| is_downloadable_image_url(url))
    .map(ToOwned::to_owned)
}

fn spawn_feed_card(
    world: &mut World,
    home_feed: Entity,
    image_cmd_tx: &Sender<ImageCommand>,
    illust: Illust,
) -> Entity {
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

    if let Some(url) = preferred_thumbnail_url(&illust) {
        let _ = image_cmd_tx.send(ImageCommand::Download {
            entity,
            kind: ImageKind::Thumb,
            url,
        });
    }
    if is_downloadable_image_url(&illust.user.profile_image_urls.medium) {
        let _ = image_cmd_tx.send(ImageCommand::Download {
            entity,
            kind: ImageKind::Avatar,
            url: illust.user.profile_image_urls.medium.clone(),
        });
    }

    entity
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
        let feed_generation = feed_generation_for_command(&cmd);

        AsyncComputeTaskPool::get()
            .spawn(async move {
                let result = match run_network_command(&client, &auth, cmd) {
                    Ok(result) => result,
                    Err(err) => {
                        let details = err.to_string();
                        let summary = summarize_error(&details);
                        NetworkResult::Error {
                            summary,
                            details,
                            feed_generation,
                        }
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
        NetworkCommand::FetchHome { generation } => {
            let token = access_token(auth)?;
            let payload = client.recommended_illusts(&token)?;
            Ok(NetworkResult::FeedLoaded {
                source: NavTab::Home,
                payload,
                generation,
                append: false,
            })
        }
        NetworkCommand::FetchRanking { generation } => {
            let token = access_token(auth)?;
            let payload = client.ranking_illusts(&token, "day")?;
            Ok(NetworkResult::FeedLoaded {
                source: NavTab::Rankings,
                payload,
                generation,
                append: false,
            })
        }
        NetworkCommand::FetchManga { generation } => {
            let token = access_token(auth)?;
            let payload = client.recommended_manga(&token)?;
            Ok(NetworkResult::FeedLoaded {
                source: NavTab::Manga,
                payload,
                generation,
                append: false,
            })
        }
        NetworkCommand::FetchNovels { generation } => {
            let token = access_token(auth)?;
            let payload = client.recommended_novels(&token)?;
            Ok(NetworkResult::FeedLoaded {
                source: NavTab::Novels,
                payload,
                generation,
                append: false,
            })
        }
        NetworkCommand::Search { word, generation } => {
            let token = access_token(auth)?;
            let payload = client.search_illusts(&token, &word)?;
            Ok(NetworkResult::FeedLoaded {
                source: NavTab::Search,
                payload,
                generation,
                append: false,
            })
        }
        NetworkCommand::FetchNext {
            source,
            generation,
            url,
        } => {
            let token = access_token(auth)?;
            let payload = fetch_next_payload(client, &token, source, &url)?;
            Ok(NetworkResult::FeedLoaded {
                source,
                payload,
                generation,
                append: true,
            })
        }
        NetworkCommand::Bookmark { illust_id } => {
            let token = access_token(auth)?;
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
                if let Err(error) = super::persistence::save_auth_session(&session) {
                    eprintln!("pixiv credential persist failed: {error}");
                }
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

                let generation = begin_feed_request(world);
                let _ = world
                    .resource::<NetworkBridge>()
                    .cmd_tx
                    .send(NetworkCommand::FetchHome { generation });
            }
            NetworkResult::FeedLoaded {
                source,
                payload,
                generation,
                append,
            } => {
                let current_generation = world.resource::<FeedPagination>().generation;
                if generation != current_generation {
                    continue;
                }

                let tree = *world.resource::<PixivUiTree>();
                world.resource_mut::<UiState>().active_tab = source;

                let mut next_order = if append {
                    std::mem::take(&mut world.resource_mut::<FeedOrder>().0)
                } else {
                    for entity in std::mem::take(&mut world.resource_mut::<FeedOrder>().0) {
                        if world.get_entity(entity).is_ok() {
                            world.entity_mut(entity).despawn();
                        }
                    }

                    if let Some(mut scroll_view) = world.get_mut::<UiScrollView>(tree.feed_scroll) {
                        scroll_view.scroll_offset = Vec2::ZERO;
                        scroll_view.clamp_scroll_offset();
                    }

                    Vec::new()
                };
                let mut seen_ids = if append {
                    std::mem::take(&mut world.resource_mut::<FeedSeenIds>().0)
                } else {
                    world.resource_mut::<FeedSeenIds>().0.clear();
                    std::mem::take(&mut world.resource_mut::<FeedSeenIds>().0)
                };

                let next_url = payload.next_url.clone();
                let mut added = 0_usize;
                for illust in payload.illusts {
                    if !seen_ids.insert(illust.id) {
                        continue;
                    }

                    let entity = spawn_feed_card(world, tree.home_feed, &image_cmd_tx, illust);
                    next_order.push(entity);
                    added += 1;
                }

                world.resource_mut::<FeedOrder>().0 = next_order;
                world.resource_mut::<FeedSeenIds>().0 = seen_ids;
                {
                    let mut pagination = world.resource_mut::<FeedPagination>();
                    pagination.next_url = next_url;
                    pagination.loading = false;
                }

                let message = if append {
                    format!(
                        "{} {} ({source:?})",
                        tr(
                            world,
                            "pixiv.status.appended_illustrations",
                            "Appended illustrations",
                        ),
                        added
                    )
                } else {
                    format!(
                        "{} {} ({source:?})",
                        tr(
                            world,
                            "pixiv.status.loaded_illustrations",
                            "Loaded illustrations",
                        ),
                        added
                    )
                };
                set_status(world, message);
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
            NetworkResult::Error {
                summary,
                details,
                feed_generation,
            } => {
                if let Some(feed_generation) = feed_generation {
                    let current_generation = world.resource::<FeedPagination>().generation;
                    if feed_generation != current_generation {
                        continue;
                    }
                    world.resource_mut::<FeedPagination>().loading = false;
                }

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

    sync_bound_text_inputs(world);
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
