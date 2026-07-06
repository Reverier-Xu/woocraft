use std::ops::Range;

use gpui::{Along, Axis, Bounds, Context, ElementId, EventEmitter, IsZero, Pixels, Window, px};

use crate::PixelsExt;

mod panel;
mod resize_handle;

pub use panel::*;
pub(crate) use resize_handle::*;

pub(crate) const PANEL_MIN_SIZE: Pixels = px(100.);

pub fn h_resizable(id: impl Into<ElementId>) -> ResizablePanelGroup {
  ResizablePanelGroup::new(id).axis(Axis::Horizontal)
}

pub fn v_resizable(id: impl Into<ElementId>) -> ResizablePanelGroup {
  ResizablePanelGroup::new(id).axis(Axis::Vertical)
}

pub fn resizable_panel() -> ResizablePanel {
  ResizablePanel::new()
}

#[derive(Debug, Clone)]
pub struct ResizableState {
  axis: Axis,
  panels: Vec<ResizablePanelState>,
  sizes: Vec<Pixels>,
  pub(crate) resizing_panel_ix: Option<usize>,
  bounds: Bounds<Pixels>,
  /// Resize target captured from the latest mouse-move event. The actual
  /// size redistribution is deferred to the next frame render so that many
  /// mouse events within one display interval are coalesced into a single
  /// layout pass.
  pending_resize: Option<(usize, Pixels)>,
}

impl Default for ResizableState {
  fn default() -> Self {
    Self {
      axis: Axis::Horizontal,
      panels: vec![],
      sizes: vec![],
      resizing_panel_ix: None,
      bounds: Bounds::default(),
      pending_resize: None,
    }
  }
}

impl ResizableState {
  pub fn sizes(&self) -> &Vec<Pixels> {
    &self.sizes
  }

  /// Returns the live size for a panel, preferring any in-progress drag
  /// preview size stored on the panel state.
  pub(crate) fn display_size(&self, ix: usize) -> Option<Pixels> {
    self
      .panels
      .get(ix)
      .and_then(|panel| panel.size)
      .or_else(|| self.sizes.get(ix).copied())
  }

  #[allow(dead_code)]
  pub(crate) fn insert_panel(
    &mut self, size: Option<Pixels>, ix: Option<usize>, cx: &mut Context<Self>,
  ) {
    let panel_state = ResizablePanelState {
      size,
      ..Default::default()
    };

    let size = size.unwrap_or(PANEL_MIN_SIZE);
    let container_size = self.container_size().max(px(1.));
    let total_leftover_size = (container_size - size).max(px(1.));

    for (i, panel) in self.panels.iter_mut().enumerate() {
      let ratio = self.sizes[i] / container_size;
      self.sizes[i] = total_leftover_size * ratio;
      panel.size = Some(self.sizes[i]);
    }

    if let Some(ix) = ix {
      self.panels.insert(ix, panel_state);
      self.sizes.insert(ix, size);
    } else {
      self.panels.push(panel_state);
      self.sizes.push(size);
    }

    cx.notify();
  }

  pub(crate) fn sync_panels_count(
    &mut self, axis: Axis, panels_count: usize, cx: &mut Context<Self>,
  ) {
    let mut changed = self.axis != axis;
    self.axis = axis;

    if panels_count > self.panels.len() {
      let diff = panels_count - self.panels.len();
      self
        .panels
        .extend(vec![ResizablePanelState::default(); diff]);
      self.sizes.extend(vec![PANEL_MIN_SIZE; diff]);
      changed = true;
    }

    if panels_count < self.panels.len() {
      self.panels.truncate(panels_count);
      self.sizes.truncate(panels_count);
      changed = true;
    }

    if changed {
      self.adjust_to_container_size(cx);
    }
  }

  pub(crate) fn update_panel_size(
    &mut self, panel_ix: usize, bounds: Bounds<Pixels>, size_range: Range<Pixels>,
    cx: &mut Context<Self>,
  ) {
    let size = bounds.size.along(self.axis);
    if self.sizes[panel_ix].as_f32() == PANEL_MIN_SIZE.as_f32() {
      self.sizes[panel_ix] = size;
      self.panels[panel_ix].size = Some(size);
    }

    let panel = &mut self.panels[panel_ix];
    if panel.bounds == bounds && panel.size_range == size_range {
      return;
    }

    panel.bounds = bounds;
    panel.size_range = size_range;
    cx.notify();
  }

  #[allow(dead_code)]
  pub(crate) fn remove_panel(&mut self, panel_ix: usize, cx: &mut Context<Self>) {
    self.panels.remove(panel_ix);
    self.sizes.remove(panel_ix);
    if let Some(resizing_panel_ix) = self.resizing_panel_ix
      && resizing_panel_ix > panel_ix
    {
      self.resizing_panel_ix = Some(resizing_panel_ix - 1);
    }
    self.adjust_to_container_size(cx);
  }

