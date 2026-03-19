use super::*;

fn extract_code_from_url(url: &Url, depth: u8) -> Option<String> {
    if depth == 0 {
        return None;
    }

    if let Some((_, code)) = url
        .query_pairs()
        .find(|(key, value)| key == "code" && !value.is_empty())
    {
        return Some(code.into_owned());
    }

    for (key, value) in url.query_pairs() {
        if matches!(key.as_ref(), "return_to" | "redirect" | "redirect_uri")
            && let Ok(nested_url) = Url::parse(value.as_ref())
            && let Some(code) = extract_code_from_url(&nested_url, depth - 1)
        {
            return Some(code);
        }
    }

    None
}

pub(super) fn extract_auth_code_from_input(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(url) = Url::parse(trimmed) {
        if let Some(code) = extract_code_from_url(&url, 4) {
            return Some(code);
        }
        return None;
    }

    Some(trimmed.to_string())
}

pub(super) fn is_pixiv_callback_uri(input: &str) -> bool {
    Url::parse(input)
        .map(|url| url.scheme().eq_ignore_ascii_case("pixiv"))
        .unwrap_or(false)
}

fn process_activation_uri(world: &mut World, uri: &str) {
    if !is_pixiv_callback_uri(uri) {
        return;
    }

    {
        let mut auth = world.resource_mut::<AuthState>();
        auth.auth_code_input = uri.to_string();
    }

    let Some(code) = extract_auth_code_from_input(uri) else {
        set_status_key(
            world,
            "pixiv.status.activation_code_missing",
            "Received pixiv callback but no `code=` was found.",
        );
        return;
    };

    let code_verifier = world.resource::<AuthState>().code_verifier_input.clone();
    if code_verifier.trim().is_empty() {
        world.resource_mut::<AuthState>().login_dialog_open = true;
        ensure_auth_dialog_overlay(world);
        set_status_key(
            world,
            "pixiv.status.activation_verifier_missing",
            "Received pixiv callback, but PKCE code_verifier is empty. Click Open Browser Login first.",
        );
        return;
    }

    let _ = world
        .resource::<NetworkBridge>()
        .cmd_tx
        .send(NetworkCommand::ExchangeCode {
            code,
            code_verifier,
        });

    set_status_key(
        world,
        "pixiv.status.activation_exchange_started",
        "Received pixiv callback. Exchanging auth code automatically…",
    );

    sync_bound_text_inputs(world);
}

#[cfg(not(target_os = "macos"))]
pub(super) fn poll_activation_messages(world: &mut World) {
    let Some(mut activation) = world.get_resource_mut::<ActivationBridge>() else {
        return;
    };

    let mut pending_uris = std::mem::take(&mut activation.startup_uris);
    if let Ok(mut service) = activation.service.lock() {
        pending_uris.extend(service.drain_uris());
    }

    for uri in pending_uris {
        process_activation_uri(world, &uri);
    }
}

#[cfg(target_os = "macos")]
pub(super) fn poll_activation_messages(world: &mut World) {
    let Some(mut activation) = world.get_non_send_resource_mut::<ActivationBridge>() else {
        return;
    };

    let mut pending_uris = std::mem::take(&mut activation.startup_uris);
    pending_uris.extend(activation.service.drain_uris());

    for uri in pending_uris {
        process_activation_uri(world, &uri);
    }
}
