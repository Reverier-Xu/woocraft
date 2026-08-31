//! Pins how the emulator parses underline-heavy output like `eza`'s file
//! listings (broken-symlink targets are printed with SGR 4).
//!
//! The renderer paints decorations cell-aligned by hand (see
//! `BatchedTextRun::paint` in the view layer); these assertions ensure the
//! underline *data* it relies on never includes trailing padding cells, so a
//! regression here — not in painting — can be ruled out first when underline
//! placement looks wrong.

use woocraft_terminal::{CellFlags, SpawnOptions, TerminalBounds, TerminalSession};

#[test]
fn eza_style_underline_spans_exactly_the_filename() {
  let session = TerminalSession::spawn_display_only(
    SpawnOptions::default(),
    TerminalBounds::new(20.0, 8.0, 100, 4),
  );
  // Mimic `eza --long`: an underlined broken-symlink target at end of line,
  // and a grid-mode line with unstyled padding between columns.
  session.feed_display(
    b"\x1b[4;31mnonexistent\x1b[0m\r\n\
      \x1b[4;31mname.txt\x1b[0m  \x1b[36mother.txt\x1b[0m  \x1b[4;31mthird.txt\x1b[0m\r\n",
  );
  let content = session.snapshot();

  let underline_mask = |row: usize| -> String {
    content
      .row(row)
      .unwrap()
      .iter()
      .map(|indexed| {
        if indexed.cell.flags.contains(CellFlags::UNDERLINE) {
          'u'
        } else {
          '.'
        }
      })
      .collect()
  };

  // Underline covers exactly the 11 filename cells; padding is clean.
  assert_eq!(
    underline_mask(0).trim_end_matches('.'),
    "u".repeat(11),
    "row 0: {:?}",
    underline_mask(0)
  );
  let row1 = underline_mask(1);
  assert!(row1.starts_with("uuuuuuuu."));
  assert!(row1.contains("uuuuuuuuu.."));
  assert_eq!(row1.matches('u').count(), 8 + 9);
}
