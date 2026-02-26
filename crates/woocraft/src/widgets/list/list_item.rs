use gpui::{
  AnyElement, App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement, MouseMoveEvent,
  ParentElement, RenderOnce, Stateful, StatefulInteractiveElement as _, StyleRefinement, Styled,
  Window, div, prelude::FluentBuilder as _,
};
use smallvec::SmallVec;

use crate::{
  ActiveTheme, Icon, Selectable, Sizable, Size, StyleSized, StyledExt, TableThemeExt, h_flex,
};

type ListItemClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type ListItemMouseEnterHandler = Box<dyn Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static>;
type ListItemSuffixBuilder = Box<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ListItemMode {
  #[default]
  Entry,
  Separator,
}

impl ListItemMode {
  #[inline]
  fn is_separator(&self) -> bool {
    matches!(self, ListItemMode::Separator)
  }
}

#[derive(IntoElement)]
pub struct ListItem {
  base: Stateful<Div>,
  mode: ListItemMode,
  style: StyleRefinement,
  size: Size,
  disabled: bool,
  selected: bool,
  secondary_selected: bool,
  confirmed: bool,
  check_icon: Option<Icon>,
  on_click: Option<ListItemClickHandler>,
  on_mouse_enter: Option<ListItemMouseEnterHandler>,
  suffix: Option<ListItemSuffixBuilder>,
  children: SmallVec<[AnyElement; 2]>,
}

impl ListItem {
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id: ElementId = id.into();
    Self {
      mode: ListItemMode::Entry,
      base: h_flex().id(id),
      style: StyleRefinement::default(),
      size: Size::Medium,
      disabled: false,
      selected: false,
      secondary_selected: false,
      confirmed: false,
      on_click: None,
      on_mouse_enter: None,
      check_icon: None,
      suffix: None,
      children: SmallVec::new(),
    }
  }

  /// Set this list item as a separator, it cannot be selected.
  pub fn separator(mut self) -> Self {
    self.mode = ListItemMode::Separator;
    self
  }

  /// Set to show check icon, default is None.
  pub fn check_icon(mut self, icon: impl Into<Icon>) -> Self {
    self.check_icon = Some(icon.into());
    self
  }

  /// Set ListItem as selected style.
  pub fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  /// Set ListItem as confirmed style.
  pub fn confirmed(mut self, confirmed: bool) -> Self {
    self.confirmed = confirmed;
    self
  }

  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  /// Set suffix element for the list item.
  pub fn suffix<F, E>(mut self, builder: F) -> Self
  where
    F: Fn(&mut Window, &mut App) -> E + 'static,
    E: IntoElement, {
    self.suffix = Some(Box::new(move |window, cx| {
      builder(window, cx).into_any_element()
    }));
    self
  }

  pub fn on_click(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Box::new(handler));
    self
  }

  pub fn on_mouse_enter(
    mut self, handler: impl Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_mouse_enter = Some(Box::new(handler));
    self
  }
}

impl_sizable!(ListItem);

impl Selectable for ListItem {
  fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  fn is_selected(&self) -> bool {
    self.selected
  }

  fn secondary_selected(mut self, selected: bool) -> Self {
    self.secondary_selected = selected;
    self
  }
}

impl_styled!(ListItem);

impl ParentElement for ListItem {
  fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for ListItem {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let is_active = self.confirmed || self.selected;
    let is_selectable = !(self.disabled || self.mode.is_separator());
    let base_fg = cx.theme().foreground;
    let hover_bg = cx.theme().table_hover();
    let active_bg = cx.theme().table_active();

    self
      .base
      .relative()
      .container_size(self.size)
      .container_gap(self.size)
      .text_base()
      .text_color(base_fg)
      .relative()
      .items_center()
      .justify_between()
      .refine_style(&self.style)
      .when(is_selectable, |this| {
        this
          .when_some(self.on_click, |this, on_click| this.on_click(on_click))
          .when_some(self.on_mouse_enter, |this, on_mouse_enter| {
            this.on_mouse_move(move |ev, window, cx| (on_mouse_enter)(ev, window, cx))
          })
          .when(!is_active, |this| {
            this
              .hover(move |this| this.bg(hover_bg))
              .active(move |this| this.bg(active_bg))
          })
      })
      .when(!is_selectable, |this| {
        this.text_color(cx.theme().muted_foreground)
      })
      .child(
        h_flex()
          .w_full()
          .items_center()
          .justify_between()
          .gap_x(self.size.component_gap())
          .child(div().w_full().children(self.children))
          .when_some(self.check_icon, |this, icon| {
            this.child(
              div()
                .w_5()
                .items_center()
                .justify_center()
                .when(self.confirmed, |this| {
                  this.child(icon.small().text_color(cx.theme().muted_foreground))
                }),
            )
          }),
      )
      .when_some(self.suffix, |this, suffix| this.child(suffix(window, cx)))
      .map(|this| {
        if is_selectable && self.selected {
          this.bg(active_bg)
        } else if is_selectable && self.secondary_selected {
          this.bg(hover_bg)
        } else {
          this
        }
      })
  }
}
