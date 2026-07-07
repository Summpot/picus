use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    io,
    path::Path,
};

use bevy_asset::{AssetEvent, Assets};
use bevy_ecs::{message::MessageReader, prelude::*, system::NonSendMut};
use bevy_text::Font;

use crate::MasonryRuntime;

/// Font bridge resource that stores pending font files for registration in Masonry Core/Parley.
///
/// Fonts can be queued either from raw bytes or by file path (for example
/// `assets/fonts/NotoSansCJK-Regular.otf`).
#[derive(Resource, Debug, Default)]
pub struct XilemFontBridge {
    registered_fonts: Vec<Vec<u8>>,
    pending_fonts: Vec<Vec<u8>>,
    registered_fingerprints: HashSet<u64>,
}

pub(crate) fn font_bytes_fingerprint(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

impl XilemFontBridge {
    /// Queue a font from raw bytes. Returns `true` when queued, `false` if duplicate.
    pub fn register_font_bytes(&mut self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }

        let fingerprint = font_bytes_fingerprint(bytes);

        if !self.registered_fingerprints.insert(fingerprint) {
            return false;
        }

        let bytes = bytes.to_vec();
        self.registered_fonts.push(bytes.clone());
        self.pending_fonts.push(bytes);
        true
    }

    /// Queue a font by reading it from disk.
    ///
    /// Typical path for Bevy projects: `assets/fonts/<font-file>.ttf|otf`.
    pub fn register_font_path(&mut self, path: impl AsRef<Path>) -> io::Result<bool> {
        let data = fs::read(path)?;
        Ok(self.register_font_bytes(&data))
    }

    pub fn take_pending_fonts(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending_fonts)
    }

    #[must_use]
    pub fn has_pending_fonts(&self) -> bool {
        !self.pending_fonts.is_empty()
    }

    pub fn registered_font_bytes(&self) -> impl Iterator<Item = &[u8]> {
        self.registered_fonts.iter().map(Vec::as_slice)
    }
}

/// Option A bridge: consume Bevy `AssetEvent<Font>` and queue loaded font bytes.
///
/// This enables dynamic loading via `AssetServer::load("fonts/...")`.
pub fn collect_bevy_font_assets(
    mut font_events: MessageReader<AssetEvent<Font>>,
    fonts: Option<Res<Assets<Font>>>,
    mut bridge: ResMut<XilemFontBridge>,
) {
    let Some(fonts) = fonts else {
        return;
    };

    for event in font_events.read() {
        let Some(id) = (match event {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => Some(*id),
            AssetEvent::Removed { .. } | AssetEvent::Unused { .. } => None,
        }) else {
            continue;
        };

        if let Some(font) = fonts.get(id) {
            bridge.register_font_bytes(font.data.data());
        }
    }
}

/// Sync pending font bytes into Masonry Core's internal text/font database.
///
/// This is the bridge between Bevy-side app setup and retained font shaping.
pub fn sync_fonts_to_xilem(
    runtime: Option<NonSendMut<MasonryRuntime>>,
    mut bridge: ResMut<XilemFontBridge>,
) {
    let Some(mut runtime) = runtime else {
        return;
    };
    if runtime.windows.is_empty() || !bridge.has_pending_fonts() {
        return;
    }

    let pending = bridge.take_pending_fonts();
    for font_bytes in pending {
        runtime.register_fonts_all(font_bytes);
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn xilem_font_bridge_deduplicates_same_font_bytes() {
        let mut bridge = crate::XilemFontBridge::default();
        assert!(bridge.register_font_bytes(b"font-data"));
        assert!(!bridge.register_font_bytes(b"font-data"));
        assert_eq!(bridge.registered_font_bytes().count(), 1);
        assert!(bridge.has_pending_fonts());
    }

    #[test]
    fn xilem_font_bridge_keeps_registered_fonts_after_draining_pending() {
        let mut bridge = crate::XilemFontBridge::default();
        assert!(bridge.register_font_bytes(b"font-data"));

        let pending = bridge.take_pending_fonts();

        assert_eq!(pending, vec![b"font-data".to_vec()]);
        assert!(!bridge.has_pending_fonts());
        assert_eq!(
            bridge.registered_font_bytes().collect::<Vec<_>>(),
            vec![b"font-data".as_slice()]
        );
    }


}