  #[allow(dead_code)]
  pub(crate) fn replace_panel(
    &mut self, panel_ix: usize, panel: ResizablePanelState, cx: &mut Context<Self>,
  ) {
    let old_size = self.sizes[panel_ix];

    self.panels[panel_ix] = panel;
    self.sizes[panel_ix] = old_size;
    self.adjust_to_container_size(cx);
  }

  #[allow(dead_code)]
  pub(crate) fn clear(&mut self) {
    self.panels.clear();
    self.sizes.clear();
  }

  #[inline]
  pub(crate) fn container_size(&self) -> Pixels {
    self.bounds.size.along(self.axis)
  }

  pub(crate) fn done_resizing(&mut self, cx: &mut Context<Self>) {
    for (ix, panel) in self.panels.iter().enumerate() {
      if let Some(size) = panel.size {
        self.sizes[ix] = size;
      }
    }

    self.resizing_panel_ix = None;
    cx.notify();
    cx.emit(ResizablePanelEvent::Resized);
  }

  fn panel_size_range(&self, ix: usize) -> Range<Pixels> {
    let Some(panel) = self.panels.get(ix) else {
      return PANEL_MIN_SIZE..Pixels::MAX;
    };

    panel.size_range.clone()
  }

  fn display_sizes(&self) -> Vec<Pixels> {
    self
      .panels
      .iter()
      .enumerate()
      .map(|(ix, panel)| panel.size.unwrap_or(self.sizes[ix]))
      .collect()
  }

  fn resize_panel(
    &mut self, ix: usize, size: Pixels, _window: &mut Window, _cx: &mut Context<Self>,
  ) {
    if ix >= self.panels.len().saturating_sub(1) {
      return;
    }

    let size_range = self.panel_size_range(ix);
    let size = size.clamp(size_range.start, size_range.end);
    if self.pending_resize == Some((ix, size)) {
      return;
    }

    self.pending_resize = Some((ix, size));
    // The drag is active, so dispatch_mouse_event will call window.refresh()
    // for every MouseMoveEvent. Rely on that instead of notifying this entity
    // on every high-frequency mouse report.
  }

  /// Apply the resize target captured by the latest mouse-move event. This
  /// is called once per frame during render, so many mouse events coalesce
  /// into a single redistribution pass.
  pub(crate) fn apply_pending_resize(&mut self, _cx: &mut Context<Self>) {
    let Some((mut ix, size)) = self.pending_resize.take() else {
      return;
    };

    let old_sizes = self.display_sizes();
    if ix >= old_sizes.len() - 1 {
      return;
    }

    let container_size = self.container_size();
    let move_changed = size - old_sizes[ix];
    if move_changed == px(0.) {
      return;
    }

    let size_range = self.panel_size_range(ix);
    let new_size = size.clamp(size_range.start, size_range.end);
    let is_expand = move_changed > px(0.);

    let main_ix = ix;
    let mut new_sizes = old_sizes.clone();

    if is_expand {
      let mut changed = new_size - old_sizes[ix];
      new_sizes[ix] = new_size;

      while changed > px(0.) && ix < old_sizes.len() - 1 {
        ix += 1;
        let size_range = self.panel_size_range(ix);
        let available_size = (new_sizes[ix] - size_range.start).max(px(0.));
        let to_reduce = changed.min(available_size);
        new_sizes[ix] -= to_reduce;
        changed -= to_reduce;
      }
    } else {
      let mut changed = new_size - size;
      new_sizes[ix] = new_size;

      while changed > px(0.) && ix > 0 {
        ix -= 1;
        let size_range = self.panel_size_range(ix);
        let available_size = (new_sizes[ix] - size_range.start).max(px(0.));
        let to_reduce = changed.min(available_size);
        changed -= to_reduce;
        new_sizes[ix] -= to_reduce;
      }

      new_sizes[main_ix + 1] += old_sizes[main_ix] - size - changed;
    }

    let total_size: Pixels = new_sizes.iter().map(|s| s.as_f32()).sum::<f32>().into();
    if total_size > container_size {
      let overflow = total_size - container_size;
      new_sizes[main_ix] = (new_sizes[main_ix] - overflow).max(size_range.start);
    }

    for (i, _) in old_sizes.iter().enumerate() {
      let size = new_sizes[i];
      self.panels[i].size = Some(size);
    }
    // This runs during render; the current frame already picks up the new
    // sizes, so do not schedule another notification here.
  }

  fn adjust_to_container_size(&mut self, cx: &mut Context<Self>) {
    if self.container_size().is_zero() {
      return;
    }

    let container_size = self.container_size();
    let total_size = px(self.sizes.iter().map(|s| s.as_f32()).sum::<f32>());

    for i in 0..self.panels.len() {
      let size = self.sizes[i];
      let ratio = size / total_size;
      let new_size = container_size * ratio;

      self.sizes[i] = new_size;
      self.panels[i].size = Some(new_size);
    }
    cx.notify();
  }
}

impl EventEmitter<ResizablePanelEvent> for ResizableState {}

#[derive(Debug, Clone, Default)]
pub(crate) struct ResizablePanelState {
  pub size: Option<Pixels>,
  pub size_range: Range<Pixels>,
  bounds: Bounds<Pixels>,
}
