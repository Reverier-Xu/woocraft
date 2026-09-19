//! crate-wide error and result types.
//!
//! every fallible infrastructure operation in woocraft surfaces through
//! [`Error`]. the gpui runtime reports failures as opaque messages, so font
//! registration preserves the rendered message instead of a source chain.

use std::path::PathBuf;

/// convenience alias for results in this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// errors surfaced by woocraft infrastructure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// a file could not be read from disk.
  #[error("failed to read `{path}`")]
  Io {
    /// file system path that could not be read.
    path: PathBuf,
    /// underlying i/o failure.
    #[source]
    source: std::io::Error,
  },

  /// theme tokens are not well-formed json.
  #[error("invalid theme tokens (json): {0}")]
  Json(#[from] serde_json::Error),

  /// theme tokens are not well-formed toml.
  #[error("invalid theme tokens (toml): {0}")]
  Toml(#[from] toml::de::Error),

  /// an embedded zstd-compressed asset failed to decompress.
  #[error("failed to decompress embedded asset `{asset}`")]
  AssetDecompress {
    /// embedded path of the asset that failed to decompress.
    asset: String,
    /// underlying decompression failure.
    #[source]
    source: std::io::Error,
  },

  /// the gpui text system rejected embedded font data.
  #[error("failed to register embedded fonts: {0}")]
  FontRegistration(String),
}

#[cfg(test)]
mod tests {
  use std::error::Error as _;

  use super::Error;

  #[test]
  fn io_error_keeps_path_and_source() {
    let source = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let error = Error::Io {
      path: "/tmp/tokens.toml".into(),
      source,
    };

    assert!(error.to_string().contains("/tmp/tokens.toml"));
    assert!(error.source().is_some());
  }

  #[test]
  fn decompress_error_names_the_asset() {
    let source = std::io::Error::other("bad zstd frame");
    let error = Error::AssetDecompress {
      asset: "fonts/maple-mono-regular.ttf.zst".into(),
      source,
    };

    assert!(error.to_string().contains("maple-mono-regular.ttf.zst"));
    assert!(error.source().is_some());
  }
}
