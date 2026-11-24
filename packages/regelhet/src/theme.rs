//! The accent colour of the screens.
//!
//! The NLDD design system sets its accent as a theme: one named palette of
//! the Rijkshuisstijl, chosen with `data-nldd-theme="<name>"` on the page,
//! which recolours every component in light and dark mode at once. A
//! deployment names one with `REGELHET_ACCENT_COLOR` ([`crate::config`]);
//! the server hands it to the screens on `GET /api/uiterlijk`
//! ([`crate::api`]), and they set it on `<html>`. A single CSS colour would
//! not do: the accent is a scale of steps per colour scheme, and the design
//! system generates those per theme.

/// The themes of `@nldd/design-system` (`dist/css/themes/`). A theme the
/// design system adds later is refused until it is listed here; one it
/// drops leaves the screens on their default accent.
pub const NLDD_THEMES: [&str; 17] = [
    "bruin",
    "donkerblauw",
    "donkerbruin",
    "donkergeel",
    "donkergroen",
    "geel",
    "groen",
    "hemelblauw",
    "lichtblauw",
    "mintgroen",
    "mosgroen",
    "oranje",
    "paars",
    "robijnrood",
    "rood",
    "roze",
    "violet",
];

/// One of [`NLDD_THEMES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccentColor(&'static str);

impl AccentColor {
    /// The theme's name, as `data-nldd-theme` takes it.
    pub fn name(self) -> &'static str {
        self.0
    }
}

/// A name that is none of [`NLDD_THEMES`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is no theme of the NLDD design system; it is one of {themes}", themes = NLDD_THEMES.join(", "))]
pub struct UnknownAccentColor(pub String);

impl std::str::FromStr for AccentColor {
    type Err = UnknownAccentColor;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let name = s.trim();
        NLDD_THEMES
            .into_iter()
            .find(|t| *t == name)
            .map(Self)
            .ok_or_else(|| UnknownAccentColor(name.to_string()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_of_the_design_system_is_an_accent_colour() {
        for theme in NLDD_THEMES {
            assert_eq!(theme.parse::<AccentColor>().unwrap().name(), theme);
        }
        assert_eq!(" paars ".parse::<AccentColor>().unwrap().name(), "paars");
    }

    #[test]
    fn a_css_colour_or_an_unknown_name_is_refused() {
        for value in ["#7d3f98", "purple", "Paars", "lintblauw"] {
            let e = value.parse::<AccentColor>().unwrap_err();
            assert_eq!(e, UnknownAccentColor(value.to_string()));
            assert!(e.to_string().contains("donkerblauw"), "{e}");
        }
    }
}
