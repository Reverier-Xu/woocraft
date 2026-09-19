//! Compact icon-plus-text row for list entries, menu items, and captions.
//!
//! [`IconLabel`] is a display-only convenience: an optional icon and a
//! truncated label on one line, optionally clickable. For fully interactive
//! rows use [`Button`](crate::Button).

use std::rc::Rc;

use gpui::{
  AnyElement, App, ClickEvent, ElementId, Hsla, InteractiveElement as _, IntoElement,
  ParentElement, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement,
  Styled, Window, prelude::FluentBuilder as _, rems,
};

use crate::{
  ActiveTheme, Icon,
  base::{StyledExt, h_flex},
  theme::opacity,
};

type IconLabelClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct IconLabel {
  id: ElementId,
  label: Option<SharedString>,
  icon: Option<Icon>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
  disabled: bool,
  selected: bool,
  on_click: Option<IconLabelClickHandler>,
}

impl IconLabel {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      label: None,
      icon: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
      disabled: false,
      selected: false,
      on_click: None,
    }
  }

  /// sets the text label.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// sets the leading icon.
  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// makes the row clickable.
  pub fn on_click(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Rc::new(handler));
    self
  }

  /// renders the row dimmed and non-clickable.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  /// tints the row with the primary color to mark selection.
  pub fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }
}

impl Styled for IconLabel {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for IconLabel {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for IconLabel {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let clickable = !self.disabled && self.on_click.is_some();

    let mut text_color = if self.selected {
      theme.primary
    } else {
      theme.foreground
    };

    if self.disabled {
      text_color = Hsla {
        a: opacity::DISABLED,
        ..text_color
      };
    }

    h_flex()
      .id(self.id)
      .items_center()
      .gap(rems(0.5))
      .px(rems(0.25))
      .py(rems(0.125))
      .truncate()
      .min_w_0()
      .text_color(text_color)
      .when(clickable, |this| this.cursor_pointer())
      .when_some(self.icon, |this, icon| this.child(icon))
      .when_some(self.label, |this, label| this.child(label))
      .children(self.children)
      .when_some(self.on_click.filter(|_| clickable), |this, on_click| {
        this.on_click(move |event, window, cx| on_click(event, window, cx))
      })
      .refine_style(&self.style)
  }
}

impl From<IconLabel> for AnyElement {
  fn from(value: IconLabel) -> Self {
    value.into_any_element()
  }
}
