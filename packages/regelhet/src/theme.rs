//! How the screens look: their accent colour and the title in their bar.
//!
//! The NLDD design system sets its accent as a theme: one named palette of
//! the Rijkshuisstijl, chosen with `data-nldd-theme="<name>"` on the page,
//! which recolours every component in light and dark mode at once. A
//! deployment names one with `REGELHET_ACCENT_COLOR` ([`crate::config`]);
//! the server hands it to the screens on `GET /api/uiterlijk`
//! ([`crate::api`]), and they set it on `<html>`. A single CSS colour would
//! not do: the accent is a scale of steps per colour scheme, and the design
//! system generates those per theme.
//!
//! The title is the name in the bar at the top of a side without sections,
//! such as the citizen's. A deployment names one with `REGELHET_TITLE`
//! ([`Title`]); without it the screens carry the case's own `titel`.

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

/// The most characters a [`Title`] holds: it stands in a toolbar beside the
/// account menu, and on a narrow screen beside an icon button.
pub const TITLE_MAX_CHARS: usize = 80;

/// The title in the bar of the screens: trimmed, not empty, at most
/// [`TITLE_MAX_CHARS`] characters, and on one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title(String);

impl Title {
    /// The title as the screens show it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a text is no [`Title`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BadTitle {
    #[error("it is empty")]
    Empty,
    #[error("it is {0} characters long; a title is at most {TITLE_MAX_CHARS}")]
    TooLong(usize),
    #[error("it holds a line break or another control character")]
    Control,
}

impl std::str::FromStr for Title {
    type Err = BadTitle;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let title = s.trim();
        if title.is_empty() {
            return Err(BadTitle::Empty);
        }
        if title.chars().any(char::is_control) {
            return Err(BadTitle::Control);
        }
        let length = title.chars().count();
        if length > TITLE_MAX_CHARS {
            return Err(BadTitle::TooLong(length));
        }
        Ok(Self(title.to_string()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_trimmed_one_line_and_short() {
        let t: Title = "  Bedrijfsparkeervergunning ".parse().unwrap();
        assert_eq!(t.as_str(), "Bedrijfsparkeervergunning");
        assert_eq!("   ".parse::<Title>().unwrap_err(), BadTitle::Empty);
        assert_eq!("a\nb".parse::<Title>().unwrap_err(), BadTitle::Control);
        let long = "é".repeat(TITLE_MAX_CHARS + 1);
        assert_eq!(
            long.parse::<Title>().unwrap_err(),
            BadTitle::TooLong(TITLE_MAX_CHARS + 1)
        );
        assert!("é".repeat(TITLE_MAX_CHARS).parse::<Title>().is_ok());
    }

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
