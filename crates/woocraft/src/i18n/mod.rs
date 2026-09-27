use std::{
  borrow::Cow,
  collections::HashMap,
  ops::Deref,
  sync::{LazyLock, RwLock},
};

use gpui::SharedString;

pub const SUPPORTED_LOCALES: [&str; 4] = ["zh-hans", "zh-hant", "en-us", "ja-jp"];
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

/// Builds a woocraft-domain i18n key.
pub fn woocraft_key(key: impl AsRef<str>) -> String {
  format_woocraft_key(key.as_ref())
}

/// Cow-based variant of [`woocraft_key`] for crate-internal hot paths.
///
/// Keys that are already in the woocraft domain are passed through as
/// [`Cow::Borrowed`] when the input permits borrowing (`&'static str`), so
/// domain-prefixed static keys cost no allocation.
pub(crate) fn woocraft_key_cow(key: impl Into<Cow<'static, str>>) -> Cow<'static, str> {
  match key.into() {
    Cow::Borrowed(key) if is_woocraft_domain_key(key) => Cow::Borrowed(key),
    Cow::Owned(key) if is_woocraft_domain_key(&key) => Cow::Owned(key),
    key => Cow::Owned(format_woocraft_key(&key)),
  }
}

/// Prefixes `key` with the woocraft i18n domain, returning a fresh `String`.
///
/// Internal `&str` primitive behind [`woocraft_key`] for inputs with a
/// non-static lifetime (the public entries take `impl AsRef<str>`).
fn format_woocraft_key(key: &str) -> String {
  if is_woocraft_domain_key(key) {
    key.to_string()
  } else {
    format!("{WOOCRAFT_I18N_DOMAIN}.{key}")
  }
}

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
  } else {
    None
  }
}

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
  // The active locale changed, so every cached translation is stale.
  clear_translation_cache();
}

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
  // Custom translation data changed for the locale, invalidate the cache.
  clear_translation_cache();
}

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

  // Custom translation data changed for the locale, invalidate the cache.
  clear_translation_cache();
}

pub fn try_translate_woocraft_in_locale(
  locale: impl AsRef<str>, key: impl AsRef<str>,
) -> Option<String> {
  let locale = normalize_locale(locale.as_ref());
  let key = format_woocraft_key(key.as_ref());
  lookup_rust_i18n_translation_merged(&locale, &key)
}

pub fn try_translate_woocraft(key: impl AsRef<str>) -> Option<String> {
  let locale = locale();
  try_translate_woocraft_in_locale(&*locale, key)
}

pub fn translate_woocraft_in_locale(locale: impl AsRef<str>, key: impl AsRef<str>) -> String {
  let locale = normalize_locale(locale.as_ref());
  let key = format_woocraft_key(key.as_ref());

  lookup_rust_i18n_translation_merged(&locale, &key)
    .unwrap_or_else(|| crate::_rust_i18n_translate(&locale, &key).into_owned())
}

pub fn translate_woocraft(key: impl AsRef<str>) -> String {
  let locale = locale();
  translate_woocraft_in_locale(&*locale, key)
}

/// Cache entry of the hot-path translation cache, keyed by
/// `(locale, key)`.
type CachedTranslation = (String, String, SharedString);

/// Cache of hot-path woocraft-domain translations, keyed by `(locale, key)`.
///
/// Entries are cleared whenever the active locale changes or custom
/// translations are (re)loaded, mirroring the per-locale invalidation of the
/// calendar's `LocaleCache`: a cached value can never outlive the translation
/// data it was built from.
static TRANSLATION_CACHE: LazyLock<RwLock<Vec<CachedTranslation>>> =
  LazyLock::new(|| RwLock::new(Vec::new()));

fn clear_translation_cache() {
  TRANSLATION_CACHE
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
    .clear();
}

/// Translates a woocraft-domain key in the active locale, caching the result
/// by `(locale, key)`.
///
/// Equivalent to [`translate_woocraft`] but returns a cheaply cloneable
/// [`SharedString`] and skips locale normalization, key formatting, and
/// fallback-chain lookups on cache hits, which makes it the entry point of
/// choice for render hot paths.
pub fn translate_static(key: &'static str) -> SharedString {
  let locale = locale();
  let locale: &str = &locale;

  {
    let cache = TRANSLATION_CACHE
      .read()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, _, value)) = cache
      .iter()
      .find(|(cached_locale, cached_key, _)| cached_locale == locale && cached_key == key)
    {
      return value.clone();
    }
  }

  let value = SharedString::from(translate_woocraft(key));
  TRANSLATION_CACHE
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
    .push((locale.to_string(), key.to_string(), value.clone()));
  value
}

