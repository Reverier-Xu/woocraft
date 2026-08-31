//! GPUI-free value types shared between the terminal core and its views.
//!
//! These types mirror the shape of the alacritty backend state but are owned by
//! this crate so that view layers and external controllers never need to touch
//! `alacritty_terminal` directly.

use std::{cmp::Ordering, sync::Arc};

use alacritty_terminal::{
  grid::Dimensions as AlacDimensions,
  index::{Column, Line, Point as AlacPoint},
  term::{TermMode, cell::Flags as AlacFlags},
};
pub use vte::ansi::{Color as CellColor, NamedColor, Rgb};

/// A `line`/`column` position inside the terminal grid.
///
/// `line` is relative to the visible viewport (`0` is the topmost visible
/// line, negative values reach into the scrollback history).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
  pub line: i32,
  pub column: usize,
}

impl Point {
  pub const fn new(line: i32, column: usize) -> Self {
    Self { line, column }
  }
}

impl PartialOrd for Point {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Point {
  fn cmp(&self, other: &Self) -> Ordering {
    (self.line, self.column).cmp(&(other.line, other.column))
  }
}

impl Point {
  pub(crate) fn to_alacritty(self) -> AlacPoint {
    AlacPoint::new(Line(self.line), Column(self.column))
  }

  pub(crate) fn from_alacritty(point: AlacPoint) -> Self {
    Self {
      line: point.line.0,
      column: point.column.0,
    }
  }
}

/// An inclusive range of grid points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
  start: Point,
  end: Point,
}

impl Range {
  pub const fn new(start: Point, end: Point) -> Self {
    Self { start, end }
  }

  pub const fn start(&self) -> Point {
    self.start
  }

  pub const fn end(&self) -> Point {
    self.end
  }

  pub fn contains(&self, point: Point) -> bool {
    self.start <= point && point <= self.end
  }
}

/// The semantics of a user selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionKind {
  /// Character-wise selection, as produced by dragging the mouse.
  #[default]
  Characters,
  /// Word-wise selection, as produced by double-clicking and dragging.
  Words,
  /// Whole-line selection, as produced by triple-clicking.
  Lines,
  /// Rectangular block selection, as produced by alt-dragging.
  Block,
}

/// The user selection currently rendered on the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
  pub start: Point,
  pub end: Point,
  pub is_block: bool,
}

impl SelectionRange {
  pub fn point_range(self) -> Range {
    Range::new(self.start, self.end)
  }
}

/// Cell-level style flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CellFlags(u16);

impl CellFlags {
  pub const INVERSE: Self = Self(1 << 0);
  pub const BOLD: Self = Self(1 << 1);
  pub const ITALIC: Self = Self(1 << 2);
  pub const UNDERLINE: Self = Self(1 << 3);
  pub const WRAPLINE: Self = Self(1 << 4);
  pub const WIDE_CHAR: Self = Self(1 << 5);
  pub const WIDE_CHAR_SPACER: Self = Self(1 << 6);
  pub const DIM: Self = Self(1 << 7);
  pub const HIDDEN: Self = Self(1 << 8);
  pub const STRIKEOUT: Self = Self(1 << 9);
  pub const LEADING_WIDE_CHAR_SPACER: Self = Self(1 << 10);
  pub const DOUBLE_UNDERLINE: Self = Self(1 << 11);
  pub const UNDERCURL: Self = Self(1 << 12);
  pub const DOTTED_UNDERLINE: Self = Self(1 << 13);
  pub const DASHED_UNDERLINE: Self = Self(1 << 14);

  pub const UNDERLINE_KINDS: Self = Self(
    Self::UNDERLINE.0
      | Self::DOUBLE_UNDERLINE.0
      | Self::UNDERCURL.0
      | Self::DOTTED_UNDERLINE.0
      | Self::DASHED_UNDERLINE.0,
  );

  pub const fn empty() -> Self {
    Self(0)
  }

