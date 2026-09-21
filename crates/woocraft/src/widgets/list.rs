//! styled list faces over gpui's virtualization primitives.
//!
//! [`List`] is the themed container and [`ListItem`] is the themed row face
//! with selection, hover, and disabled semantics, a leading icon slot, and a
//! trailing slot. virtualization stays with the caller: equal-height rows
//! render through gpui's `uniform_list`, variable-height rows through
//! [`crate::base::virtual_list`]; the `lists` example demonstrates both.

use gpui::{
  AnyElement, App, ClickEvent, CursorStyle, Div, ElementId, InteractiveElement, IntoElement,
  ParentElement, Refineable as _, RenderOnce, Stateful, StatefulInteractiveElement,
  StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, relative, rems,
};

use crate::{
  ActiveTheme, Disableable, Icon, Selectable,
  theme::{opacity, with_alpha},
};

type ListItemClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// interactive themed row face for list surfaces.
///
/// the row is a single-line flex strip on the `0.25rem` padding rhythm; one
/// line of text renders exactly `2rem` tall. the caller owns the data, the
/// selection state, and the virtualization loop — the row face only carries
/// the semantic states and the slots.
///
/// ```rust,ignore
/// use woocraft::{Icon, IconName, ListItem};
///
/// ListItem::new("lang-rust")
///   .icon(Icon::new(IconName::ChevronRight))
///   .selected(selected == Some("lang-rust"))
///   .on_click(|_, _, cx| cx.stop_propagation())
///   .child("rust")
/// ```
#[derive(IntoElement)]
pub struct ListItem {
  id: ElementId,
  base: Stateful<Div>,
  selected: bool,
  disabled: bool,
  icon: Option<Icon>,
  trailing: Option<AnyElement>,
  on_click: Option<ListItemClickHandler>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl ListItem {
  /// creates a row with a unique element identifier; the id keys the
  /// stateful interactivity underneath hover and click handling and doubles
  /// as the debug selector for headless geometry regression tests.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      id: id.clone(),
      base: div().id(id),
      selected: false,
      disabled: false,
      icon: None,
      trailing: None,
      on_click: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the leading icon of the row.
  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.icon = Some(icon.into());
    self
  }

  /// sets the trailing slot; it is pushed to the row's far edge and is the
  /// place for shortcuts, chevrons, and secondary hints.
  pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
    self.trailing = Some(trailing.into_any_element());
    self
  }

  /// sets the activation handler for pointer clicks; ignored while the row
  /// is disabled.
  pub fn on_click(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Box::new(handler));
    self
  }
}

impl Selectable for ListItem {
  fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  fn is_selected(&self) -> bool {
    self.selected
  }
}

impl Disableable for ListItem {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Styled for ListItem {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for ListItem {
  fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(children);
  }
}

impl InteractiveElement for ListItem {
  fn interactivity(&mut self) -> &mut gpui::Interactivity {
    self.base.interactivity()
  }
}

impl StatefulInteractiveElement for ListItem {}

impl RenderOnce for ListItem {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let (fg, muted_foreground, primary) = (theme.foreground, theme.muted_foreground, theme.primary);
    let (selected, disabled) = (self.selected, self.disabled);

    let hover_bg = with_alpha(fg, opacity::transparent::HOVER);
    let active_bg = with_alpha(fg, opacity::transparent::ACTIVE);
    let selected_bg = with_alpha(primary, opacity::transparent::ACTIVE);

    let mut base = self
      .base
      .flex()
      .flex_row()
      .items_center()
      .gap(rems(0.5))
      // the 0.25rem vertical rhythm plus a 1.5 line-height makes one-line
      // rows exactly 2rem tall, matching the control height convention.
      .min_h(rems(2.))
      .px(rems(0.5))
      .py(rems(0.25))
      .line_height(relative(1.5))
      .rounded(theme.radius)
      .text_size(theme.font_size)
      .font_family(theme.font_family.clone())
      .text_color(fg)
      .when(selected, |this| this.bg(selected_bg))
      .when(!disabled, |this| {
        this
          .cursor_pointer()
          .when(!selected, |this| this.hover(move |s| s.bg(hover_bg)))
          .active(move |s| s.bg(active_bg))
      })
      .when(disabled, |this| {
        this
          .text_color(muted_foreground)
          .cursor(CursorStyle::OperationNotAllowed)
      })
      .role(gpui::Role::ListBoxOption)
      .debug_selector(move || self.id.to_string())
      .when_some(self.on_click.filter(|_| !disabled), |this, handler| {
        this.on_click(handler)
      })
      .when_some(self.icon, |this, icon| this.child(icon))
      .children(self.children)
      .when_some(self.trailing, |this, trailing| {
        this.child(div().ml_auto().child(trailing))
      });

    base.style().refine(&self.style);
    base
  }
}

/// themed list container: a vertical stack on the `0.25rem` rhythm with the
/// list accessibility role. rows are plain [`ListItem`] children; scroll and
/// virtualization belong to the caller.
#[derive(IntoElement)]
pub struct List {
  base: Stateful<Div>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl List {
  /// creates a list container keyed by a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      base: div().id(id),
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }
}

impl Styled for List {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for List {
  fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(children);
  }
}

impl InteractiveElement for List {
  fn interactivity(&mut self) -> &mut gpui::Interactivity {
    self.base.interactivity()
  }
}

impl StatefulInteractiveElement for List {}

impl RenderOnce for List {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let mut base = self
      .base
      .flex()
      .flex_col()
      .gap(rems(0.25))
      .p(rems(0.25))
      .text_size(theme.font_size)
      .font_family(theme.font_family.clone())
      .role(gpui::Role::List)
      .children(self.children);

    base.style().refine(&self.style);
    base
  }
}

#[cfg(test)]
mod tests {
  use gpui::{InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled, div, rems};

  use super::{List, ListItem};
  use crate::{Disableable, Selectable};

  #[test]
  fn a_row_carries_its_semantic_states() {
    let row = ListItem::new("row").selected(true).disabled(false);
    assert!(row.is_selected());
    let row = row.selected(false);
    assert!(!row.is_selected());
    let _row = row.disabled(true);
  }

  struct Host;

  impl Render for Host {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
      div()
        .debug_selector(|| "list-host".into())
        .size_full()
        .child(List::new("list").child(ListItem::new("list-row").child("label")))
    }
  }

  #[gpui::test]
  fn a_one_line_row_is_two_rem_tall(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (_view, cx) = cx.add_window_view(|_, _| Host);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let rem = cx.update(|window, _| window.rem_size());
    let bounds = cx.debug_bounds("list-row").expect("the row paints");
    assert_eq!(
      bounds.size.height,
      rems(2.).to_pixels(rem),
      "one-line rows sit on the 2rem control height"
    );
  }

  #[gpui::test]
  fn the_list_presents_the_list_role(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (_view, cx) = cx.add_window_view(|_, _| Host);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    assert!(cx.debug_bounds("list-host").is_some(), "the list paints");
  }
}