pub fn try_translate_in_locale(locale: impl AsRef<str>, key: impl AsRef<str>) -> Option<String> {
  let locale = normalize_locale(locale.as_ref());
  let key = key.as_ref();

  // First, try to find the translation in the custom locale chain
  if let Some(value) = lookup_custom_translation_merged(&locale, key) {
    return Some(value);
  }

  // If not found in custom or rust_i18n fallbacks, try rust_i18n directly
  crate::_rust_i18n_try_translate(&locale, key).map(|value| value.into_owned())
}

pub fn try_translate(key: impl AsRef<str>) -> Option<String> {
  let locale = locale();
  try_translate_in_locale(&*locale, key)
}

pub fn translate_in_locale(locale: impl AsRef<str>, key: impl AsRef<str>) -> String {
  let locale = normalize_locale(locale.as_ref());
  let key = key.as_ref();

  // First, try to find the translation in the custom locale chain with
  // rust_i18n fallback
  if let Some(value) = lookup_custom_translation_merged(&locale, key) {
    return value;
  }

  // This should not be reached, as rust_i18n should always return something
  // (the key itself)
  crate::_rust_i18n_translate(&locale, key).into_owned()
}

pub fn translate(key: impl AsRef<str>) -> String {
  let locale = locale();
  translate_in_locale(&*locale, key)
}

pub fn locale_display_name(locale: impl AsRef<str>) -> String {
  let locale = normalize_locale(locale.as_ref());
  let builtin_key = woocraft_key_cow("i18n.name");

  // First try the built-in woocraft domain key
  if let Some(name) = lookup_rust_i18n_translation_merged(&locale, &builtin_key) {
    return name;
  }

  // Then try user-defined locale display names in custom locales
  if let Some(name) = lookup_custom_translation_merged(&locale, "i18n.name") {
    return name;
  }

  // If the locale has custom translations registered, use the locale code as
  // fallback
  let custom_locales = CUSTOM_LOCALES
    .read()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  if custom_locales.contains_key(&locale) {
    return locale;
  }
  drop(custom_locales);

  // Last resort: just return the locale code
  locale
}

/// Look up a translation with proper merging of custom and rust_i18n
/// translations.
///
/// This function implements a merged lookup strategy:
/// 1. Check custom locale chain first
/// 2. If not found, check rust_i18n for the same locale
/// 3. If found in rust_i18n, return it
/// 4. If not found, continue with fallback locale from rust_i18n
/// 5. This ensures that incomplete user translations don't hide built-in
///    translations
fn lookup_custom_translation_merged(locale: &str, key: &str) -> Option<String> {
  if is_woocraft_domain_key(key) {
    return lookup_rust_i18n_translation_merged(locale, key);
  }

  let custom_locales = CUSTOM_LOCALES
    .read()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  let mut current_locale = Some(locale.to_string());

  while let Some(locale_str) = current_locale {
    // First check if this locale has custom translations
    if let Some(translations) = custom_locales.get(&locale_str)
      && let Some(value) = translations.get(key)
    {
      return Some(value.clone());
    }

    if let Some(value) = crate::_rust_i18n_try_translate(&locale_str, key) {
      return Some(value.into_owned());
    }

    current_locale = crate::_rust_i18n_lookup_fallback(&locale_str).map(|s| s.to_string());
  }

  None
}

fn lookup_rust_i18n_translation_merged(locale: &str, key: &str) -> Option<String> {
  let mut current_locale = Some(locale.to_string());

  while let Some(locale_str) = current_locale {
    if let Some(value) = crate::_rust_i18n_try_translate(&locale_str, key) {
      return Some(value.into_owned());
    }

    current_locale = crate::_rust_i18n_lookup_fallback(&locale_str).map(|s| s.to_string());
  }

  None
}
#[cfg(test)]
mod tests {
  use std::sync::{LazyLock, Mutex, MutexGuard};

  use super::*;

  static TEST_LOCALE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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

  #[test]
  fn test_incomplete_custom_translation_merges_with_builtin() {
    let _guard = LocaleTestGuard::new();

    // Load only a partial Chinese (Simplified) translation
    let mut partial_translations = HashMap::new();
    partial_translations.insert("custom_key".to_string(), "自定义翻译".to_string());
    // Note: NOT adding built-in domain key to simulate incomplete translation

    load_locale("zh-hans", partial_translations);

    // Should return the custom translation for custom_key
    assert_eq!(translate_in_locale("zh-hans", "custom_key"), "自定义翻译");

    // Should Fall back to built-in translation for keys not in custom
    // translation (i18n.name exists in the built-in translation)
    let display_name = locale_display_name("zh-hans");
    assert!(!display_name.is_empty());
    // The display name should be a proper name, not "i18n.name" (the key
    // itself)
    assert_ne!(
      display_name, "i18n.name",
      "Should not return the key itself when merging with built-in translations"
    );
  }