  pub const fn bits(self) -> u16 {
    self.0
  }

  pub const fn is_empty(self) -> bool {
    self.0 == 0
  }

  pub const fn contains(self, other: Self) -> bool {
    self.0 & other.0 == other.0
  }

  pub const fn intersects(self, other: Self) -> bool {
    self.0 & other.0 != 0
  }

  pub const fn insert(&mut self, other: Self) {
    self.0 |= other.0;
  }

  pub const fn remove(&mut self, other: Self) {
    self.0 &= !other.0;
  }

  /// The union of two flag sets.
  pub const fn union(self, other: Self) -> Self {
    Self(self.0 | other.0)
  }

  pub(crate) const fn from_alacritty(flags: AlacFlags) -> Self {
    let mut out = Self::empty();
    if flags.intersects(AlacFlags::INVERSE) {
      out.insert(Self::INVERSE);
    }
    if flags.intersects(AlacFlags::BOLD) {
      out.insert(Self::BOLD);
    }
    if flags.intersects(AlacFlags::ITALIC) {
      out.insert(Self::ITALIC);
    }
    if flags.intersects(AlacFlags::UNDERLINE) {
      out.insert(Self::UNDERLINE);
    }
    if flags.intersects(AlacFlags::WRAPLINE) {
      out.insert(Self::WRAPLINE);
    }
    if flags.intersects(AlacFlags::WIDE_CHAR) {
      out.insert(Self::WIDE_CHAR);
    }
    if flags.intersects(AlacFlags::WIDE_CHAR_SPACER) {
      out.insert(Self::WIDE_CHAR_SPACER);
    }
    if flags.intersects(AlacFlags::DIM) {
      out.insert(Self::DIM);
    }
    if flags.intersects(AlacFlags::HIDDEN) {
      out.insert(Self::HIDDEN);
    }
    if flags.intersects(AlacFlags::STRIKEOUT) {
      out.insert(Self::STRIKEOUT);
    }
    if flags.intersects(AlacFlags::LEADING_WIDE_CHAR_SPACER) {
      out.insert(Self::LEADING_WIDE_CHAR_SPACER);
    }
    if flags.intersects(AlacFlags::DOUBLE_UNDERLINE) {
      out.insert(Self::DOUBLE_UNDERLINE);
    }
    if flags.intersects(AlacFlags::UNDERCURL) {
      out.insert(Self::UNDERCURL);
    }
    if flags.intersects(AlacFlags::DOTTED_UNDERLINE) {
      out.insert(Self::DOTTED_UNDERLINE);
    }
    if flags.intersects(AlacFlags::DASHED_UNDERLINE) {
      out.insert(Self::DASHED_UNDERLINE);
    }
    out
  }
}

/// A single terminal cell with its resolved style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
  pub c: char,
  pub fg: CellColor,
  pub bg: CellColor,
  pub flags: CellFlags,
  pub hyperlink: Option<Arc<str>>,
  /// Zero-width characters attached to this cell (combining marks).
  pub zerowidth: Vec<char>,
}

impl Default for Cell {
  fn default() -> Self {
    Self {
      c: ' ',
      fg: CellColor::Named(NamedColor::Foreground),
      bg: CellColor::Named(NamedColor::Background),
      flags: CellFlags::empty(),
      hyperlink: None,
      zerowidth: Vec::new(),
    }
  }
}

/// A cell paired with its grid position, as produced by [`Content`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedCell {
  pub point: Point,
  pub cell: Cell,
}

/// Terminal modes that influence input handling and rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modes(u32);

