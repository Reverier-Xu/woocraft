use std::ops::Deref;

pub const SUPPORTED_LOCALES: [&str; 4] = ["zh-hans", "zh-hant", "en-us", "ja-jp"];

fn normalize_locale(locale: &str) -> &'static str {
  let normalized = locale.to_ascii_lowercase().replace('_', "-");

  if normalized.starts_with("zh-hans") || normalized.starts_with("zh-cn") {
    "zh-hans"
  } else if normalized.starts_with("zh-hant")
    || normalized.starts_with("zh-tw")
    || normalized.starts_with("zh-hk")
  {
    "zh-hant"
  } else if normalized.starts_with("ja") || normalized.starts_with("jp") {
    "ja-jp"
  } else {
    "en-us"
  }
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
  rust_i18n::set_locale(normalize_locale(locale));
}
