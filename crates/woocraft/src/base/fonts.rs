//! Font configuration: primary family and fallback selection.
//!
//! The embedded [`crate::DEFAULT_FONT_FAMILY`] (Iosevka Term) covers Latin,
//! Greek, Cyrillic and symbol scripts with single-cell metrics; every other
//! script resolves through the platform text stack. This module centralizes
//! how the primary family and the fallback list are chosen:
//!
//! 1. Application overrides ([`set_font_overrides`]) always win — including
//!    overriding the primary family itself (e.g. to a sans font).
//! 2. Platform discovery ([`platform_font_fallbacks`]): `None` on macOS and
//!    Windows, where the OS cascade is already locale-aware and sans-biased; a
//!    locale-aware fontconfig probe on Linux, whose own cascade is a static
//!    Noto wish-list instead of a user-language-ordered list.
//!
//! The same sans-preferring fallback applies to every widget; the terminal
//! instead chains the embedded icon family first (see
//! [`terminal_font_fallbacks`]), and widgets may expose their own primary
//! font setting (the terminal does).

use std::sync::{Arc, LazyLock, RwLock};

use gpui::{Font, FontFallbacks, FontFeatures, FontStyle, FontWeight, SharedString};

use crate::DEFAULT_FONT_FAMILY;

/// Application-provided font overrides, consulted before platform defaults.
///
/// Fields set to `None` keep the library default; fields set to `Some(..)`
/// replace it. In particular, `fallbacks: Some(vec![])` explicitly disables
/// fallbacks while `fallbacks: None` defers to [`platform_font_fallbacks`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FontOverrides {
  /// Primary font family. `None` keeps [`DEFAULT_FONT_FAMILY`].
  pub family: Option<SharedString>,
  /// Fallback font families. `None` keeps the platform default.
  pub fallbacks: Option<Vec<String>>,
}

impl FontOverrides {
  /// Sets the primary font family.
  #[must_use]
  pub fn family(mut self, family: impl Into<SharedString>) -> Self {
    self.family = Some(family.into());
    self
  }

  /// Sets the fallback font families.
  #[must_use]
  pub fn fallbacks(mut self, fallbacks: Vec<String>) -> Self {
    self.fallbacks = Some(fallbacks);
    self
  }
}

/// Immutable snapshot of the current font configuration, shared by `Arc` so
/// render-path readers clone a pointer instead of deep-copying the override
/// family list on every call. `override_fallbacks` is prebuilt from
/// [`FontOverrides::fallbacks`] at write time (see [`set_font_overrides`])
/// so readers never rebuild it; `None` means no fallback override is set and
/// the platform default applies.
#[derive(Clone, Default)]
struct FontSnapshot {
  overrides: FontOverrides,
  override_fallbacks: Option<FontFallbacks>,
}

static OVERRIDES: LazyLock<RwLock<Arc<FontSnapshot>>> =
  LazyLock::new(|| RwLock::new(Arc::new(FontSnapshot::default())));

/// Returns the current font snapshot, cloning an `Arc` rather than the
/// configuration itself.
fn font_snapshot() -> Arc<FontSnapshot> {
  OVERRIDES
    .read()
    .expect("font overrides lock should not be poisoned")
    .clone()
}

/// Replaces the application-wide font overrides.
///
/// Call this once during application startup, before the first frame is
/// rendered. Both the primary family and the fallback list are honored by
/// [`default_font`], the flex layout helpers, and the terminal widget
/// (whose own font family setting, when set, still takes precedence).
pub fn set_font_overrides(overrides: FontOverrides) {
  let override_fallbacks = overrides
    .fallbacks
    .as_ref()
    .map(|fallbacks| FontFallbacks::from_fonts(fallbacks.clone()));
  *OVERRIDES
    .write()
    .expect("font overrides lock should not be poisoned") = Arc::new(FontSnapshot {
    overrides,
    override_fallbacks,
  });
}

/// Returns a snapshot of the current application font overrides.
pub fn font_overrides() -> FontOverrides {
  font_snapshot().overrides.clone()
}

/// Applies the application fallback override, falling back to the given
/// platform default when no override is set.
pub fn font_fallbacks_with(platform_default: Option<FontFallbacks>) -> Option<FontFallbacks> {
  match &font_snapshot().override_fallbacks {
    Some(fallbacks) => Some(fallbacks.clone()),
    None => platform_default,
  }
}