impl Modes {
  pub const NONE: Self = Self(0);
  pub const APP_CURSOR: Self = Self(1 << 0);
  pub const APP_KEYPAD: Self = Self(1 << 1);
  pub const SHOW_CURSOR: Self = Self(1 << 2);
  pub const LINE_WRAP: Self = Self(1 << 3);
  pub const ORIGIN: Self = Self(1 << 4);
  pub const INSERT: Self = Self(1 << 5);
  pub const LINE_FEED_NEW_LINE: Self = Self(1 << 6);
  pub const FOCUS_IN_OUT: Self = Self(1 << 7);
  pub const ALTERNATE_SCROLL: Self = Self(1 << 8);
  pub const BRACKETED_PASTE: Self = Self(1 << 9);
  pub const SGR_MOUSE: Self = Self(1 << 10);
  pub const UTF8_MOUSE: Self = Self(1 << 11);
  pub const ALT_SCREEN: Self = Self(1 << 12);
  pub const MOUSE_REPORT_CLICK: Self = Self(1 << 13);
  pub const MOUSE_DRAG: Self = Self(1 << 14);
  pub const MOUSE_MOTION: Self = Self(1 << 15);
  pub const VI: Self = Self(1 << 16);
  pub const DISAMBIGUATE_ESC_CODES: Self = Self(1 << 17);
  pub const REPORT_EVENT_TYPES: Self = Self(1 << 18);
  pub const REPORT_ALTERNATE_KEYS: Self = Self(1 << 19);
  pub const REPORT_ALL_KEYS_AS_ESC: Self = Self(1 << 20);
  pub const REPORT_ASSOCIATED_TEXT: Self = Self(1 << 21);
  pub const MOUSE_MODE: Self =
    Self(Self::MOUSE_REPORT_CLICK.0 | Self::MOUSE_DRAG.0 | Self::MOUSE_MOTION.0);
  pub const KITTY_KEYBOARD_PROTOCOL: Self = Self(
    Self::DISAMBIGUATE_ESC_CODES.0
      | Self::REPORT_EVENT_TYPES.0
      | Self::REPORT_ALTERNATE_KEYS.0
      | Self::REPORT_ALL_KEYS_AS_ESC.0
      | Self::REPORT_ASSOCIATED_TEXT.0,
  );

  pub const fn empty() -> Self {
    Self::NONE
  }

  pub const fn bits(self) -> u32 {
    self.0
  }

  pub const fn is_empty(self) -> bool {
    self.0 == 0
  }

  pub const fn contains(self, other: Self) -> bool {
    self.0 & other.0 == other.0
  }

  pub const fn intersects(self, other: Self) -> bool {
    self.0 & other.0 != 0
  }

  pub const fn insert(&mut self, other: Self) {
    self.0 |= other.0;
  }

  pub const fn remove(&mut self, other: Self) {
    self.0 &= !other.0;
  }

