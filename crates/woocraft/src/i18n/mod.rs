use std::{
  borrow::Cow,
  collections::HashMap,
  ops::Deref,
  sync::{LazyLock, RwLock},
};

/// Locales woocraft ships translations for, in menu display order.
///
/// The list targets the mainstream UI-localization languages (roughly the
/// CLDR "modern" locale set, ordered by global usage): East Asian, major
/// European and major South-East Asian languages. Right-to-left languages
/// (Arabic, Hebrew, Persian, Urdu) are intentionally absent until RTL layout
/// support lands. Applications narrow this list down to their own subset via
/// the title-bar language APIs; they can also register additional locales at
/// runtime with [`load_locale`].
pub const SUPPORTED_LOCALES: [&str; 28] = [
  "en-us", "zh-hans", "zh-hant", "ja-jp", "ko-kr", "es-es", "fr-fr", "de-de", "pt-br", "it-it",
  "ru-ru", "uk-ua", "pl-pl", "nl-nl", "tr-tr", "vi-vn", "th-th", "id-id", "ms-my", "hi-in",
  "sv-se", "da-dk", "nb-no", "fi-fi", "el-gr", "cs-cz", "hu-hu", "ro-ro",
];
/// Domain every builtin translation key lives under.
///
/// [`translate`] applies it automatically, so callers normally write bare keys
/// (`menu.file`); it is public for the rare case that needs the fully
/// qualified form.
pub const WOOCRAFT_I18N_DOMAIN: &str = "tech.woooo.woocraft";

type LocaleTranslations = HashMap<String, String>;
type CustomLocaleStore = HashMap<String, LocaleTranslations>;

static CUSTOM_LOCALES: LazyLock<RwLock<CustomLocaleStore>> =
  LazyLock::new(|| RwLock::new(HashMap::new()));

fn is_woocraft_domain_key(key: &str) -> bool {
  key == WOOCRAFT_I18N_DOMAIN
    || key
      .strip_prefix(WOOCRAFT_I18N_DOMAIN)
      .is_some_and(|rest| rest.starts_with('.'))
}

/// Prefixes `key` with the woocraft i18n domain unless it already carries it,
/// borrowing in the common already-qualified case.
fn woocraft_key_cow(key: &str) -> Cow<'_, str> {
  if is_woocraft_domain_key(key) {
    Cow::Borrowed(key)
  } else {
    Cow::Owned(format!("{WOOCRAFT_I18N_DOMAIN}.{key}"))
  }
}

/// Maps a raw locale tag onto the canonical [`SUPPORTED_LOCALES`] entry that
/// ships translations for it (language-only tags and regional variants of a
/// single-variant language all collapse onto that variant).
fn normalize_known_locale(locale: &str) -> Option<&'static str> {
  if locale == "zh"
    || locale.starts_with("zh-hans")
    || locale.starts_with("zh-cn")
    || locale.starts_with("zh-sg")
  {
    Some("zh-hans")
  } else if locale.starts_with("zh-hant")
    || locale.starts_with("zh-tw")
    || locale.starts_with("zh-hk")
    || locale.starts_with("zh-mo")
  {
    Some("zh-hant")
  } else if locale == "ja"
    || locale == "jp"
    || locale.starts_with("ja-")
    || locale.starts_with("jp-")
  {
    Some("ja-jp")
  } else if locale == "en" || locale.starts_with("en-us") {
    Some("en-us")
  } else if locale == "ko" || locale.starts_with("ko-") {
    Some("ko-kr")
  } else if locale == "es" || locale.starts_with("es-") {
    Some("es-es")
  } else if locale == "fr" || locale.starts_with("fr-") {
    Some("fr-fr")
  } else if locale == "de" || locale.starts_with("de-") {
    Some("de-de")
  } else if locale == "pt" || locale.starts_with("pt-") {
    Some("pt-br")
  } else if locale == "it" || locale.starts_with("it-") {
    Some("it-it")
  } else if locale == "ru" || locale.starts_with("ru-") {
    Some("ru-ru")
  } else if locale == "uk" || locale.starts_with("uk-") {
    Some("uk-ua")
  } else if locale == "pl" || locale.starts_with("pl-") {
    Some("pl-pl")
  } else if locale == "nl" || locale.starts_with("nl-") {
    Some("nl-nl")
  } else if locale == "tr" || locale.starts_with("tr-") {
    Some("tr-tr")
  } else if locale == "vi" || locale.starts_with("vi-") {
    Some("vi-vn")
  } else if locale == "th" || locale.starts_with("th-") {
    Some("th-th")
  } else if locale == "id" || locale.starts_with("id-") {
    Some("id-id")
  } else if locale == "ms" || locale.starts_with("ms-") {
    Some("ms-my")
  } else if locale == "hi" || locale.starts_with("hi-") {
    Some("hi-in")
  } else if locale == "sv" || locale.starts_with("sv-") {
    Some("sv-se")
  } else if locale == "da" || locale.starts_with("da-") {
    Some("da-dk")
  } else if locale == "nb"
    || locale == "nn"
    || locale == "no"
    || locale.starts_with("nb-")
    || locale.starts_with("nn-")
    || locale.starts_with("no-")
  {
    // Norwegian bokmål is the default variant of the macro language `no`.
    Some("nb-no")
  } else if locale == "fi" || locale.starts_with("fi-") {
    Some("fi-fi")
  } else if locale == "el" || locale.starts_with("el-") {
    Some("el-gr")
  } else if locale == "cs" || locale.starts_with("cs-") {
    Some("cs-cz")
  } else if locale == "hu" || locale.starts_with("hu-") {
    Some("hu-hu")
  } else if locale == "ro" || locale.starts_with("ro-") {
    Some("ro-ro")
  } else {
    None
  }
}

