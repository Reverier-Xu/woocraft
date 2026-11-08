//! Mouse event mapping: grid coordinates, mouse-report encoding, and
//! alternate-screen scrolling.
//!
//! The report encodings follow the xterm conventions implemented by
//! Alacritty: SGR (CSI < b ; x ; y M/m), normal X10-style (CSI M CbCxCy), and
//! the UTF-8-extended variant for coordinates above 95.

use std::cmp;

use gpui::{Modifiers, MouseButton, Pixels, Point, ScrollDelta, px};
use woocraft_terminal::{Modes, Point as GridPoint, TerminalBounds};

/// The wire format selected by the terminal's mouse modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MouseFormat {
  Sgr,
  Normal { utf8: bool },
}

impl MouseFormat {
  fn from_mode(mode: Modes) -> Self {
    if mode.contains(Modes::SGR_MOUSE) {
      Self::Sgr
    } else if mode.contains(Modes::UTF8_MOUSE) {
      Self::Normal { utf8: true }
    } else {
      Self::Normal { utf8: false }
    }
  }
}

/// The xterm button codes for mouse reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MouseButtonCode {
  Left       = 0,
  Middle     = 1,
  Right      = 2,
  LeftMove   = 32,
  MiddleMove = 33,
  RightMove  = 34,
  NoneMove   = 35,
  ScrollUp   = 64,
  ScrollDown = 65,
  Other      = 99,
}

impl MouseButtonCode {
  fn from_button(button: MouseButton) -> Self {
    match button {
      MouseButton::Left => Self::Left,
      MouseButton::Middle => Self::Middle,
      MouseButton::Right => Self::Right,
      MouseButton::Navigate(_) => Self::Other,
    }
  }

  fn from_move_button(button: Option<MouseButton>) -> Self {
    match button {
      Some(MouseButton::Left) => Self::LeftMove,
      Some(MouseButton::Middle) => Self::MiddleMove,
      Some(MouseButton::Right) => Self::RightMove,
      Some(MouseButton::Navigate(_)) => Self::Other,
      None => Self::NoneMove,
    }
  }

  fn from_scroll_delta(delta: ScrollDelta) -> Self {
    let upward = match delta {
      ScrollDelta::Pixels(pixels) => pixels.y > px(0.),
      ScrollDelta::Lines(lines) => lines.y > 0.,
    };
    if upward {
      Self::ScrollUp
    } else {
      Self::ScrollDown
    }
  }

  fn is_other(self) -> bool {
    self == Self::Other
  }
}

/// Converts a pixel position within the terminal to a grid point, ignoring
/// which half of the cell was hit.
pub fn grid_point(
  position: Point<Pixels>, bounds: TerminalBounds, display_offset: usize,
) -> GridPoint {
  grid_point_and_side(position, bounds, display_offset).0
}

/// Converts a pixel position within the terminal to a grid point, along with
/// which half of the cell the position falls in (used to anchor selections).
pub fn grid_point_and_side(
  position: Point<Pixels>, bounds: TerminalBounds, display_offset: usize,
) -> (GridPoint, bool) {
  let column = (position.x / px(bounds.cell_width())) as usize;
  let cell_x = cmp::max(px(0.), position.x) % px(bounds.cell_width());
  let mut after_midpoint = cell_x > px(bounds.cell_width()) / 2.0;

  let last_column = bounds.num_columns().saturating_sub(1);
  let column = if column > last_column {
    after_midpoint = true;
    last_column
  } else {
    column
  };

  let line = (position.y / px(bounds.line_height())) as i32;
  let bottommost_line = i32::try_from(bounds.num_lines().saturating_sub(1)).unwrap_or(i32::MAX);
  let line = if line > bottommost_line {
    after_midpoint = true;
    bottommost_line
  } else {
    line.max(0)
  };

  let display_offset = i32::try_from(display_offset).unwrap_or(i32::MAX);
  (
    GridPoint::new(line.saturating_sub(display_offset), column),
    after_midpoint,
  )
}

/// Encodes the scroll-wheel reports for a scroll of `scroll_lines` rows while
/// a mouse-report mode is active. Each scrolled line produces one report.
pub fn scroll_reports(
  point: GridPoint, scroll_lines: i32, delta: ScrollDelta, mode: Modes,
) -> Option<impl Iterator<Item = Vec<u8>>> {
  if !mode.intersects(Modes::MOUSE_MODE) {
    return None;
  }
  let code = MouseButtonCode::from_scroll_delta(delta);
  mouse_report(
    point,
    code,
    true,
    Modifiers::default(),
    MouseFormat::from_mode(mode),
  )
  .map(|report| std::iter::repeat_n(report, scroll_lines.unsigned_abs() as usize))
}

