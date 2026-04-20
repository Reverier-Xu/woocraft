use gpuim::{App, AppContext, Axis, Bounds, Entity, Pixels, WeakEntity, Window, point, px, size};
use serde::{Deserialize, Serialize};

use super::{Dock, DockArea, DockItem, Panel, PanelRegistry};
use crate::DockPlacement;

/// Used to serialize and deserialize the DockArea
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct DockAreaState {
  /// The version is used to mark this persisted state is compatible with the
  /// current version For example, some times we many totally changed the
  /// structure of the Panel, then we can compare the version to decide
  /// whether we can use the state or ignore.
  #[serde(default)]
  pub version: Option<usize>,
  pub center: PanelState,
  /// Whether the center area is enabled, default is true.
  #[serde(default = "default_center_enabled")]
  pub center_enabled: bool,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub left_dock: Option<DockState>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub right_dock: Option<DockState>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub bottom_dock: Option<DockState>,
}

fn default_center_enabled() -> bool {
  true
}

/// Used to serialize and deserialize the Dock
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DockState {
  panel: PanelState,
  placement: DockPlacement,
  size: Pixels,
  #[serde(default)]
  collapsed: bool,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  open: Option<bool>,
}

impl DockState {
  fn is_collapsed(&self) -> bool {
    self.open.map(|open| !open).unwrap_or(self.collapsed)
  }

  pub fn new(dock: Entity<Dock>, cx: &App) -> Self {
    let dock = dock.read(cx);

    Self {
      placement: dock.placement,
      size: dock.size,
      collapsed: dock.collapsed,
      open: None,
      panel: dock.panel.view().dump(cx),
    }
  }

  /// Convert the DockState to Dock
  pub fn to_dock(
    &self, dock_area: WeakEntity<DockArea>, window: &mut Window, cx: &mut App,
  ) -> Entity<Dock> {
    let item = self.panel.to_item(dock_area.clone(), window, cx);
    cx.new(|cx| {
      Dock::from_state(
        dock_area.clone(),
        self.placement,
        self.size,
        item,
        self.is_collapsed(),
        window,
        cx,
      )
    })
  }
}

/// Used to serialize and deserialize the DockerItem
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PanelState {
  pub panel_name: String,
  pub children: Vec<PanelState>,
  pub info: PanelInfo,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileMeta {
  pub bounds: Bounds<Pixels>,
  pub z_index: usize,
}

impl Default for TileMeta {
  fn default() -> Self {
    Self {
      bounds: Bounds {
        origin: point(px(10.), px(10.)),
        size: size(px(200.), px(200.)),
      },
      z_index: 0,
    }
  }
}

impl From<Bounds<Pixels>> for TileMeta {
  fn from(bounds: Bounds<Pixels>) -> Self {
    Self { bounds, z_index: 0 }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PanelInfo {
  #[serde(rename = "stack")]
  Stack {
    sizes: Vec<Pixels>,
    axis: usize, // 0 for horizontal, 1 for vertical
  },
  #[serde(rename = "tabs")]
  Tabs { active_index: usize },
  #[serde(rename = "panel")]
  Panel(serde_json::Value),
  #[serde(rename = "tiles")]
  Tiles { metas: Vec<TileMeta> },
}

impl PanelInfo {
  pub fn stack(sizes: Vec<Pixels>, axis: Axis) -> Self {
    Self::Stack {
      sizes,
      axis: if axis == Axis::Horizontal { 0 } else { 1 },
    }
  }

  pub fn tabs(active_index: usize) -> Self {
    Self::Tabs { active_index }
  }

  pub fn panel(info: serde_json::Value) -> Self {
    Self::Panel(info)
  }

  pub fn tiles(metas: Vec<TileMeta>) -> Self {
    Self::Tiles { metas }
  }

  pub fn axis(&self) -> Option<Axis> {
    match self {
      Self::Stack { axis, .. } => Some(if *axis == 0 {
        Axis::Horizontal
      } else {
        Axis::Vertical
      }),
      _ => None,
    }
  }

  pub fn sizes(&self) -> Option<&Vec<Pixels>> {
    match self {
      Self::Stack { sizes, .. } => Some(sizes),
      _ => None,
    }
  }

  pub fn active_index(&self) -> Option<usize> {
    match self {
      Self::Tabs { active_index } => Some(*active_index),
      _ => None,
    }
  }
}

impl Default for PanelState {
  fn default() -> Self {
    Self {
      panel_name: "".to_string(),
      children: Vec::new(),
      info: PanelInfo::Panel(serde_json::Value::Null),
    }
  }
}

impl PanelState {
  pub fn new<P: Panel>(panel: &P) -> Self {
    Self {
      panel_name: panel.panel_name().to_string(),
      ..Default::default()
    }
  }

  pub fn add_child(&mut self, panel: PanelState) {
    self.children.push(panel);
  }

  pub fn to_item(
    &self, dock_area: WeakEntity<DockArea>, window: &mut Window, cx: &mut App,
  ) -> DockItem {
    let info = self.info.clone();

    let items: Vec<DockItem> = self
      .children
      .iter()
      .map(|child| child.to_item(dock_area.clone(), window, cx))
      .collect();

    match info {
      PanelInfo::Stack { sizes, axis } => {
        let axis = if axis == 0 {
          Axis::Horizontal
        } else {
          Axis::Vertical
        };
        let sizes = sizes.iter().map(|s| Some(*s)).collect();
        DockItem::split_with_sizes(axis, items, sizes, &dock_area, window, cx)
      }
      PanelInfo::Tabs { active_index } => {
        if items.len() == 1 {
          return items[0].clone();
        }

        let items = items
          .iter()
          .flat_map(|item| match item {
            DockItem::Tabs { items, .. } => items.clone(),
            _ => {
              // ignore invalid panels in tabs
              vec![]
            }
          })
          .collect();

        DockItem::tabs(items, &dock_area, window, cx).active_index(active_index, cx)
      }
      PanelInfo::Panel(_) => {
        let view =
          PanelRegistry::build_panel(&self.panel_name, dock_area.clone(), self, &info, window, cx);
        DockItem::tabs(vec![view.into()], &dock_area, window, cx)
      }
      PanelInfo::Tiles { metas } => DockItem::tiles(items, metas, &dock_area, window, cx),
    }
  }
}
