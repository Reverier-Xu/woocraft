use std::rc::Rc;

use gpui::{App, Pixels, Size};

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
  #[inline]
  pub(crate) fn eq_index_path(&self, path: &IndexPath) -> bool {
    match self {
      RowEntry::Entry(index_path) => index_path == path,
      RowEntry::SectionHeader(_) | RowEntry::SectionFooter(_) => false,
    }
  }

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
  measured_size: MeasuredEntrySize,
}

impl RowsCache {
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

  /// Index of the item with `path` in flattened rows.
  pub(crate) fn position_of(&self, path: &IndexPath) -> Option<usize> {
    self
      .entities
      .iter()
      .position(|p| p.is_entry() && p.eq_index_path(path))
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

  pub(crate) fn prepare_if_needed<F>(
    &mut self, sections_count: usize, measured_size: MeasuredEntrySize, cx: &App, rows_count_f: F,
  ) where
    F: Fn(usize, &App) -> usize, {
    let mut new_sections = vec![];
    for section_ix in 0..sections_count {
      new_sections.push(rows_count_f(section_ix, cx));
    }

    let need_update = new_sections != *self.sections || self.measured_size != measured_size;
    if !need_update {
      return;
    }

    let mut entries_sizes = vec![];
    let mut total_items_count = 0;
    self.measured_size = measured_size;
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
            children.push(RowEntry::Entry(IndexPath {
              section,
              row,
              ..Default::default()
            }));
            entries_sizes.push(measured_size.item_size);
          }
          children.push(RowEntry::SectionFooter(section));
          entries_sizes.push(measured_size.section_footer_size);
          children
        })
        .collect(),
    );
    self.entries_sizes = Rc::new(entries_sizes);
    self.items_count = total_items_count;
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::super::cache::{RowEntry, RowsCache};
  use crate::IndexPath;

  fn build_entities(sections: &[usize]) -> Vec<RowEntry> {
    sections
      .iter()
      .enumerate()
      .flat_map(|(section, items_count)| {
        let mut children = vec![];
        if *items_count == 0 {
          return children;
        }

        children.push(RowEntry::SectionHeader(section));
        for row in 0..*items_count {
          children.push(RowEntry::Entry(IndexPath {
            section,
            row,
            ..Default::default()
          }));
        }
        children.push(RowEntry::SectionFooter(section));
        children
      })
      .collect()
  }

  #[test]
  fn test_prev_next() {
    let mut row_cache = RowsCache::default();
    row_cache.sections = Rc::new(vec![2, 4, 3]);
    row_cache.entities = Rc::new(build_entities(&[2, 4, 3]));

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
