//! collision-resistant ids for library-managed content.
//!
//! toasts, dialog stack entries, action items, dock panels — anything the
//! library or an application keys internally mints ids through this one
//! generator instead of inventing per-component schemes. the format is
//! `<prefix>-` followed by 21 random lowercase alphanumeric characters,
//! e.g. `toast-24ugbb9qg5533qhg235`; the prefix names the owning component.

use gpui::SharedString;

/// the alphabet of the random segment: digits and lowercase letters.
const ID_ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// length of the random segment.
const ID_RANDOM_LEN: usize = 21;

/// generates an id for library-managed content: `prefix-` plus 21 random
/// lowercase alphanumeric characters.
///
/// the random source is process-seeded (`fastrand`); ids are unique for
/// practical purposes, not cryptographically distributed.
pub fn new_id(prefix: &str) -> SharedString {
  let mut id = String::with_capacity(prefix.len() + 1 + ID_RANDOM_LEN);
  id.push_str(prefix);
  id.push('-');
  let mut bytes = [0u8; ID_RANDOM_LEN];
  fastrand::fill(&mut bytes);
  for byte in bytes {
    id.push(ID_ALPHABET[usize::from(byte) % ID_ALPHABET.len()] as char);
  }
  id.into()
}

#[cfg(test)]
mod tests {
  use std::collections::HashSet;

  use super::{ID_RANDOM_LEN, new_id};

  #[test]
  fn the_id_carries_the_prefix_and_a_21_char_random_segment() {
    let id = new_id("toast");
    let id: &str = id.as_ref();
    assert!(id.starts_with("toast-"));
    let segment = &id["toast-".len()..];
    assert_eq!(segment.len(), ID_RANDOM_LEN);
    assert!(
      segment
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit()),
      "the random segment is lowercase alphanumeric: {segment}"
    );
  }

  #[test]
  fn ids_do_not_collide() {
    let ids: HashSet<_> = (0..10_000).map(|_| new_id("toast")).collect();
    assert_eq!(ids.len(), 10_000);
  }
}