  pub(crate) const fn from_term_mode(mode: TermMode) -> Self {
    let mut out = Self::NONE;
    if mode.intersects(TermMode::APP_CURSOR) {
      out.insert(Self::APP_CURSOR);
    }
    if mode.intersects(TermMode::APP_KEYPAD) {
      out.insert(Self::APP_KEYPAD);
    }
    if mode.intersects(TermMode::SHOW_CURSOR) {
      out.insert(Self::SHOW_CURSOR);
    }
    if mode.intersects(TermMode::LINE_WRAP) {
      out.insert(Self::LINE_WRAP);
    }
    if mode.intersects(TermMode::ORIGIN) {
      out.insert(Self::ORIGIN);
    }
    if mode.intersects(TermMode::INSERT) {
      out.insert(Self::INSERT);
    }
    if mode.intersects(TermMode::LINE_FEED_NEW_LINE) {
      out.insert(Self::LINE_FEED_NEW_LINE);
    }
    if mode.intersects(TermMode::FOCUS_IN_OUT) {
      out.insert(Self::FOCUS_IN_OUT);
    }
    if mode.intersects(TermMode::ALTERNATE_SCROLL) {
      out.insert(Self::ALTERNATE_SCROLL);
    }
    if mode.intersects(TermMode::BRACKETED_PASTE) {
      out.insert(Self::BRACKETED_PASTE);
    }
    if mode.intersects(TermMode::SGR_MOUSE) {
      out.insert(Self::SGR_MOUSE);
    }
    if mode.intersects(TermMode::UTF8_MOUSE) {
      out.insert(Self::UTF8_MOUSE);
    }
    if mode.intersects(TermMode::ALT_SCREEN) {
      out.insert(Self::ALT_SCREEN);
    }
    if mode.intersects(TermMode::MOUSE_REPORT_CLICK) {
      out.insert(Self::MOUSE_REPORT_CLICK);
    }
    if mode.intersects(TermMode::MOUSE_DRAG) {
      out.insert(Self::MOUSE_DRAG);
    }
    if mode.intersects(TermMode::MOUSE_MOTION) {
      out.insert(Self::MOUSE_MOTION);
    }
    if mode.intersects(TermMode::VI) {
      out.insert(Self::VI);
    }
    if mode.intersects(TermMode::DISAMBIGUATE_ESC_CODES) {
      out.insert(Self::DISAMBIGUATE_ESC_CODES);
    }
    if mode.intersects(TermMode::REPORT_EVENT_TYPES) {
      out.insert(Self::REPORT_EVENT_TYPES);
    }
    if mode.intersects(TermMode::REPORT_ALTERNATE_KEYS) {
      out.insert(Self::REPORT_ALTERNATE_KEYS);
    }
    if mode.intersects(TermMode::REPORT_ALL_KEYS_AS_ESC) {
      out.insert(Self::REPORT_ALL_KEYS_AS_ESC);
    }
    if mode.intersects(TermMode::REPORT_ASSOCIATED_TEXT) {
      out.insert(Self::REPORT_ASSOCIATED_TEXT);
    }
    out
  }
}

impl std::ops::BitOr for Modes {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self {
    Self(self.0 | rhs.0)
  }
}

impl std::ops::BitOrAssign for Modes {
  fn bitor_assign(&mut self, rhs: Self) {
    self.insert(rhs);
  }
}

impl std::ops::BitOr for CellFlags {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self {
    Self(self.0 | rhs.0)
  }
}

impl std::ops::BitOrAssign for CellFlags {
  fn bitor_assign(&mut self, rhs: Self) {
    self.0 |= rhs.0;
  }
}

/// The shape of the terminal cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorShape {
  Block,
  Underline,
  Bar,
  HollowBlock,
  Hidden,
}

/// The cursor position and shape, as of a content snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
  pub shape: CursorShape,
  pub point: Point,
}

/// A scrolling request against the terminal display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollKind {
  /// Scroll by `n` lines; positive scrolls towards the history.
  Delta(i32),
  PageUp,
  PageDown,
  Top,
  Bottom,
}

impl ScrollKind {
  pub(crate) fn to_alacritty(self) -> alacritty_terminal::grid::Scroll {
    match self {
      ScrollKind::Delta(delta) => alacritty_terminal::grid::Scroll::Delta(delta),
      ScrollKind::PageUp => alacritty_terminal::grid::Scroll::PageUp,
      ScrollKind::PageDown => alacritty_terminal::grid::Scroll::PageDown,
      ScrollKind::Top => alacritty_terminal::grid::Scroll::Top,
      ScrollKind::Bottom => alacritty_terminal::grid::Scroll::Bottom,
    }
  }
}

/// The geometry of the terminal viewport, in pixels and cells.
///
/// This is the shared contract between the view layer (which measures fonts)
/// and the session (which resizes the PTY and the emulator grid).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalBounds {
  line_height: f32,
  cell_width: f32,
  columns: usize,
  lines: usize,
}

impl TerminalBounds {
  /// Creates new bounds; values are normalized to at least one line/column
  /// and one pixel per cell.
  pub const fn new(line_height: f32, cell_width: f32, columns: usize, lines: usize) -> Self {
    Self {
      line_height: if line_height < 1.0 { 1.0 } else { line_height },
      cell_width: if cell_width < 1.0 { 1.0 } else { cell_width },
      columns: if columns < 1 { 1 } else { columns },
      lines: if lines < 1 { 1 } else { lines },
    }
  }

