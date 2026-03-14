use gpui::SharedString;

use crate::widgets::editor::highlighter::LanguageConfig;

#[cfg(not(feature = "tree-sitter-languages"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, enum_iterator::Sequence)]
pub enum Language {
  Json,
}

#[cfg(feature = "tree-sitter-languages")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, enum_iterator::Sequence)]
pub enum Language {
  Agda,
  Json,
  Plain,
  Astro,
  Bash,
  C,
  CMake,
  CSharp,
  Cpp,
  Css,
  Diff,
  Ejs,
  Elixir,
  Erb,
  Go,
  GraphQL,
  Haskell,
  Html,
  Java,
  JavaScript,
  JsDoc,
  Julia,
  Kotlin,
  Lua,
  Make,
  Markdown,
  MarkdownInline,
  Ocaml,
  OcamlInterface,
  OcamlType,
  Php,
  Proto,
  Python,
  Ql,
  QlDbscheme,
  Razor,
  Regex,
  Ruby,
  Rust,
  Scala,
  Sql,
  Svelte,
  Swift,
  Toml,
  Tsx,
  TypeScript,
  Verilog,
  Yaml,
  Zig,
}

impl From<Language> for SharedString {
  fn from(language: Language) -> Self {
    language.name().into()
  }
}

impl std::str::FromStr for Language {
  type Err = std::convert::Infallible;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(Self::from_name(s))
  }
}

impl Language {
  pub fn all() -> impl Iterator<Item = Self> {
    enum_iterator::all::<Language>()
  }

  pub fn name(&self) -> &'static str {
    #[cfg(not(feature = "tree-sitter-languages"))]
    return "json";