/// Maps a raw locale tag onto the canonical locale that ships translations.
///
/// Strips encoding/modifier suffixes (`zh_CN.UTF-8` → `zh-hans`) and collapses
/// language-only or regional variants onto the single shipped variant. Unknown
/// locales pass through lowercased, so applications can register their own.
pub fn normalize_locale(locale: &str) -> String {
  let mut normalized = locale.trim().to_ascii_lowercase().replace('_', "-");

  if let Some((prefix, _)) = normalized.split_once('.') {
    normalized = prefix.to_string();
  }

  if let Some((prefix, _)) = normalized.split_once('@') {
    normalized = prefix.to_string();
  }

  if normalized.is_empty() {
    return "en-us".to_string();
  }

  if let Some(mapped) = normalize_known_locale(&normalized) {
    return mapped.to_string();
  }

  normalized
}

/// Sets the active locale from the environment (`LC_ALL`, then `LANG`).
///
/// Applications call this once at startup when they have no preference of
/// their own; otherwise use [`set_locale`].
pub fn init() {
  let locale = std::env::var("LC_ALL")
    .ok()
    .or_else(|| std::env::var("LANG").ok())
    .unwrap_or_else(|| "en-us".to_string());

  set_locale(&locale);
}

#[inline]
pub fn locale() -> impl Deref<Target = str> {
  rust_i18n::locale()
}

#[inline]
pub fn set_locale(locale: &str) {
  let locale = normalize_locale(locale);
  rust_i18n::set_locale(&locale);
}

/// Every locale that has translations: the shipped set, whatever the builtin
/// tables provide, and every locale registered at runtime, without duplicates.
pub fn available_locales() -> Vec<String> {
  let mut locales = SUPPORTED_LOCALES
    .iter()
    .map(|locale| locale.to_string())
    .collect::<Vec<_>>();

  for locale in rust_i18n::available_locales!() {
    let locale = normalize_locale(&locale);
    if !locales.iter().any(|existing| existing == &locale) {
      locales.push(locale);
    }
  }

  let custom_locales = CUSTOM_LOCALES
    .read()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  let mut custom_locales = custom_locales.keys().cloned().collect::<Vec<_>>();
  custom_locales.sort();

  for locale in custom_locales {
    if !locales.iter().any(|existing| existing == &locale) {
      locales.push(locale);
    }
  }

  locales
}

/// Registers (or replaces) an application's whole translation table for
/// `locale`.
///
/// Keys in the woocraft domain are ignored: builtin translations cannot be
/// overridden. Use [`extend_locale`] to add entries without dropping the ones
/// already registered.
pub fn load_locale(locale: impl AsRef<str>, translations: HashMap<String, String>) {
  let locale = normalize_locale(locale.as_ref());
  let translations = translations
    .into_iter()
    .filter(|(key, _)| !is_woocraft_domain_key(key))
    .collect();

  CUSTOM_LOCALES
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
    .insert(locale, translations);
}