  pub const fn line_height(&self) -> f32 {
    self.line_height
  }

  pub const fn cell_width(&self) -> f32 {
    self.cell_width
  }

  pub const fn num_columns(&self) -> usize {
    self.columns
  }

  pub const fn num_lines(&self) -> usize {
    self.lines
  }

  pub const fn width(&self) -> f32 {
    self.cell_width * self.columns as f32
  }

  pub const fn height(&self) -> f32 {
    self.line_height * self.lines as f32
  }

  pub(crate) fn to_window_size(self) -> alacritty_terminal::event::WindowSize {
    alacritty_terminal::event::WindowSize {
      num_lines: self.lines as u16,
      num_cols: self.columns as u16,
      cell_width: self.cell_width as u16,
      cell_height: self.line_height as u16,
    }
  }
}

impl Default for TerminalBounds {
  fn default() -> Self {
    Self::new(20.0, 10.0, 80, 24)
  }
}

impl AlacDimensions for TerminalBounds {
  fn total_lines(&self) -> usize {
    self.lines
  }

  fn screen_lines(&self) -> usize {
    self.lines
  }

  fn columns(&self) -> usize {
    self.columns
  }
}

/// Colors explicitly set by the running application via OSC 10/11/12.
///
/// `None` entries fall back to the host theme. Hosts should consult these
/// when rendering default-styled cells so that applications that repaint
/// their background (e.g. `vim` color schemes) look correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DynamicColors {
  pub foreground: Option<CellColor>,
  pub background: Option<CellColor>,
  pub cursor: Option<CellColor>,
}

/// A point-in-time snapshot of the terminal state, taken under a short lock.
#[derive(Debug, Clone)]
pub struct Content {
  /// All visible cells, ordered row-major from the top of the viewport.
  pub cells: Vec<IndexedCell>,
  pub mode: Modes,
  pub cursor: Cursor,
  /// The character under the cursor (used by views to render block cursors).
  pub cursor_char: char,
  pub total_lines: usize,
  pub display_offset: usize,
  pub columns: usize,
  pub screen_lines: usize,
  pub selection: Option<SelectionRange>,
  pub scrolled_to_top: bool,
  pub scrolled_to_bottom: bool,
  /// Colors the application set via OSC 10/11/12, if any.
  pub dynamic_colors: DynamicColors,
  /// The bounds that were active when the snapshot was taken.
  pub terminal_bounds: TerminalBounds,
}

impl Default for Content {
  fn default() -> Self {
    Self::empty()
  }
}

impl Content {
  /// An empty snapshot with default viewport bounds.
  pub fn empty() -> Self {
    Self {
      cells: Vec::new(),
      mode: Modes::empty(),
      cursor: Cursor {
        shape: CursorShape::Block,
        point: Point::new(0, 0),
      },
      cursor_char: ' ',
      total_lines: 0,
      display_offset: 0,
      columns: 0,
      screen_lines: 0,
      selection: None,
      scrolled_to_top: true,
      scrolled_to_bottom: true,
      dynamic_colors: DynamicColors::default(),
      terminal_bounds: TerminalBounds::default(),
    }
  }

  /// The cells of one viewport row (`0` is the topmost visible line).
  ///
  /// Rows are viewport-relative: they move with scrollback, unlike
  /// [`Point`] coordinates carried by the cells themselves, which are also
  /// viewport-relative.
  pub fn row(&self, row: usize) -> Option<&[IndexedCell]> {
    if row >= self.screen_lines {
      return None;
    }
    let start = row * self.columns;
    self.cells.get(start..start + self.columns)
  }