/// The default UI font: the overridden or embedded primary family, with
/// overridden or platform-discovered fallbacks.
pub fn default_font() -> Font {
  let snapshot = font_snapshot();
  Font {
    family: snapshot
      .overrides
      .family
      .clone()
      .unwrap_or_else(|| SharedString::from(DEFAULT_FONT_FAMILY)),
    weight: FontWeight::NORMAL,
    style: FontStyle::Normal,
    features: FontFeatures::default(),
    fallbacks: match &snapshot.override_fallbacks {
      Some(fallbacks) => Some(fallbacks.clone()),
      None => platform_font_fallbacks(),
    },
  }
}

/// Returns the platform-appropriate font fallback families for non-Latin
/// scripts in UI text.
///
/// The embedded primary font covers Latin plus terminal symbols; everything
/// else resolves through this list (before the platform's own cascade).
///
/// - macOS (CoreText) and Windows (DirectWrite): `None`. Their system cascades
///   are locale-aware and sans-biased, honoring the user's language settings;
///   pinning a CJK family here would pre-empt the correct Han unification
///   variants for Japanese/Korean/Traditional-Chinese users.
/// - Linux: a locale-derived fontconfig probe (`sans-serif:lang=<locale>`), so
///   the user's own fontconfig preferences decide the order. The probe asks
///   fontconfig rather than telling it what to answer — the same
///   sans-preferring behavior macOS and Windows exhibit out of the box.
///
/// Returns `None` on Linux when fontconfig cannot be consulted (no
/// `fc-match` binary, musl/minimal images), letting the platform cascade
/// decide entirely.
pub fn platform_font_fallbacks() -> Option<FontFallbacks> {
  #[cfg(target_os = "linux")]
  {
    static FAMILIES: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
    fontconfig_fallbacks("sans-serif", &FAMILIES)
  }
  #[cfg(not(target_os = "linux"))]
  {
    None
  }
}

/// Like [`platform_font_fallbacks`], but preferring a monospace font.
///
/// Used by widgets with intrinsic monospace grids (the terminal), where CJK
/// cells must align with the Latin grid. UI text should use
/// [`platform_font_fallbacks`] instead.
pub fn platform_monospace_font_fallbacks() -> Option<FontFallbacks> {
  #[cfg(target_os = "linux")]
  {
    static FAMILIES: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
    fontconfig_fallbacks("monospace", &FAMILIES)
  }
  #[cfg(not(target_os = "linux"))]
  {
    None
  }
}

/// Returns the fallback chain for grid-aligned text (the terminal): the
/// embedded icon family ([`crate::SYMBOLS_FONT_FAMILY`]) first, then the
/// locale-aware monospace platform chain for CJK.
///
/// The embedded primary family carries no Nerd Font glyphs, so terminal
/// icon codepoints resolve through this single shared fallback face instead
/// of being duplicated into every text face of the family. Application
/// fallback overrides ([`set_font_overrides`]) replace the whole chain —
/// including the icon family — when set.
pub fn terminal_font_fallbacks() -> Option<FontFallbacks> {
  #[cfg(target_os = "linux")]
  let platform_families: Option<Vec<String>> = {
    static FAMILIES: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
    FAMILIES
      .get_or_init(|| fontconfig_families("monospace"))
      .clone()
  };
  #[cfg(not(target_os = "linux"))]
  let platform_families: Option<Vec<String>> = None;

  let mut families = vec![crate::SYMBOLS_FONT_FAMILY.to_owned()];
  families.extend(platform_families.unwrap_or_default());
  Some(FontFallbacks::from_fonts(families))
}

/// Queries fontconfig for the best families of `generic_family`, optionally
/// constrained to the user's locale language. Callers own the caching.
#[cfg(target_os = "linux")]
fn fontconfig_families(generic_family: &str) -> Option<Vec<String>> {
  let output = std::process::Command::new("fc-match")
    .args([
      "-s",
      "-f",
      "%{family}\n",
      &match locale_language_tag() {
        Some(lang) => format!("{generic_family}:lang={lang}"),
        None => generic_family.to_string(),
      },
    ])
    .output()
    .ok()
    .filter(|output| output.status.success())?;
  let families = parse_fc_match_families(&String::from_utf8_lossy(&output.stdout));
  (!families.is_empty()).then_some(families)
}

