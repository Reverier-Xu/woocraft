//! The player event contract.
//!
//! This module carries the event stream and the property payload it names,
//! shared by [`player`](super::player) and [`video`](super::video) so the
//! video output can report failures on the player's channel without depending
//! on the player entity itself.

/// What a player reports to its views.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerEvent {
  /// A file was loaded and playback may start.
  FileLoaded,
  /// Playback of the current entry ended (or failed).
  EndFile {
    /// The reason mpv reported, for example `eof` or `error`.
    reason: String,
  },
  /// An observed property changed.
  PropertyChanged {
    /// The mpv property name, for example `time-pos`.
    name: String,
    /// Its new value.
    value: PropertyValue,
  },
  /// The video track was (re)configured.
  VideoReconfig,
  /// The audio track was (re)configured.
  AudioReconfig,
  /// mpv logged an error.
  Error(String),
}

/// A scalar mpv property value.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
  /// A string property.
  String(String),
  /// A boolean property.
  Flag(bool),
  /// A 64-bit integer property.
  Int64(i64),
  /// A floating point property.
  Double(f64),
}

impl From<String> for PropertyValue {
  fn from(value: String) -> Self {
    Self::String(value)
  }
}

impl From<&str> for PropertyValue {
  fn from(value: &str) -> Self {
    Self::String(value.to_string())
  }
}

impl From<bool> for PropertyValue {
  fn from(value: bool) -> Self {
    Self::Flag(value)
  }
}

impl From<i64> for PropertyValue {
  fn from(value: i64) -> Self {
    Self::Int64(value)
  }
}

impl From<f64> for PropertyValue {
  fn from(value: f64) -> Self {
    Self::Double(value)
  }
}
