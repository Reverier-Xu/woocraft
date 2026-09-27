use std::{
  fmt::Debug,
  time::{Duration, Instant},
};

/// A HistoryItem represents a single change in the history.
/// It must implement Clone and PartialEq to be used in the History.
pub trait HistoryItem: Clone + PartialEq {
  fn version(&self) -> usize;
  fn set_version(&mut self, version: usize);
}

/// The History is used to keep track of changes to a model and to allow undo
/// and redo operations.
///
/// This is now used in Input for undo/redo operations. You can also use this in
/// your own models to keep track of changes, for example to track the tab
/// history for prev/next features.
///
/// ## Use cases
///
/// - Undo/redo operations in Input
/// - Tracking tab history for prev/next features
#[derive(Debug)]
pub struct History<I: HistoryItem> {
  undos: Vec<I>,
  redos: Vec<I>,
  last_changed_at: Instant,
  version: usize,
  pub(crate) ignore: bool,
  max_undos: usize,
  group_interval: Option<Duration>,
  grouping: bool,
  unique: bool,
}

impl<I> Default for History<I>
where
  I: HistoryItem,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<I> History<I>
where
  I: HistoryItem,
{
  pub fn new() -> Self {
    Self {
      undos: Default::default(),
      redos: Default::default(),
      ignore: false,
      last_changed_at: Instant::now(),
      version: 0,
      max_undos: 1000,
      group_interval: None,
      grouping: false,
      unique: false,
    }
  }

  /// Set the maximum number of undo steps to keep, defaults to 1000.
  pub fn max_undos(mut self, max_undos: usize) -> Self {
    self.max_undos = max_undos;
    self
  }

  /// Set the history to be unique, defaults to false.
  /// If set to true, the history will only keep unique changes.
  pub fn unique(mut self) -> Self {
    self.unique = true;
    self
  }

  /// Set the interval in milliseconds to group changes, defaults to None.
  pub fn group_interval(mut self, group_interval: Duration) -> Self {
    self.group_interval = Some(group_interval);
    self
  }

  /// Start grouping changes, this will prevent the version from being
  /// incremented until `end_grouping` is called.
  pub fn start_grouping(&mut self) {
    self.grouping = true;
  }

  /// End grouping changes, this will allow the version to be incremented again.
  pub fn end_grouping(&mut self) {
    self.grouping = false;
  }

  /// Increment the version number if the last change was made more than
  /// `GROUP_INTERVAL` milliseconds ago.
  fn inc_version(&mut self) -> usize {
    let t = Instant::now();
    if !self.grouping && Some(self.last_changed_at.elapsed()) > self.group_interval {
      self.version += 1;
    }

    self.last_changed_at = t;
    self.version
  }

  /// Get the current version number.
  pub fn version(&self) -> usize {
    self.version
  }

  /// Push a new change to the history.
  pub fn push(&mut self, item: I) {
    let version = self.inc_version();

    if self.undos.len() >= self.max_undos {
      self.undos.remove(0);
    }

    if self.unique {
      self.undos.retain(|c| *c != item);
      self.redos.retain(|c| *c != item);
    }

    let mut item = item;
    item.set_version(version);
    self.undos.push(item);
  }

  /// Get the undo stack.
  pub fn undos(&self) -> &Vec<I> {
    &self.undos
  }

  /// Get the redo stack.
  pub fn redos(&self) -> &Vec<I> {
    &self.redos
  }

  /// Clear the undo and redo stacks.
  pub fn clear(&mut self) {
    self.undos.clear();
    self.redos.clear();
  }

  /// Undo the last change and return the changes that were undone.
  pub fn undo(&mut self) -> Option<Vec<I>> {
    let first_change = self.undos.pop()?;
    let version = first_change.version();
    let mut changes = vec![first_change.clone()];
    // Versions are pushed in non-decreasing order (and `retain` in `push`
    // preserves that), so all entries sharing a version are contiguous at the
    // top of the stack: pop while the top entry still matches instead of
    // rescanning the whole stack per popped entry.
    while self.undos.last().is_some_and(|c| c.version() == version) {
      changes.push(self.undos.pop().unwrap());
    }

    self.redos.extend(changes.clone());
    Some(changes)
  }

  /// Redo the last undone change and return the changes that were redone.
  pub fn redo(&mut self) -> Option<Vec<I>> {
    let first_change = self.redos.pop()?;
    let version = first_change.version();
    let mut changes = vec![first_change.clone()];
    // See `undo`: same-version entries are contiguous at the top.
    while self.redos.last().is_some_and(|c| c.version() == version) {
      changes.push(self.redos.pop().unwrap());
    }
    self.undos.extend(changes.clone());
    Some(changes)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Clone)]
  struct TabIndex {
    tab_index: usize,
    version: usize,
  }

  impl PartialEq for TabIndex {
    fn eq(&self, other: &Self) -> bool {
      self.tab_index == other.tab_index
    }
  }

  impl From<usize> for TabIndex {
    fn from(value: usize) -> Self {
      TabIndex {
        tab_index: value,
        version: 0,
      }
    }
  }

  impl HistoryItem for TabIndex {
    fn version(&self) -> usize {
      self.version
    }
    fn set_version(&mut self, version: usize) {
      self.version = version;
    }
  }

  #[test]
  fn test_history() {
    let mut history: History<TabIndex> = History::new().max_undos(100);
    history.push(0.into());
    history.push(3.into());
    history.push(2.into());
    history.push(1.into());

    assert_eq!(history.version(), 4);
    let changes = history.undo().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].tab_index, 1);

    let changes = history.undo().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].tab_index, 2);

    history.push(5.into());

    let changes = history.redo().unwrap();
    assert_eq!(changes[0].tab_index, 2);

    let changes = history.redo().unwrap();
    assert_eq!(changes[0].tab_index, 1);

    let changes = history.undo().unwrap();
    assert_eq!(changes[0].tab_index, 1);

    let changes = history.undo().unwrap();
    assert_eq!(changes[0].tab_index, 2);

    let changes = history.undo().unwrap();
    assert_eq!(changes[0].tab_index, 5);

    let changes = history.undo().unwrap();
    assert_eq!(changes[0].tab_index, 3);

    let changes = history.undo().unwrap();
    assert_eq!(changes[0].tab_index, 0);

    assert!(history.undo().is_none());
  }

  #[test]
  fn test_unique_history() {
    let mut history: History<TabIndex> = History::new().max_undos(100).unique();

    // Push some items
    history.push(0.into());
    history.push(1.into());
    history.push(1.into()); // Duplicate, should be ignored
    history.push(2.into());
    history.push(1.into()); // Duplicate, should be remove old, and add new

    // Check the version and undo stack
    assert_eq!(history.version(), 5);
    assert_eq!(history.undos().len(), 3);
    assert_eq!(history.undos().last().unwrap().tab_index, 1);

    // Undo the last change
    let changes = history.undo().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].tab_index, 1);

    assert_eq!(history.redos().len(), 1);
    // Push duplicate, should be ignored
    history.push(2.into());

    assert_eq!(history.undos().len(), 2);
    assert_eq!(history.redos().len(), 1);

    // Redo the last undone change
    let changes = history.redo().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].tab_index, 1);

    // Push another item
    history.push(3.into());

    // Check the version and undo stack
    assert_eq!(history.version(), 7);
    assert_eq!(history.undos().len(), 4);

    // Undo all changes
    for _ in 0..4 {
      history.undo();
    }

    // Check the undo stack is empty and redo stack has all changes
    assert_eq!(history.undos().len(), 0);
    assert_eq!(history.redos().len(), 4);
  }

  #[test]
  fn test_grouped_history() {
    let mut history: History<TabIndex> = History::new();
    history.start_grouping();
    history.push(1.into());
    history.push(2.into());
    history.push(3.into());
    history.end_grouping();
    history.push(4.into());

    // Grouped pushes share one version (0); the ungrouped push gets its own
    // (1).
    assert_eq!(history.version(), 1);
    assert_eq!(history.undos().len(), 4);
    assert_eq!(history.undos().first().unwrap().version(), 0);
    assert_eq!(history.undos().last().unwrap().version(), 1);

    // The ungrouped entry is undone on its own.
    let changes = history.undo().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].tab_index, 4);
    assert_eq!(history.undos().len(), 3);

    // The three grouped entries are undone together as one version.
    let changes = history.undo().unwrap();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].tab_index, 3);
    assert_eq!(changes[2].tab_index, 1);
    assert!(history.undo().is_none());

    // Redo replays whole groups back in push order.
    let changes = history.redo().unwrap();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].tab_index, 1);
    assert_eq!(changes[2].tab_index, 3);

    let changes = history.redo().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].tab_index, 4);
    assert!(history.redo().is_none());
    assert_eq!(history.undos().len(), 4);
  }
}