/// Queries fontconfig once (per caller-owned `cache`) for the best families
/// of `generic_family` and wraps them into [`FontFallbacks`].
#[cfg(target_os = "linux")]
fn fontconfig_fallbacks(
  generic_family: &str, cache: &std::sync::OnceLock<Option<Vec<String>>>,
) -> Option<FontFallbacks> {
  cache
    .get_or_init(|| fontconfig_families(generic_family))
    .as_ref()
    .map(|families| FontFallbacks::from_fonts(families.clone()))
}

/// Extracts up to two distinct font family names from sorted `fc-match -s`
/// output. Each line may carry a comma-separated alias list; only the first
/// (canonical) family of each line is kept.
#[cfg(target_os = "linux")]
fn parse_fc_match_families(stdout: &str) -> Vec<String> {
  let mut families = Vec::new();
  for line in stdout.lines() {
    let Some(family) = line
      .split(',')
      .next()
      .map(str::trim)
      .filter(|family| !family.is_empty())
    else {
      continue;
    };
    if !families.contains(&family.to_owned()) {
      families.push(family.to_owned());
    }
    if families.len() == 2 {
      break;
    }
  }
  families
}

/// Derives a fontconfig language tag (e.g. `zh-cn`) from the current
/// locale environment (`LC_ALL` > `LC_CTYPE` > `LANG`).
#[cfg(target_os = "linux")]
fn locale_language_tag() -> Option<String> {
  ["LC_ALL", "LC_CTYPE", "LANG"]
    .iter()
    .find_map(|name| {
      std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
    })
    .as_deref()
    .and_then(language_tag_from_locale)
}

/// Converts a locale string such as `zh_CN.UTF-8`, `ca_ES@valencia`, or
/// `de_DE@euro.ISO-8859-15` into a fontconfig language tag. Returns `None`
/// for non-language locales (`C`, `POSIX`, empty).
#[cfg(target_os = "linux")]
fn language_tag_from_locale(locale: &str) -> Option<String> {
  let base = locale.trim().split('@').next()?.split('.').next()?;
  if base.is_empty() {
    return None;
  }
  let (language, country) = match base.split_once('_') {
    Some((language, country)) => (language, Some(country)),
    None => (base, None),
  };
  let language = language.trim().to_ascii_lowercase();
  if language.is_empty() || language == "c" || language == "posix" {
    return None;
  }
  Some(
    match country.map(|country| country.trim().to_ascii_lowercase()) {
      Some(country) if !country.is_empty() => format!("{language}-{country}"),
      _ => language,
    },
  )
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
  use super::*;

  #[test]
  fn language_tags_follow_locale_naming() {
    assert_eq!(
      language_tag_from_locale("zh_CN.UTF-8").as_deref(),
      Some("zh-cn")
    );
    assert_eq!(language_tag_from_locale("ja_JP").as_deref(), Some("ja-jp"));
    assert_eq!(language_tag_from_locale("ja").as_deref(), Some("ja"));
    assert_eq!(
      language_tag_from_locale("ca_ES@valencia").as_deref(),
      Some("ca-es")
    );
    assert_eq!(
      language_tag_from_locale("de_DE@euro.ISO-8859-15").as_deref(),
      Some("de-de")
    );
    assert_eq!(language_tag_from_locale("C"), None);
    assert_eq!(language_tag_from_locale("C.UTF-8"), None);
    assert_eq!(language_tag_from_locale("POSIX"), None);
    assert_eq!(language_tag_from_locale(""), None);
    assert_eq!(language_tag_from_locale("_DE"), None);
  }

  #[test]
  fn fc_match_output_yields_distinct_families() {
    let output = "Noto Sans CJK SC,Noto Sans CJK SC Thin\nWenQuanYi Zen Hei\n\n";
    assert_eq!(
      parse_fc_match_families(output),
      vec![
        "Noto Sans CJK SC".to_owned(),
        "WenQuanYi Zen Hei".to_owned(),
      ]
    );
    assert!(parse_fc_match_families("\n \n").is_empty());
  }
}
