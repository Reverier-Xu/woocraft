use std::{
  collections::HashMap,
  ops::Deref,
  sync::{LazyLock, RwLock},
};

pub const SUPPORTED_LOCALES: [&str; 4] = ["zh-hans", "zh-hant", "en-us", "ja-jp"];

type LocaleTranslations = HashMap<String, String>;
type CustomLocaleStore = HashMap<String, LocaleTranslations>;

static CUSTOM_LOCALES: LazyLock<RwLock<CustomLocaleStore>> =
  LazyLock::new(|| RwLock::new(HashMap::new()));

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
}

pub fn available_locales() -> Vec<String> {
  let mut locales = SUPPORTED_LOCALES
    .iter()
    .map(|locale| locale.to_string())
    .collect::<Vec<_>>();

  for locale in rust_i18n::available_locales!() {
    let locale = normalize_locale(locale);
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
  CUSTOM_LOCALES
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
    .insert(locale, translations);
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
    locale_translations.insert(key.into(), value.into());
  }
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

  // First, try to find the translation in the custom locale chain with rust_i18n fallback
  if let Some(value) = lookup_custom_translation_merged(&locale, key) {
    return value;
  }

  // This should not be reached, as rust_i18n should always return something (the key itself)
  crate::_rust_i18n_translate(&locale, key).into_owned()
}

pub fn translate(key: impl AsRef<str>) -> String {
  let locale = locale();
  translate_in_locale(&*locale, key)
}

pub fn locale_display_name(locale: impl AsRef<str>) -> String {
  let locale = normalize_locale(locale.as_ref());
  
  // First try the merged lookup (custom + rust_i18n)
  if let Some(name) = lookup_custom_translation_merged(&locale, "i18n.name") {
    return name;
  }

  // If the locale has custom translations registered, use the locale code as fallback
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

/// Look up a translation with proper merging of custom and rust_i18n translations.
/// 
/// This function implements a merged lookup strategy:
/// 1. Check custom locale chain first
/// 2. If not found, check rust_i18n for the same locale
/// 3. If found in rust_i18n, return it
/// 4. If not found, continue with fallback locale from rust_i18n
/// 5. This ensures that incomplete user translations don't hide built-in translations
fn lookup_custom_translation_merged(locale: &str, key: &str) -> Option<String> {
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

    // If custom translation not found, try rust_i18n for this specific locale
    // This ensures built-in translations are used as fallback
    if let Some(value) = crate::_rust_i18n_try_translate(&locale_str, key) {
      return Some(value.into_owned());
    }

    // Move to the next locale in the fallback chain
    current_locale = crate::_rust_i18n_lookup_fallback(&locale_str).map(|s| s.to_string());
  }

  None
}
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_incomplete_custom_translation_merges_with_builtin() {
    // Load only a partial Chinese (Simplified) translation
    let mut partial_translations = HashMap::new();
    partial_translations.insert("custom_key".to_string(), "自定义翻译".to_string());
    // Note: NOT adding "i18n.name" to simulate incomplete translation

    load_locale("zh-hans", partial_translations);

    // Should return the custom translation for custom_key
    assert_eq!(translate_in_locale("zh-hans", "custom_key"), "自定义翻译");

    // Should Fall back to built-in translation for keys not in custom translation
    // (i18n.name exists in the built-in translation)
    let display_name = locale_display_name("zh-hans");
    assert!(!display_name.is_empty());
    // The display name should be a proper name, not "i18n.name" (the key itself)
    assert_ne!(display_name, "i18n.name", "Should not return the key itself when merging with built-in translations");
  }

  #[test]
  fn test_custom_translation_priority_over_builtin() {
    let mut custom_translations = HashMap::new();
    custom_translations.insert("i18n.name".to_string(), "我的自定义语言名".to_string());
    custom_translations.insert("some_key".to_string(), "自定义值".to_string());

    load_locale("test-locale", custom_translations);

    // Custom translation should take priority
    assert_eq!(translate_in_locale("test-locale", "i18n.name"), "我的自定义语言名");
    assert_eq!(translate_in_locale("test-locale", "some_key"), "自定义值");
  }

  #[test]
  fn test_extend_locale_preserves_builtin_translations() {
    // Clear and extend with just a few keys
    let mut partial_translations = HashMap::new();
    partial_translations.insert("extended_key".to_string(), "扩展翻译".to_string());

    extend_locale("zh-hans", partial_translations);

    // Custom extended translation should be available
    assert_eq!(translate_in_locale("zh-hans", "extended_key"), "扩展翻译");
  }
}