/// Adds application translations to `locale`, keeping what is already
/// registered there.
///
/// Keys in the woocraft domain are ignored.
pub fn extend_locale<I, K, V>(locale: impl AsRef<str>, translations: I)
where
  I: IntoIterator<Item = (K, V)>,
  K: Into<String>,
  V: Into<String>, {
  let locale = normalize_locale(locale.as_ref());
  let mut custom_locales = CUSTOM_LOCALES
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  let locale_translations = custom_locales.entry(locale).or_default();

  for (key, value) in translations {
    let key = key.into();
    if !is_woocraft_domain_key(&key) {
      locale_translations.insert(key, value.into());
    }
  }
}

/// Translates `key` in an explicit `locale`.
///
/// Same contract as [`translate`], with the locale passed in instead of read
/// from the global state.
pub fn translate_in_locale(locale: impl AsRef<str>, key: impl AsRef<str>) -> String {
  let locale = normalize_locale(locale.as_ref());
  translate_normalized(&locale, key.as_ref())
}

/// Lookup core shared by [`translate`] and [`translate_in_locale`], for callers
/// that already hold a normalized locale.
fn translate_normalized(locale: &str, key: &str) -> String {
  find_translation(locale, key).unwrap_or_else(|| key.to_string())
}

/// Translates `key` in the active locale.
///
/// `key` is either an application key registered through [`load_locale`] /
/// [`extend_locale`], or one of woocraft's own keys, written either bare
/// (`menu.file`) or fully qualified (`tech.woooo.woocraft.menu.file`).
///
/// A key that resolves to nothing is returned unchanged, which is what makes
/// this the right call at widget boundaries that accept a label: widgets
/// document that a label may be a key (translated automatically) or literal
/// text (passed through), instead of leaking rust-i18n's `<locale>.<key>`
/// placeholder into the UI.
pub fn translate(key: impl AsRef<str>) -> String {
  let locale = locale();
  translate_normalized(&locale, key.as_ref())
}

/// Human-readable name of `locale`, for language menus.
///
/// Builtin locales answer with their native name; an application may register
/// its own under the plain `i18n.name` key. Anything else falls back to the
/// locale tag itself.
pub fn locale_display_name(locale: impl AsRef<str>) -> String {
  let locale = normalize_locale(locale.as_ref());
  // The builtin table carries a native name per shipped locale and the
  // configured `en-us` fallback guarantees one for the rest; a custom locale
  // may register its own name under the plain `i18n.name` key.
  find_translation(&locale, "i18n.name").unwrap_or(locale)
}

/// Resolves `key` in `locale`, against the custom store first and the builtin
/// tables second.
///
/// 1. Application translations, walked once along the locale's RFC 4647
///    fallback chain (exact locale first, then e.g. `zh-hant-tw` → `zh-hant` →
///    `zh`), so an application may register a translation for a parent locale
///    only.
/// 2. The builtin tables, which are stored under [`WOOCRAFT_I18N_DOMAIN`], so
///    both `common.loading` and the fully-qualified key resolve. rust-i18n
///    already applies the same fallback chain plus the configured fallback
///    locales (`en-us`), so there is no need to walk it here as well.
///
/// Woocraft-domain keys are never stored in the custom store (they are
/// filtered by [`load_locale`] and [`extend_locale`]), so they skip step 1.
fn find_translation(locale: &str, key: &str) -> Option<String> {
  if !is_woocraft_domain_key(key) {
    let custom_locales = CUSTOM_LOCALES
      .read()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut current_locale = Some(locale);

    while let Some(locale_str) = current_locale {
      if let Some(value) = custom_locales
        .get(locale_str)
        .and_then(|translations| translations.get(key))
      {
        return Some(value.clone());
      }

      current_locale = crate::_rust_i18n_lookup_fallback(locale_str);
    }
  }

  let builtin_key = woocraft_key_cow(key);
  crate::_rust_i18n_try_translate(locale, &builtin_key).map(Cow::into_owned)
}