/// Encodes the arrow-key sequences used to scroll in the alternate screen
/// when `ALTERNATE_SCROLL` mode is active.
pub fn alt_scroll(scroll_lines: i32) -> Vec<u8> {
  let command = if scroll_lines > 0 { b'A' } else { b'B' };
  let mut content = Vec::with_capacity(scroll_lines.unsigned_abs() as usize * 3);
  for _ in 0..scroll_lines.abs() {
    content.extend_from_slice(b"\x1bO");
    content.push(command);
  }
  content
}

/// Encodes a button press/release report, if mouse reporting applies.
pub fn mouse_button_report(
  point: GridPoint, button: MouseButton, modifiers: Modifiers, pressed: bool, mode: Modes,
) -> Option<Vec<u8>> {
  let code = MouseButtonCode::from_button(button);
  if code.is_other() || !mode.intersects(Modes::MOUSE_MODE) {
    return None;
  }
  mouse_report(
    point,
    code,
    pressed,
    modifiers,
    MouseFormat::from_mode(mode),
  )
}

/// Encodes a mouse-motion report, if mouse reporting applies.
///
/// Motion is only reported in motion mode, or drag mode with a pressed button.
pub fn mouse_moved_report(
  point: GridPoint, button: Option<MouseButton>, modifiers: Modifiers, mode: Modes,
) -> Option<Vec<u8>> {
  let code = MouseButtonCode::from_move_button(button);
  if code.is_other() || !mode.intersects(Modes::MOUSE_MOTION | Modes::MOUSE_DRAG) {
    return None;
  }
  // Drag mode only reports while a button is held.
  if mode.contains(Modes::MOUSE_DRAG) && code == MouseButtonCode::NoneMove {
    return None;
  }
  mouse_report(point, code, true, modifiers, MouseFormat::from_mode(mode))
}

fn mouse_report(
  point: GridPoint, button: MouseButtonCode, pressed: bool, modifiers: Modifiers,
  format: MouseFormat,
) -> Option<Vec<u8>> {
  // Reports outside the grid are dropped.
  if point.line < 0 {
    return None;
  }

  let mut mods = 0;
  if modifiers.shift {
    mods += 4;
  }
  if modifiers.alt {
    mods += 8;
  }
  if modifiers.control {
    mods += 16;
  }

  match format {
    MouseFormat::Sgr => Some(sgr_mouse_report(point, button as u8 + mods, pressed)),
    MouseFormat::Normal { utf8 } => {
      if pressed {
        normal_mouse_report(point, button as u8 + mods, utf8)
      } else {
        normal_mouse_report(point, 3 + mods, utf8)
      }
    }
  }
}

fn normal_mouse_report(point: GridPoint, button: u8, utf8: bool) -> Option<Vec<u8>> {
  let max = if utf8 { 2015 } else { 223 };
  if point.line >= max || point.column >= max as usize {
    return None;
  }

  let mut message = vec![b'\x1b', b'[', b'M', 32 + button];
  if utf8 && point.column >= 95 {
    message.extend_from_slice(&utf8_position(point.column));
  } else {
    message.push(32 + 1 + point.column as u8);
  }
  if utf8 && point.line >= 95 {
    message.extend_from_slice(&utf8_position(point.line as usize));
  } else {
    message.push(32 + 1 + point.line as u8);
  }
  Some(message)
}

fn utf8_position(position: usize) -> [u8; 2] {
  let value = 32 + 1 + position;
  [(0xC0 + value / 64) as u8, (0x80 + (value % 64)) as u8]
}