    #[cfg(feature = "tree-sitter-languages")]
    match self {
      Self::Agda => "agda",
      Self::Plain => "text",
      Self::Astro => "astro",
      Self::Bash => "bash",
      Self::C => "c",
      Self::CMake => "cmake",
      Self::CSharp => "csharp",
      Self::Cpp => "cpp",
      Self::Css => "css",
      Self::Diff => "diff",
      Self::Ejs => "ejs",
      Self::Elixir => "elixir",
      Self::Erb => "erb",
      Self::Go => "go",
      Self::GraphQL => "graphql",
      Self::Haskell => "haskell",
      Self::Html => "html",
      Self::Java => "java",
      Self::JavaScript => "javascript",
      Self::JsDoc => "jsdoc",
      Self::Json => "json",
      Self::Julia => "julia",
      Self::Kotlin => "kotlin",
      Self::Lua => "lua",
      Self::Make => "make",
      Self::Markdown => "markdown",
      Self::MarkdownInline => "markdown_inline",
      Self::Ocaml => "ocaml",
      Self::OcamlInterface => "ocaml_interface",
      Self::OcamlType => "ocaml_type",
      Self::Php => "php",
      Self::Proto => "proto",
      Self::Python => "python",
      Self::Ql => "ql",
      Self::QlDbscheme => "ql_dbscheme",
      Self::Razor => "razor",
      Self::Regex => "regex",
      Self::Ruby => "ruby",
      Self::Rust => "rust",
      Self::Scala => "scala",
      Self::Sql => "sql",
      Self::Svelte => "svelte",
      Self::Swift => "swift",
      Self::Toml => "toml",
      Self::Tsx => "tsx",
      Self::TypeScript => "typescript",
      Self::Verilog => "verilog",
      Self::Yaml => "yaml",
      Self::Zig => "zig",
    }
  }

  pub fn from_name(s: &str) -> Self {
    #[cfg(not(feature = "tree-sitter-languages"))]
    return Self::Json;

    #[cfg(feature = "tree-sitter-languages")]
    match s {
      "agda" => Self::Agda,
      "astro" => Self::Astro,
      "bash" | "sh" => Self::Bash,
      "c" => Self::C,
      "cmake" => Self::CMake,
      "cpp" | "c++" => Self::Cpp,
      "csharp" | "cs" => Self::CSharp,
      "css" | "scss" => Self::Css,
      "diff" => Self::Diff,
      "ejs" => Self::Ejs,
      "elixir" | "ex" => Self::Elixir,
      "erb" => Self::Erb,
      "go" => Self::Go,
      "graphql" => Self::GraphQL,
      "haskell" | "hs" | "lhs" | "haskell_persistent" => Self::Haskell,
      "html" => Self::Html,
      "java" => Self::Java,
      "javascript" | "js" => Self::JavaScript,
      "jsdoc" => Self::JsDoc,
      "json" | "jsonc" => Self::Json,
      "julia" | "jl" => Self::Julia,
      "kt" | "kts" | "ktm" => Self::Kotlin,
      "lua" => Self::Lua,
      "make" | "makefile" => Self::Make,
      "markdown" | "md" | "mdx" => Self::Markdown,
      "markdown_inline" | "markdown-inline" => Self::MarkdownInline,
      "ocaml" | "ml" => Self::Ocaml,
      "ocaml_interface" | "ocaml-interface" | "mli" => Self::OcamlInterface,
      "ocaml_type" | "ocaml-type" => Self::OcamlType,
      "php" | "php3" | "php4" | "php5" | "phtml" => Self::Php,
      "proto" | "protobuf" => Self::Proto,
      "python" | "py" => Self::Python,
      "ql" => Self::Ql,
      "ql_dbscheme" | "ql-dbscheme" | "dbscheme" => Self::QlDbscheme,
      "razor" | "cshtml" => Self::Razor,
      "regex" | "regexp" => Self::Regex,
      "ruby" | "rb" => Self::Ruby,
      "rust" | "rs" => Self::Rust,
      "scala" => Self::Scala,
      "sql" => Self::Sql,
      "svelte" => Self::Svelte,
      "swift" => Self::Swift,
      "toml" => Self::Toml,
      "tsx" => Self::Tsx,
      "typescript" | "ts" => Self::TypeScript,
      "systemverilog" | "verilog" | "sv" | "svh" | "v" | "vh" => Self::Verilog,
      "yaml" | "yml" => Self::Yaml,
      "zig" => Self::Zig,
      _ => Self::Plain,
    }
  }

  #[allow(unused)]
  pub(super) fn injection_languages(&self) -> Vec<SharedString> {
    #[cfg(not(feature = "tree-sitter-languages"))]
    return vec![];

    #[cfg(feature = "tree-sitter-languages")]
    match self {
      Self::Haskell => vec![
        "css",
        "html",
        "javascript",
        "typescript",
        "json",
        "sql",
        "haskell_persistent",
      ],
      Self::Julia => vec!["markdown", "regex", "bash"],
      Self::Markdown => vec!["markdown-inline", "html", "toml", "yaml"],
      Self::MarkdownInline => vec![],
      Self::Html => vec!["javascript", "css"],
      Self::Rust => vec!["rust"],
      Self::JavaScript | Self::TypeScript => vec![
        "jsdoc",
        "json",
        "css",
        "html",
        "sql",
        "typescript",
        "javascript",
        "tsx",
        "yaml",
        "graphql",
      ],
      Self::Astro => vec!["html", "css", "javascript", "typescript"],
      Self::Php => vec![
        "php",
        "html",
        "css",
        "javascript",
        "json",
        "jsdoc",
        "graphql",
      ],
      Self::Svelte => vec!["svelte", "html", "css", "typescript"],
      _ => vec![],
    }
    .into_iter()
    .map(|s| s.into())
    .collect()
  }

  /// Return the language info for the language.
  ///
  /// (language, query, injection, locals)
  pub(super) fn config(&self) -> LanguageConfig {
    #[cfg(not(feature = "tree-sitter-languages"))]
    let (language, query, injection, locals) = match self {
      Self::Json => (
        tree_sitter_json::LANGUAGE.into(),
        include_str!("languages/json/highlights.scm"),
        "",
        "",
      ),
    };

    #[cfg(feature = "tree-sitter-languages")]
    let (language, query, injection, locals) = match self {
      Self::Agda => (
        tree_sitter_agda::LANGUAGE.into(),
        tree_sitter_agda::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::Plain => (tree_sitter_json::LANGUAGE.into(), "", "", ""),
      Self::Json => (
        tree_sitter_json::LANGUAGE.into(),
        include_str!("languages/json/highlights.scm"),
        "",
        "",
      ),
      Self::Markdown => (
        tree_sitter_md::LANGUAGE.into(),
        include_str!("languages/markdown/highlights.scm"),
        include_str!("languages/markdown/injections.scm"),
        "",
      ),
      Self::MarkdownInline => (
        tree_sitter_md::INLINE_LANGUAGE.into(),
        include_str!("languages/markdown_inline/highlights.scm"),
        "",
        "",
      ),
      Self::Toml => (
        tree_sitter_toml_ng::LANGUAGE.into(),
        tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::Yaml => (
        tree_sitter_yaml::LANGUAGE.into(),
        tree_sitter_yaml::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::Rust => (
        tree_sitter_rust::LANGUAGE.into(),
        include_str!("languages/rust/highlights.scm"),
        include_str!("languages/rust/injections.scm"),
        "",
      ),
      Self::Go => (
        tree_sitter_go::LANGUAGE.into(),
        include_str!("languages/go/highlights.scm"),
        "",
        "",
      ),
      Self::C => (
        tree_sitter_c::LANGUAGE.into(),
        tree_sitter_c::HIGHLIGHT_QUERY,
        "",
        "",
      ),
      Self::Cpp => (
        tree_sitter_cpp::LANGUAGE.into(),
        tree_sitter_cpp::HIGHLIGHT_QUERY,
        "",
        "",
      ),
      Self::JavaScript => (
        tree_sitter_javascript::LANGUAGE.into(),
        include_str!("languages/javascript/highlights.scm"),
        include_str!("languages/javascript/injections.scm"),
        tree_sitter_javascript::LOCALS_QUERY,
      ),
      Self::JsDoc => (
        tree_sitter_jsdoc::LANGUAGE.into(),
        tree_sitter_jsdoc::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::Zig => (
        tree_sitter_zig::LANGUAGE.into(),
        include_str!("languages/zig/highlights.scm"),
        include_str!("languages/zig/injections.scm"),
        "",
      ),
      Self::Java => (
        tree_sitter_java::LANGUAGE.into(),
        tree_sitter_java::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::Python => (
        tree_sitter_python::LANGUAGE.into(),
        tree_sitter_python::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::Ruby => (
        tree_sitter_ruby::LANGUAGE.into(),
        tree_sitter_ruby::HIGHLIGHTS_QUERY,
        "",
        tree_sitter_ruby::LOCALS_QUERY,
      ),
      Self::Bash => (
        tree_sitter_bash::LANGUAGE.into(),
        tree_sitter_bash::HIGHLIGHT_QUERY,
        "",
        "",
      ),
      Self::Haskell => (
        tree_sitter_haskell::LANGUAGE.into(),
        tree_sitter_haskell::HIGHLIGHTS_QUERY,
        tree_sitter_haskell::INJECTIONS_QUERY,
        tree_sitter_haskell::LOCALS_QUERY,
      ),
      Self::Html => (
        tree_sitter_html::LANGUAGE.into(),
        include_str!("languages/html/highlights.scm"),
        include_str!("languages/html/injections.scm"),
        "",
      ),
      Self::Css => (
        tree_sitter_css::LANGUAGE.into(),
        tree_sitter_css::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::Julia => (
        tree_sitter_julia::LANGUAGE.into(),
        include_str!("languages/julia/highlights.scm"),
        include_str!("languages/julia/injections.scm"),
        include_str!("languages/julia/locals.scm"),
      ),
      Self::Swift => (tree_sitter_swift::LANGUAGE.into(), "", "", ""),
      Self::Scala => (
        tree_sitter_scala::LANGUAGE.into(),
        tree_sitter_scala::HIGHLIGHTS_QUERY,
        "",
        tree_sitter_scala::LOCALS_QUERY,
      ),
      Self::Sql => (
        tree_sitter_sequel::LANGUAGE.into(),
        tree_sitter_sequel::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::CSharp => (tree_sitter_c_sharp::LANGUAGE.into(), "", "", ""),
      Self::GraphQL => (tree_sitter_graphql::LANGUAGE.into(), "", "", ""),
      Self::Ocaml => (
        tree_sitter_ocaml::LANGUAGE_OCAML.into(),
        tree_sitter_ocaml::HIGHLIGHTS_QUERY,
        "",
        tree_sitter_ocaml::LOCALS_QUERY,
      ),
      Self::OcamlInterface => (
        tree_sitter_ocaml::LANGUAGE_OCAML_INTERFACE.into(),
        tree_sitter_ocaml::HIGHLIGHTS_QUERY,
        "",
        tree_sitter_ocaml::LOCALS_QUERY,
      ),
      Self::OcamlType => (
        tree_sitter_ocaml::LANGUAGE_OCAML_TYPE.into(),
        tree_sitter_ocaml::HIGHLIGHTS_QUERY,
        "",
        tree_sitter_ocaml::LOCALS_QUERY,
      ),
      Self::Php => (
        tree_sitter_php::LANGUAGE_PHP.into(),
        tree_sitter_php::HIGHLIGHTS_QUERY,
        include_str!("languages/php/injections.scm"),
        "",
      ),
      Self::Proto => (tree_sitter_proto::LANGUAGE.into(), "", "", ""),
      Self::Make => (
        tree_sitter_make::LANGUAGE.into(),
        tree_sitter_make::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::Ql => (
        tree_sitter_ql::LANGUAGE.into(),
        tree_sitter_ql::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::QlDbscheme => (tree_sitter_ql_dbscheme::LANGUAGE.into(), "", "", ""),
      Self::Razor => (tree_sitter_razor::LANGUAGE.into(), "", "", ""),
      Self::Regex => (
        tree_sitter_regex::LANGUAGE.into(),
        tree_sitter_regex::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::CMake => (tree_sitter_cmake::LANGUAGE.into(), "", "", ""),
      Self::TypeScript => (
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        include_str!("languages/typescript/highlights.scm"),
        include_str!("languages/javascript/injections.scm"),
        tree_sitter_typescript::LOCALS_QUERY,
      ),
      Self::Tsx => (
        tree_sitter_typescript::LANGUAGE_TSX.into(),
        tree_sitter_typescript::HIGHLIGHTS_QUERY,
        "",
        tree_sitter_typescript::LOCALS_QUERY,
      ),
      Self::Diff => (
        tree_sitter_diff::LANGUAGE.into(),
        tree_sitter_diff::HIGHLIGHTS_QUERY,
        "",
        "",
      ),
      Self::Elixir => (
        tree_sitter_elixir::LANGUAGE.into(),
        tree_sitter_elixir::HIGHLIGHTS_QUERY,
        tree_sitter_elixir::INJECTIONS_QUERY,
        "",
      ),
      Self::Erb => (
        tree_sitter_embedded_template::LANGUAGE.into(),
        tree_sitter_embedded_template::HIGHLIGHTS_QUERY,
        tree_sitter_embedded_template::INJECTIONS_EJS_QUERY,
        "",
      ),
      Self::Ejs => (
        tree_sitter_embedded_template::LANGUAGE.into(),
        tree_sitter_embedded_template::HIGHLIGHTS_QUERY,
        tree_sitter_embedded_template::INJECTIONS_EJS_QUERY,
        "",
      ),
      Self::Astro => (
        tree_sitter_astro_next::LANGUAGE.into(),
        tree_sitter_astro_next::HIGHLIGHTS_QUERY,
        tree_sitter_astro_next::INJECTIONS_QUERY,
        "",
      ),
      Self::Kotlin => (
        tree_sitter_kotlin_sg::LANGUAGE.into(),
        include_str!("languages/kotlin/highlights.scm"),
        "",
        "",
      ),
      Self::Lua => (
        tree_sitter_lua::LANGUAGE.into(),
        include_str!("languages/lua/highlights.scm"),
        tree_sitter_lua::INJECTIONS_QUERY,
        tree_sitter_lua::LOCALS_QUERY,
      ),
      Self::Svelte => (
        tree_sitter_svelte_next::LANGUAGE.into(),
        tree_sitter_svelte_next::HIGHLIGHTS_QUERY,
        tree_sitter_svelte_next::INJECTIONS_QUERY,
        tree_sitter_svelte_next::LOCALS_QUERY,
      ),
      Self::Verilog => (tree_sitter_verilog::LANGUAGE.into(), "", "", ""),
    };

    LanguageConfig::new(
      self.name(),
      language,
      self.injection_languages(),
      query,
      injection,
      locals,
    )
  }
}

#[cfg(test)]
mod tests {
  #[test]
  #[cfg(feature = "tree-sitter-languages")]
  fn test_language_name() {
    use super::*;

    assert_eq!(Language::Agda.name(), "agda");
    assert_eq!(Language::Haskell.name(), "haskell");
    assert_eq!(Language::MarkdownInline.name(), "markdown_inline");
    assert_eq!(Language::Markdown.name(), "markdown");
    assert_eq!(Language::Json.name(), "json");
    assert_eq!(Language::Julia.name(), "julia");
    assert_eq!(Language::Ocaml.name(), "ocaml");
    assert_eq!(Language::OcamlInterface.name(), "ocaml_interface");
    assert_eq!(Language::OcamlType.name(), "ocaml_type");
    assert_eq!(Language::Ql.name(), "ql");
    assert_eq!(Language::QlDbscheme.name(), "ql_dbscheme");
    assert_eq!(Language::Razor.name(), "razor");
    assert_eq!(Language::Regex.name(), "regex");
    assert_eq!(Language::Yaml.name(), "yaml");
    assert_eq!(Language::Verilog.name(), "verilog");
    assert_eq!(Language::Rust.name(), "rust");
    assert_eq!(Language::Go.name(), "go");
    assert_eq!(Language::C.name(), "c");
    assert_eq!(Language::Cpp.name(), "cpp");
    assert_eq!(Language::Sql.name(), "sql");
    assert_eq!(Language::JavaScript.name(), "javascript");
    assert_eq!(Language::Zig.name(), "zig");
    assert_eq!(Language::CSharp.name(), "csharp");
    assert_eq!(Language::TypeScript.name(), "typescript");
    assert_eq!(Language::Tsx.name(), "tsx");
    assert_eq!(Language::Diff.name(), "diff");
    assert_eq!(Language::Elixir.name(), "elixir");
    assert_eq!(Language::Erb.name(), "erb");
    assert_eq!(Language::Ejs.name(), "ejs");
  }

  #[test]
  #[cfg(feature = "tree-sitter-languages")]
  fn test_language_aliases() {
    use super::*;

    assert_eq!(Language::from_name("hs"), Language::Haskell);
    assert_eq!(Language::from_name("jl"), Language::Julia);
    assert_eq!(Language::from_name("mli"), Language::OcamlInterface);
    assert_eq!(Language::from_name("ocaml-type"), Language::OcamlType);
    assert_eq!(Language::from_name("ql-dbscheme"), Language::QlDbscheme);
    assert_eq!(Language::from_name("cshtml"), Language::Razor);
    assert_eq!(Language::from_name("regexp"), Language::Regex);
    assert_eq!(Language::from_name("sv"), Language::Verilog);
  }
}
