/// Maximum window scale at which outline hinting improves small text.
///
/// Vello uses area coverage anti-aliasing rather than RGB subpixel masks. At
/// higher scale factors the extra pixel snapping is unnecessary and can make
/// glyph spacing less even, especially over transparent compositor backdrops.
pub(crate) const LOW_DPI_HINTING_MAX_SCALE: f64 = 1.25;

pub(crate) fn should_hint_text(hinting_enabled: bool, scale_factor: f64) -> bool {
    hinting_enabled
        && scale_factor.is_finite()
        && scale_factor <= LOW_DPI_HINTING_MAX_SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hinting_is_limited_to_low_dpi() {
        assert!(should_hint_text(true, 1.0));
        assert!(should_hint_text(true, LOW_DPI_HINTING_MAX_SCALE));
        assert!(!should_hint_text(true, 1.5));
        assert!(!should_hint_text(false, 1.0));
    }
}