fn sgr_mouse_report(point: GridPoint, button: u8, pressed: bool) -> Vec<u8> {
  let suffix = if pressed { 'M' } else { 'm' };
  format!(
    "\x1b[<{button};{};{}{suffix}",
    point.column + 1,
    point.line + 1
  )
  .into_bytes()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn grid_point_clamps_to_bounds() {
    let bounds = TerminalBounds::new(20.0, 8.0, 80, 24);
    let point = grid_point(gpui::Point::new(px(4.0), px(2.0)), bounds, 0);
    assert_eq!(point, GridPoint::new(0, 0));

    let point = grid_point(
      gpui::Point::new(px(8.0 * 100.0), px(20.0 * 40.0)),
      bounds,
      0,
    );
    assert_eq!(point, GridPoint::new(23, 79));
  }

  #[test]
  fn grid_point_applies_display_offset() {
    let bounds = TerminalBounds::new(20.0, 8.0, 80, 24);
    let point = grid_point(gpui::Point::new(px(0.0), px(20.0 * 2.0)), bounds, 5);
    // Two rows down minus five lines of scrollback.
    assert_eq!(point, GridPoint::new(-3, 0));
  }

  #[test]
  fn scroll_reports_repeat_per_line() {
    let mode = Modes::MOUSE_MODE;
    let reports: Vec<Vec<u8>> = scroll_reports(
      GridPoint::new(0, 0),
      3,
      ScrollDelta::Lines(Point::new(0., 1.)),
      mode,
    )
    .unwrap()
    .collect();
    assert_eq!(reports.len(), 3);
    assert!(reports.iter().all(|report| report == reports[0].as_slice()));

    let reports: Vec<Vec<u8>> = scroll_reports(
      GridPoint::new(0, 0),
      -2,
      ScrollDelta::Lines(Point::new(0., -1.)),
      mode,
    )
    .unwrap()
    .collect();
    assert_eq!(reports.len(), 2);
  }

  #[test]
  fn scroll_reports_absent_outside_mouse_mode() {
    assert!(
      scroll_reports(
        GridPoint::new(0, 0),
        1,
        ScrollDelta::Lines(Point::new(0., 1.)),
        Modes::NONE
      )
      .is_none()
    );
  }

  #[test]
  fn sgr_button_reports() {
    let mode = Modes::SGR_MOUSE | Modes::MOUSE_REPORT_CLICK;
    let press = mouse_button_report(
      GridPoint::new(2, 10),
      MouseButton::Left,
      Modifiers::default(),
      true,
      mode,
    )
    .unwrap();
    assert_eq!(press, b"\x1b[<0;11;3M".to_vec());

    let release = mouse_button_report(
      GridPoint::new(2, 10),
      MouseButton::Left,
      Modifiers::default(),
      false,
      mode,
    )
    .unwrap();
    assert_eq!(release, b"\x1b[<0;11;3m".to_vec());

    let shift_press = mouse_button_report(
      GridPoint::new(2, 10),
      MouseButton::Left,
      Modifiers {
        shift: true,
        ..Default::default()
      },
      true,
      mode,
    )
    .unwrap();
    assert_eq!(shift_press, b"\x1b[<4;11;3M".to_vec());
  }

  #[test]
  fn normal_button_reports() {
    let mode = Modes::MOUSE_REPORT_CLICK;
    let press = mouse_button_report(
      GridPoint::new(0, 0),
      MouseButton::Left,
      Modifiers::default(),
      true,
      mode,
    )
    .unwrap();
    assert_eq!(press, vec![0x1B, b'[', b'M', 32, 33, 33]);
  }

  #[test]
  fn motion_reports_only_in_motion_modes() {
    let point = GridPoint::new(0, 0);
    assert!(mouse_moved_report(point, None, Modifiers::default(), Modes::NONE).is_none());
    // Drag mode without a pressed button reports nothing.
    assert!(mouse_moved_report(point, None, Modifiers::default(), Modes::MOUSE_DRAG).is_none());
    // Pure motion mode reports button 35 (no button).
    let report =
      mouse_moved_report(point, None, Modifiers::default(), Modes::MOUSE_MOTION).unwrap();
    assert_eq!(report, vec![0x1B, b'[', b'M', 32 + 35, 33, 33]);
    // Drag mode with the left button held reports button 32.
    let report = mouse_moved_report(
      point,
      Some(MouseButton::Left),
      Modifiers::default(),
      Modes::MOUSE_DRAG,
    )
    .unwrap();
    assert_eq!(report, vec![0x1B, b'[', b'M', 32 + 32, 33, 33]);
  }

  #[test]
  fn alt_scroll_sequences() {
    assert_eq!(alt_scroll(2), b"\x1bOA\x1bOA".to_vec());
    assert_eq!(alt_scroll(-1), b"\x1bOB".to_vec());
    assert_eq!(alt_scroll(0), Vec::<u8>::new());
  }

  #[test]
  fn utf8_extended_coordinates() {
    let mode = Modes::UTF8_MOUSE | Modes::MOUSE_REPORT_CLICK;
    let press = mouse_button_report(
      GridPoint::new(100, 100),
      MouseButton::Left,
      Modifiers::default(),
      true,
      mode,
    )
    .unwrap();
    // Positions above 95 are encoded as two UTF-8-like bytes.
    assert_eq!(press.len(), 4 + 2 + 2);
  }
}