  #[test]
  fn test_custom_translation_priority_over_builtin() {
    let _guard = LocaleTestGuard::new();

    let mut custom_translations = HashMap::new();
    custom_translations.insert("i18n.name".to_string(), "我的自定义语言名".to_string());
    custom_translations.insert("some_key".to_string(), "自定义值".to_string());

    load_locale("test-locale", custom_translations);

    // Custom translation should take priority
    assert_eq!(
      translate_in_locale("test-locale", "i18n.name"),
      "我的自定义语言名"
    );
    assert_eq!(translate_in_locale("test-locale", "some_key"), "自定义值");
  }

  #[test]
  fn test_custom_locale_cannot_override_woocraft_domain_key() {
    let _guard = LocaleTestGuard::new();

    let mut custom_translations = HashMap::new();
    custom_translations.insert(
      woocraft_key_cow("common.loading").into_owned(),
      "This should be ignored".to_string(),
    );

    load_locale("en-us", custom_translations);

    assert_eq!(
      translate_woocraft_in_locale("en-us", "common.loading"),
      "Loading..."
    );
  }

  #[test]
  fn test_extend_locale_preserves_builtin_translations() {
    let _guard = LocaleTestGuard::new();

    // Clear and extend with just a few keys
    let mut partial_translations = HashMap::new();
    partial_translations.insert("extended_key".to_string(), "扩展翻译".to_string());

    extend_locale("zh-hans", partial_translations);

    // Custom extended translation should be available
    assert_eq!(translate_in_locale("zh-hans", "extended_key"), "扩展翻译");
  }

  #[test]
  fn test_woocraft_key_borrows_domain_keys() {
    // The public entry always returns an owned `String`.
    assert_eq!(woocraft_key(WOOCRAFT_I18N_DOMAIN), WOOCRAFT_I18N_DOMAIN);
    assert_eq!(
      woocraft_key("common.loading"),
      "tech.woooo.woocraft.common.loading"
    );

    // The crate-internal Cow variant borrows already-prefixed static keys
    // without allocating.
    assert!(matches!(
      woocraft_key_cow(WOOCRAFT_I18N_DOMAIN),
      Cow::Borrowed(_)
    ));
    assert_eq!(
      woocraft_key_cow("common.loading"),
      "tech.woooo.woocraft.common.loading"
    );

    // Owned inputs pass through unchanged when already domain-prefixed.
    assert_eq!(
      woocraft_key_cow(String::from(WOOCRAFT_I18N_DOMAIN)),
      WOOCRAFT_I18N_DOMAIN
    );
  }

  #[test]
  fn test_translate_static_caches_per_locale() {
    let _guard = LocaleTestGuard::new();

    set_locale("en-us");
    assert_eq!(translate_static("pagination.previous"), "Previous");
    assert_eq!(TRANSLATION_CACHE.read().unwrap().len(), 1);

    // Repeated lookups hit the cache instead of growing it.
    assert_eq!(translate_static("pagination.previous"), "Previous");
    assert_eq!(TRANSLATION_CACHE.read().unwrap().len(), 1);

    // Switching the locale invalidates the cache and repopulates it.
    set_locale("zh-hans");
    assert_eq!(TRANSLATION_CACHE.read().unwrap().len(), 0);
    assert_eq!(translate_static("pagination.previous"), "上一页");
    assert_eq!(TRANSLATION_CACHE.read().unwrap().len(), 1);
  }

  #[test]
  fn test_translate_static_invalidates_on_custom_translations() {
    let _guard = LocaleTestGuard::new();

    set_locale("en-us");
    assert_eq!(translate_static("pagination.previous"), "Previous");
    assert_eq!(TRANSLATION_CACHE.read().unwrap().len(), 1);

    // Loading custom translation data clears the cache.
    let mut translations = HashMap::new();
    translations.insert("custom.cached".to_string(), "translated".to_string());
    load_locale("en-us", translations);
    assert_eq!(TRANSLATION_CACHE.read().unwrap().len(), 0);

    // And the next lookup repopulates it from the current data.
    assert_eq!(translate_static("pagination.previous"), "Previous");
    assert_eq!(TRANSLATION_CACHE.read().unwrap().len(), 1);
  }
}
