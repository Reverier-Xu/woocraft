use std::rc::Rc;

use gpui::{
  AnyElement, App, ClickEvent, Div, InteractiveElement, IntoElement, MouseButton, ParentElement,
  RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
  prelude::FluentBuilder as _,
};

use crate::{
  Button, ButtonVariants as _, Disableable, Icon, IconName, Selectable, Sizable, Size, StyleSized,
  h_flex,
};

type TabClickHandler = dyn Fn(&ClickEvent, &mut Window, &mut App);
type TabCloseHandler = dyn Fn(&ClickEvent, &mut Window, &mut App);

/// A Tab element for the [`super::TabBar`].
#[derive(IntoElement)]
pub struct Tab {
  ix: usize,
  base: Div,
  pub(super) label: Option<SharedString>,
  icon: Option<Icon>,
  prefix: Option<AnyElement>,
  suffix: Option<AnyElement>,
  children: Vec<AnyElement>,
  size: Size,
  pub(super) disabled: bool,
  pub(super) selected: bool,
  closable: bool,
  on_click: Option<Rc<TabClickHandler>>,
  on_close: Option<Rc<TabCloseHandler>>,
}

impl From<&'static str> for Tab {
  fn from(label: &'static str) -> Self {
    Self::new().label(label)
  }
}

impl From<String> for Tab {
  fn from(label: String) -> Self {
    Self::new().label(label)
  }
}

impl From<SharedString> for Tab {
  fn from(label: SharedString) -> Self {
    Self::new().label(label)
  }
}

impl From<Icon> for Tab {
  fn from(icon: Icon) -> Self {
    Self::default().icon(icon)
  }
}

impl From<IconName> for Tab {
  fn from(icon_name: IconName) -> Self {
    Self::default().icon(Icon::new(icon_name))
  }
}

impl Default for Tab {
  fn default() -> Self {
    Self {
      ix: 0,
      base: h_flex(),
      label: None,
      icon: None,
      children: Vec::new(),
      disabled: false,
      selected: false,
      prefix: None,
      suffix: None,
      size: Size::default(),
      closable: false,
      on_click: None,
      on_close: None,
    }
  }
}

impl Tab {
  /// Create a new tab with a label.
  pub fn new() -> Self {
    Self::default()
  }

  /// Set label for the tab.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// Set icon for the tab.
  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.icon = Some(icon.into());
    self
  }

  /// Set the left side of the tab
  pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
    self.prefix = Some(prefix.into_any_element());
    self
  }

  /// Set the right side of the tab
  pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
    self.suffix = Some(suffix.into_any_element());
    self
  }

  /// Set disabled state to the tab, default false.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  /// Set the click handler for the tab.
  pub fn on_click(
    mut self, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Rc::new(on_click));
    self
  }

  /// Set whether the tab is closable, default is false.
  pub fn closable(mut self, closable: bool) -> Self {
    self.closable = closable;
    self
  }

  /// Set the close handler for the tab.
  pub fn on_close(
    mut self, on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_close = Some(Rc::new(on_close));
    self
  }

  /// Set index to the tab.
  pub(crate) fn ix(mut self, ix: usize) -> Self {
    self.ix = ix;
    self
  }
}

impl ParentElement for Tab {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Selectable for Tab {
  fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  fn is_selected(&self) -> bool {
    self.selected
  }
}

impl InteractiveElement for Tab {
  fn interactivity(&mut self) -> &mut gpui::Interactivity {
    self.base.interactivity()
  }
}

impl StatefulInteractiveElement for Tab {}

impl Styled for Tab {
  fn style(&mut self) -> &mut gpui::StyleRefinement {
    self.base.style()
  }
}

impl Sizable for Tab {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl RenderOnce for Tab {
  fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
    let button_id = SharedString::from(format!("tab-{}", self.ix));
    let close_button_id = SharedString::from(format!("tab-close-{}", self.ix));

    let mut button = Button::new(button_id)
      .with_size(self.size)
      .flat()
      .selected(self.selected)
      .disabled(self.disabled)
      .tab_stop(false);

    if let Some(on_click) = self.on_click {
      button = button.on_click(move |event, window, cx| on_click(event, window, cx));
    }
    if let Some(icon) = self.icon {
      button = button.child(icon);
    }
    if let Some(label) = self.label {
      button = button.child(label);
    }
    if let Some(prefix) = self.prefix {
      button = button.child(prefix);
    }
    button = button.children(self.children);
    if let Some(suffix) = self.suffix {
      button = button.child(suffix);
    }

    let close_button = if self.closable {
      let mut close_btn = Button::new(close_button_id)
        .small()
        .flat()
        .icon(IconName::Dismiss)
        .tab_stop(false);

      if let Some(on_close) = self.on_close {
        close_btn = close_btn.on_click(move |event, window, cx| {
          cx.stop_propagation();
          on_close(event, window, cx);
        });
      }

      Some(close_btn)
    } else {
      None
    };
    button = button.when_some(close_button, |this, btn| this.pr_1().child(btn));

    self
      .base
      .id(self.ix)
      .container_h(self.size)
      .items_center()
      .child(button)
      .on_mouse_down(MouseButton::Left, |_, _, cx| {
        cx.stop_propagation();
      })
  }
}