#[cfg(test)]
mod tests {
  use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard},
  };

  use super::*;

  static TEST_LOCALE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

  /// Serializes the tests that touch the global custom-locale store and clears
  /// it before and after, so no test observes another test's data.
  struct LocaleTestGuard {
    _guard: MutexGuard<'static, ()>,
  }

  impl LocaleTestGuard {
    fn new() -> Self {
      let guard = TEST_LOCALE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
      clear_custom_locales();
      Self { _guard: guard }
    }
  }

  impl Drop for LocaleTestGuard {
    fn drop(&mut self) {
      clear_custom_locales();
    }
  }

  fn clear_custom_locales() {
    CUSTOM_LOCALES
      .write()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .clear();
  }

  /// The whole contract of [`translate_in_locale`]: what it resolves, what wins
  /// when several sources match, and what it returns when nothing does.
  #[test]
  fn test_translate_resolves_keys_and_keeps_literals() {
    let _guard = LocaleTestGuard::new();

    // Application keys registered through `load_locale` and `extend_locale`.
    load_locale(
      "zh-hans",
      HashMap::from([("app.greeting".to_string(), "你好".to_string())]),
    );
    extend_locale("zh-hans", [("app.farewell", "再见")]);
    assert_eq!(translate_in_locale("zh-hans", "app.greeting"), "你好");
    assert_eq!(translate_in_locale("zh-hans", "app.farewell"), "再见");

    // Registered data wins over a builtin key of the same name ...
    extend_locale("zh-hans", [("menu.help", "自定义帮助")]);
    assert_eq!(translate_in_locale("zh-hans", "menu.help"), "自定义帮助");

    // ... but never over woocraft's own domain, which both registration
    // functions filter out.
    load_locale(
      "en-us",
      HashMap::from([(
        format!("{WOOCRAFT_I18N_DOMAIN}.common.loading"),
        "should be ignored".to_string(),
      )]),
    );
    assert_eq!(translate_in_locale("en-us", "common.loading"), "Loading...");

    // Builtin keys resolve bare and fully qualified.
    assert_eq!(translate_in_locale("en-us", "menu.file"), "File");
    assert_eq!(
      translate_in_locale("zh-hans", "tech.woooo.woocraft.menu.file"),
      "文件"
    );

    // Anything matching neither source is returned verbatim, which is what
    // lets widgets accept literal labels without leaking `<locale>.<key>`.
    assert_eq!(translate_in_locale("en-us", "New File"), "New File");
    assert_eq!(translate_in_locale("zh-hans", "新建文件"), "新建文件");
    assert_eq!(
      translate_in_locale("en-us", "some.unregistered.key"),
      "some.unregistered.key"
    );
  }

  #[test]
  fn test_translate_falls_back_through_locale_chain() {
    let _guard = LocaleTestGuard::new();

    // `en-gb` ships nothing of its own; rust-i18n resolves it through the `en`
    // subtag and the configured `en-us` fallback.
    assert_eq!(normalize_locale("en-gb"), "en-gb");
    assert_eq!(translate_in_locale("en-gb", "common.loading"), "Loading...");

    // A custom translation may be registered for a parent locale only.
    load_locale(
      "xx",
      HashMap::from([("app.greeting".to_string(), "custom greeting".to_string())]),
    );
    assert_eq!(
      translate_in_locale("xx-latn", "app.greeting"),
      "custom greeting"
    );

    // Display names come from the builtin table with the same fallback.
    assert_eq!(locale_display_name("zh-CN"), "简体中文");
    assert_eq!(locale_display_name("xx-latn"), "English (US)");
  }

  #[test]
  fn test_normalize_locale_collapses_variants() {
    // Legacy mappings keep working.
    assert_eq!(normalize_locale("zh_CN.UTF-8"), "zh-hans");
    assert_eq!(normalize_locale("zh-tw"), "zh-hant");
    assert_eq!(normalize_locale("ja"), "ja-jp");
    assert_eq!(normalize_locale("en"), "en-us");

    // Language-only tags and regional variants collapse onto the single
    // shipped variant of each supported language.
    assert_eq!(normalize_locale("KO"), "ko-kr");
    assert_eq!(normalize_locale("es-419"), "es-es");
    assert_eq!(normalize_locale("pt"), "pt-br");
    assert_eq!(normalize_locale("pt-PT"), "pt-br");
    assert_eq!(normalize_locale("nb_NO"), "nb-no");
    assert_eq!(normalize_locale("nn"), "nb-no");
    assert_eq!(normalize_locale("no"), "nb-no");
    assert_eq!(normalize_locale("sv"), "sv-se");

    // Unknown locales pass through (lowercased) for custom registration.
    assert_eq!(normalize_locale("xx-Latn"), "xx-latn");
  }
}
