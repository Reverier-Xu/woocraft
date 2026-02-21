use std::ops::{Range, RangeBounds};

/// A selection in text represented by UTF-8 byte offsets.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Selection {
  pub start: usize,
  pub end: usize,
}

impl Selection {
  pub fn new(start: usize, end: usize) -> Self {
    Self { start, end }
  }

  pub fn len(&self) -> usize {
    self.end.saturating_sub(self.start)
  }

  pub fn is_empty(&self) -> bool {
    self.start == self.end
  }

  pub fn clear(&mut self) {
    self.start = 0;
    self.end = 0;
  }

  pub fn contains(&self, offset: usize) -> bool {
    offset >= self.start && offset < self.end
  }

  pub fn normalized(&self) -> Range<usize> {
    self.start.min(self.end)..self.start.max(self.end)
  }
}

impl From<Range<usize>> for Selection {
  fn from(value: Range<usize>) -> Self {
    Self::new(value.start, value.end)
  }
}

impl From<Selection> for Range<usize> {
  fn from(value: Selection) -> Self {
    value.start..value.end
  }
}

impl RangeBounds<usize> for Selection {
  fn start_bound(&self) -> std::ops::Bound<&usize> {
    std::ops::Bound::Included(&self.start)
  }

  fn end_bound(&self) -> std::ops::Bound<&usize> {
    std::ops::Bound::Excluded(&self.end)
  }
}