  /// The plain text of one viewport row, excluding wide-character spacers.
  pub fn line_text(&self, row: usize) -> Option<String> {
    use crate::types::CellFlags as F;
    let cells = self.row(row)?;
    let mut text = String::with_capacity(self.columns);
    for indexed in cells {
      if indexed
        .cell
        .flags
        .intersects(F::WIDE_CHAR_SPACER | F::LEADING_WIDE_CHAR_SPACER)
      {
        continue;
      }
      text.push(indexed.cell.c);
      text.extend(indexed.cell.zerowidth.iter().copied());
    }
    Some(text)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn modes_bitwise_ops() {
    let mut modes = Modes::APP_CURSOR | Modes::ALT_SCREEN;
    assert!(modes.contains(Modes::APP_CURSOR));
    assert!(modes.contains(Modes::ALT_SCREEN));
    assert!(!modes.contains(Modes::BRACKETED_PASTE));

    modes.insert(Modes::BRACKETED_PASTE);
    assert!(modes.contains(Modes::BRACKETED_PASTE));

    modes.remove(Modes::ALT_SCREEN);
    assert!(!modes.contains(Modes::ALT_SCREEN));
    assert!(modes.intersects(Modes::BRACKETED_PASTE | Modes::APP_CURSOR));

    assert!(Modes::MOUSE_MODE.contains(Modes::MOUSE_DRAG));
    assert!(Modes::empty().is_empty());
  }

  #[test]
  fn bounds_are_normalized() {
    let bounds = TerminalBounds::new(0.0, 0.0, 0, 0);
    assert_eq!(bounds.num_columns(), 1);
    assert_eq!(bounds.num_lines(), 1);
    assert_eq!(bounds.cell_width(), 1.0);
    assert_eq!(bounds.line_height(), 1.0);

    let bounds = TerminalBounds::new(24.0, 8.0, 80, 24);
    assert_eq!(bounds.num_columns(), 80);
    assert_eq!(bounds.num_lines(), 24);
    assert_eq!(bounds.screen_lines(), 24);
    assert_eq!(bounds.total_lines(), 24);
    assert_eq!(bounds.width(), 640.0);
    assert_eq!(bounds.height(), 576.0);
  }

  #[test]
  fn point_ordering() {
    let a = Point::new(0, 5);
    let b = Point::new(1, 0);
    let c = Point::new(1, 2);
    assert!(a < b);
    assert!(b < c);
    assert_eq!(c, Point::new(1, 2));
  }

  #[test]
  fn range_contains() {
    let range = Range::new(Point::new(0, 2), Point::new(3, 10));
    assert!(range.contains(Point::new(0, 2)));
    assert!(range.contains(Point::new(2, 7)));
    assert!(range.contains(Point::new(3, 10)));
    assert!(!range.contains(Point::new(0, 1)));
    assert!(!range.contains(Point::new(3, 11)));
  }

  #[test]
  fn scroll_kind_conversions() {
    assert!(matches!(
      ScrollKind::Delta(3).to_alacritty(),
      alacritty_terminal::grid::Scroll::Delta(3)
    ));
    assert!(matches!(
      ScrollKind::PageUp.to_alacritty(),
      alacritty_terminal::grid::Scroll::PageUp
    ));
  }

  #[test]
  fn row_and_line_text() {
    let mut content = Content::empty();
    content.columns = 4;
    content.screen_lines = 2;
    content.cells = (0..8)
      .map(|index| IndexedCell {
        point: Point::new((index / 4) as i32, index % 4),
        cell: Cell {
          c: (b'a' + index as u8) as char,
          ..Cell::default()
        },
      })
      .collect();

    assert_eq!(content.line_text(0).as_deref(), Some("abcd"));
    assert_eq!(content.line_text(1).as_deref(), Some("efgh"));
    assert_eq!(content.line_text(2), None);
    assert_eq!(content.row(0).map(<[IndexedCell]>::len), Some(4));
    assert_eq!(content.row(9), None);
  }
}
