use std::rc::Rc;

use gpui::{Pixels, Size, size};

use crate::IndexPath;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowEntry {
  Entry(IndexPath),
  SectionHeader(usize),
  SectionFooter(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct MeasuredEntrySize {
  pub(crate) item_size: Size<Pixels>,
  pub(crate) section_header_size: Size<Pixels>,
  pub(crate) section_footer_size: Size<Pixels>,
}

impl RowEntry {
  pub(crate) fn index(&self) -> IndexPath {
    match self {
      RowEntry::Entry(index_path) => *index_path,
      RowEntry::SectionHeader(ix) => IndexPath::default().section(*ix),
      RowEntry::SectionFooter(ix) => IndexPath::default().section(*ix),
    }
  }

  #[inline]
  pub(crate) fn is_entry(&self) -> bool {
    matches!(self, RowEntry::Entry(_))
  }
}

#[derive(Default, Clone)]
pub(crate) struct RowsCache {
  /// Flattened rows. Only contains sections with items.
  pub(crate) entities: Rc<Vec<RowEntry>>,
  pub(crate) items_count: usize,
  /// Item count for each section.
  pub(crate) sections: Rc<Vec<usize>>,
  pub(crate) entries_sizes: Rc<Vec<Size<Pixels>>>,
  /// Flat index of every entry row in `entities`, ordered by (section, row).
  entry_positions: Rc<Vec<usize>>,
  /// Cached `entries_sizes` with a trailing gap row appended, paired with
  /// the gap it was built for. Reset whenever the base sizes are rebuilt.
  gap_sizes: Option<(Pixels, Rc<Vec<Size<Pixels>>>)>,
}

impl RowsCache {
  /// Force the cache to be rebuilt on the next `prepare` call.
  ///
  /// Call this after the underlying delegate data has changed structurally
  /// (items added/removed/reordered) to ensure the list picks up the changes.
  pub(crate) fn invalidate(&mut self) {
    self.sections = Rc::new(vec![]);
  }

  pub(crate) fn get(&self, flatten_ix: usize) -> Option<RowEntry> {
    self.entities.get(flatten_ix).cloned()
  }

  /// Number of flattened rows (includes header/item/footer).
  pub(crate) fn len(&self) -> usize {
    self.entities.len()
  }

  /// Number of item rows (excludes header/footer).
  pub(crate) fn items_count(&self) -> usize {
    self.items_count
  }

  /// Whether the cache already holds rows for the given per-section item
  /// counts. When this returns `true`, callers can skip re-measuring and
  /// rebuilding entirely.
  pub(crate) fn matches_sections(&self, new_sections: &[usize]) -> bool {
    self.sections.as_slice() == new_sections
  }

  /// Index of the item with `path` in flattened rows.
  ///
  /// Binary searches `entry_positions` instead of scanning all rows, keeping
  /// keyboard navigation O(log n) on large lists.
  pub(crate) fn position_of(&self, path: &IndexPath) -> Option<usize> {
    // Cache entries always live at column 0, so paths with a non-zero
    // column can never match.
    if path.column != 0 {
      return None;
    }

    let ordinal = self
      .entry_positions
      .binary_search_by(|&flat_ix| {
        let entry = self.entities[flat_ix].index();
        (entry.section, entry.row).cmp(&(path.section, path.row))
      })
      .ok()?;
    Some(self.entry_positions[ordinal])
  }

  /// Entry row sizes with a trailing zero-width gap row appended.
  ///
  /// The augmented vector is cached, so steady-state frames share one `Rc`
  /// instead of cloning the whole sizes table on every render.
  pub(crate) fn entries_sizes_with_gap(&mut self, gap: Pixels) -> Rc<Vec<Size<Pixels>>> {
    if let Some((cached_gap, sizes)) = &self.gap_sizes
      && *cached_gap == gap
    {
      return sizes.clone();
    }

    let mut sizes = (*self.entries_sizes).clone();
    sizes.push(size(Pixels::ZERO, gap));
    let sizes = Rc::new(sizes);
    self.gap_sizes = Some((gap, sizes.clone()));
    sizes
  }

  /// Previous item row, wrapping around and skipping empty sections.
  pub(crate) fn prev(&self, path: Option<IndexPath>) -> IndexPath {
    let path = path.unwrap_or_default();
    let Some(pos) = self.position_of(&path) else {
      return self
        .entities
        .iter()
        .rfind(|entry| entry.is_entry())
        .map(|entry| entry.index())
        .unwrap_or_default();
    };

    if let Some(path) = self
      .entities
      .iter()
      .take(pos)
      .rev()
      .find(|entry| entry.is_entry())
      .map(|entry| entry.index())
    {
      path
    } else {
      self
        .entities
        .iter()
        .rfind(|entry| entry.is_entry())
        .map(|entry| entry.index())
        .unwrap_or_default()
    }
  }

  /// Next item row, wrapping around and skipping empty sections.
  pub(crate) fn next(&self, path: Option<IndexPath>) -> IndexPath {
    let Some(mut path) = path else {
      return IndexPath::default();
    };

    let Some(pos) = self.position_of(&path) else {
      return self
        .entities
        .iter()
        .find(|entry| entry.is_entry())
        .map(|entry| entry.index())
        .unwrap_or_default();
    };

    if let Some(next_path) = self
      .entities
      .iter()
      .skip(pos + 1)
      .find(|entry| entry.is_entry())
      .map(|entry| entry.index())
    {
      path = next_path;
    } else {
      path = self
        .entities
        .iter()
        .find(|entry| entry.is_entry())
        .map(|entry| entry.index())
        .unwrap_or_default();
    }

    path
  }

  /// Rebuild the flattened rows and sizes for the given per-section item
  /// counts.
  ///
  /// Callers must check `matches_sections` first and only call this when the
  /// counts changed: this unconditionally rebuilds and resets the derived
  /// measurement and gap caches.
  pub(crate) fn prepare(&mut self, measured_size: MeasuredEntrySize, new_sections: Vec<usize>) {
    let mut entries_sizes = vec![];
    let mut entry_positions = vec![];
    let mut total_items_count = 0;
    let mut flat_row_count = 0;
    self.sections = Rc::new(new_sections);
    self.entities = Rc::new(
      self
        .sections
        .iter()
        .enumerate()
        .flat_map(|(section, items_count)| {
          total_items_count += items_count;
          let mut children = vec![];
          if *items_count == 0 {
            return children;
          }

          children.push(RowEntry::SectionHeader(section));
          entries_sizes.push(measured_size.section_header_size);
          for row in 0..*items_count {
            entry_positions.push(flat_row_count + children.len());
            children.push(RowEntry::Entry(IndexPath {
              section,
              row,
              ..Default::default()
            }));
            entries_sizes.push(measured_size.item_size);
          }
          children.push(RowEntry::SectionFooter(section));
          entries_sizes.push(measured_size.section_footer_size);
          flat_row_count += children.len();
          children
        })
        .collect(),
    );
    self.entry_positions = Rc::new(entry_positions);
    self.entries_sizes = Rc::new(entries_sizes);
    self.items_count = total_items_count;
    self.gap_sizes = None;
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::super::cache::{RowEntry, RowsCache};
  use crate::IndexPath;

  fn build_cache(sections: &[usize]) -> RowsCache {
    let mut entities: Vec<RowEntry> = vec![];
    let mut entry_positions: Vec<usize> = vec![];
    for (section, items_count) in sections.iter().enumerate() {
      if *items_count == 0 {
        continue;
      }

      entities.push(RowEntry::SectionHeader(section));
      for row in 0..*items_count {
        entry_positions.push(entities.len());
        entities.push(RowEntry::Entry(IndexPath {
          section,
          row,
          ..Default::default()
        }));
      }
      entities.push(RowEntry::SectionFooter(section));
    }

    RowsCache {
      sections: Rc::new(sections.to_vec()),
      entities: Rc::new(entities),
      entry_positions: Rc::new(entry_positions),
      ..Default::default()
    }
  }

  #[test]
  fn test_position_of() {
    let row_cache = build_cache(&[2, 4, 3]);

    // Flat positions account for section headers and footers.
    assert_eq!(row_cache.position_of(&IndexPath::new(0)), Some(1));
    assert_eq!(row_cache.position_of(&IndexPath::new(1)), Some(2));
    assert_eq!(row_cache.position_of(&IndexPath::new(0).section(1)), Some(5));
    assert_eq!(row_cache.position_of(&IndexPath::new(3).section(1)), Some(8));
    assert_eq!(row_cache.position_of(&IndexPath::new(2).section(2)), Some(13));

    // Out-of-range rows and sections are not found.
    assert_eq!(row_cache.position_of(&IndexPath::new(2).section(0)), None);
    assert_eq!(row_cache.position_of(&IndexPath::new(4).section(1)), None);
    assert_eq!(row_cache.position_of(&IndexPath::new(0).section(3)), None);

    // Entries always live at column 0.
    assert_eq!(
      row_cache
        .position_of(&IndexPath::new(0).section(0).column(1)),
      None
    );
  }

  #[test]
  fn test_prev_next() {
    let row_cache = build_cache(&[2, 4, 3]);

    assert_eq!(
      row_cache.next(Some(IndexPath::new(0).section(0))),
      IndexPath::new(1).section(0)
    );
    assert_eq!(
      row_cache.next(Some(IndexPath::new(1).section(0))),
      IndexPath::new(0).section(1)
    );
    assert_eq!(
      row_cache.next(Some(IndexPath::new(0).section(1))),
      IndexPath::new(1).section(1)
    );
    assert_eq!(
      row_cache.next(Some(IndexPath::new(3).section(1))),
      IndexPath::new(0).section(2)
    );
    assert_eq!(
      row_cache.next(Some(IndexPath::new(0).section(2))),
      IndexPath::new(1).section(2)
    );
    assert_eq!(
      row_cache.next(Some(IndexPath::new(1).section(2))),
      IndexPath::new(2).section(2)
    );
    assert_eq!(
      row_cache.next(Some(IndexPath::new(2).section(2))),
      IndexPath::new(0).section(0)
    );

    assert_eq!(
      row_cache.prev(Some(IndexPath::new(0).section(0))),
      IndexPath::new(2).section(2)
    );
    assert_eq!(
      row_cache.prev(Some(IndexPath::new(1).section(0))),
      IndexPath::new(0).section(0)
    );
    assert_eq!(
      row_cache.prev(Some(IndexPath::new(0).section(1))),
      IndexPath::new(1).section(0)
    );
    assert_eq!(
      row_cache.prev(Some(IndexPath::new(1).section(1))),
      IndexPath::new(0).section(1)
    );
    assert_eq!(
      row_cache.prev(Some(IndexPath::new(3).section(1))),
      IndexPath::new(2).section(1)
    );
    assert_eq!(
      row_cache.prev(Some(IndexPath::new(0).section(2))),
      IndexPath::new(3).section(1)
    );
  }
}
