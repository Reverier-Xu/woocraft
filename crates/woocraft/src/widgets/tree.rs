//! styled tree face over the base tree behavior model.
//!
//! [`crate::base::tree`] owns everything behavioral — the flattened entry
//! list, selection, expand/collapse with keyboard navigation, reveal, and
//! scroll positioning — while this module contributes the themed row:
//! depth indentation, a collapsing chevron for folders, and the semantic
//! selected / right-clicked / disabled presentations.
//!
//! applications that need custom rows pass their own renderer through
//! [`Tree::item`], exactly as on the base element.
//!
//! ```rust,ignore
//! let state = cx.new(|_| {
//!   TreeState::default().items(vec![
//!     TreeItem::new("src", "src").child(TreeItem::new("lib", "lib.rs")),
//!   ])
//! });
//! let tree = Tree::new(&state);
//! ```

use gpui::{
  App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Refineable as _,
  RenderOnce, Role, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _, px, relative, rems,
};

// the behavioral tree vocabulary is re-exported so applications can drive
// the state entity without touching gpui-base directly.
pub use crate::base::{TreeEntry, TreeEntryState, TreeEvent, TreeItem, TreeState};
use crate::{
  ActiveTheme, Icon, IconName,
  base::Tree as BaseTree,
  theme::{opacity, with_alpha},
};

/// the styled tree: wraps the base [`TreeState`] entity with the themed row
/// renderer. the element id doubles as the debug selector for headless
/// geometry tests.
#[derive(IntoElement)]
pub struct Tree {
  id: ElementId,
  base: BaseTree,
  style: StyleRefinement,
}

impl Tree {
  /// creates a tree bound to its state entity with the themed row renderer.
  ///
  /// the wrapper gives the base element's internal virtual list a flex
  /// growth chain so rows fill whatever definite height the caller gives
  /// the tree; place the tree in a sized flex container.
  pub fn new(id: impl Into<ElementId>, state: &gpui::Entity<TreeState>) -> Self {
    Self {
      id: id.into(),
      base: BaseTree::new(state)
        .item(themed_item)
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .list_style(fill_refinement()),
      style: StyleRefinement::default(),
    }
  }

  /// replaces the themed row renderer with an application-owned one.
  pub fn item<R>(mut self, render_item: R) -> Self
  where
    R: Fn(usize, &TreeEntry, TreeEntryState, &mut Window, &mut App) -> gpui::AnyElement + 'static,
  {
    self.base = self.base.item(render_item);
    self
  }
}

impl Styled for Tree {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Tree {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let id = self.id.clone();
    let mut container = div()
      .id(id.clone())
      .debug_selector(move || id.to_string())
      .role(Role::Tree)
      .flex()
      .flex_col()
      .min_h_0()
      .text_size(theme.font_size)
      .font_family(theme.font_family.clone())
      .text_color(theme.foreground)
      .child(self.base);

    container.style().refine(&self.style);
    container
  }
}

/// the flex-grow refinement applied to the base element's internal virtual
/// list so it fills the tree's definite height instead of collapsing to
/// zero and clipping every row.
fn fill_refinement() -> StyleRefinement {
  StyleRefinement {
    flex_grow: Some(1.),
    flex_shrink: Some(1.),
    flex_basis: Some(px(0.).into()),
    ..Default::default()
  }
}

/// the themed row: depth indentation, a folder chevron, and the semantic
/// presentations. the base element owns hit-testing and interaction; this
/// face only paints.
fn themed_item(
  ix: usize, entry: &TreeEntry, entry_state: TreeEntryState, _: &mut Window, cx: &mut App,
) -> gpui::AnyElement {
  let theme = cx.theme();
  let (fg, muted_foreground, primary) = (theme.foreground, theme.muted_foreground, theme.primary);
  let depth = entry.depth();
  let folder = entry.is_folder();
  let expanded = entry.is_expanded();
  let disabled = entry.is_disabled();
  let selected = entry_state.is_selected();
  let right_clicked = entry_state.is_right_clicked();

  let hover_bg = with_alpha(fg, opacity::transparent::HOVER);
  let selected_bg = with_alpha(primary, opacity::transparent::ACTIVE);

  div()
    .id(ElementId::from(("tree-row", ix)))
    .debug_selector(move || format!("tree-row-{ix}"))
    .flex()
    .flex_row()
    .items_center()
    .gap(rems(0.5))
    .w_full()
    .min_h(rems(2.))
    // one rem of indent per level on top of the shared 0.5rem padding.
    .pl(rems(0.5 + depth as f32))
    .pr(rems(0.5))
    .py(rems(0.25))
    .line_height(relative(1.5))
    .rounded(theme.radius)
    .text_color(if disabled { muted_foreground } else { fg })
    .when(selected, |this| this.bg(selected_bg))
    .when(right_clicked && !selected, |this| this.bg(hover_bg))
    .when(!disabled && !selected, |this| {
      this.hover(move |s| s.bg(hover_bg))
    })
    .when(folder, |this| {
      this.child(if expanded {
        Icon::new(IconName::ChevronDown)
      } else {
        Icon::new(IconName::ChevronRight)
      })
    })
    .child(div().truncate().child(entry.item().label.clone()))
    .into_any_element()
}

#[cfg(test)]
mod tests {
  use gpui::{
    AppContext as _, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled, div,
    rems,
  };

  use super::Tree;
  use crate::base::{TreeItem, TreeState};

  struct Host {
    state: gpui::Entity<TreeState>,
  }

  impl Render for Host {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
      div()
        .debug_selector(|| "tree-host".into())
        .size_full()
        .flex()
        .flex_col()
        .p_4()
        .child(Tree::new("tree", &self.state).flex_1())
    }
  }

  fn demo_state(cx: &mut gpui::Context<Host>) -> gpui::Entity<TreeState> {
    cx.new(|cx| {
      TreeState::new(cx).items(vec![
        TreeItem::new("src", "src")
          .expanded(true)
          .child(TreeItem::new("lib", "lib.rs"))
          .child(TreeItem::new("main", "main.rs")),
        TreeItem::new("readme", "README.md"),
      ])
    })
  }

  #[gpui::test]
  fn a_tree_row_is_two_rem_tall(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (_view, cx) = cx.add_window_view(|_, cx| {
      let state = demo_state(cx);
      Host { state }
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let rem = cx.update(|window, _| window.rem_size());
    let bounds = cx
      .debug_bounds("tree-row-1")
      .expect("the expanded folder's first child paints");
    assert_eq!(
      bounds.size.height,
      rems(2.).to_pixels(rem),
      "one-line tree rows sit on the 2rem control height"
    );
    assert!(
      cx.debug_bounds("tree-row-0").is_some(),
      "the root row paints"
    );
  }
}
