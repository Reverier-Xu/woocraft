use gpui::{
  AnyElement, App, Bounds, ClickEvent, Entity, EventEmitter, Hsla, InteractiveElement as _,
  IntoElement, Length, ParentElement, Pixels, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Window, prelude::FluentBuilder as _,
  div,
};

use crate::{
  ActiveTheme, Button, ButtonVariants, Disableable, ElementExt, Icon, IconName, Popover,
  Selectable, Sizable, Size, StyleSized, StyledExt, h_flex, v_flex,
};

#[derive(Clone, Debug, PartialEq)]
pub struct SelectItem {
  pub label: SharedString,
  pub value: SharedString,
}

impl SelectItem {
  pub fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
    Self {
      label: label.into(),
      value: value.into(),
    }
  }
}

impl From<&str> for SelectItem {
  fn from(value: &str) -> Self {
    Self::new(value.to_string(), value.to_string())
  }
}

impl From<String> for SelectItem {
  fn from(value: String) -> Self {
    Self::new(value.clone(), value)
  }
}

impl From<SharedString> for SelectItem {
  fn from(value: SharedString) -> Self {
    Self::new(value.clone(), value)
  }
}

#[derive(Clone)]
pub enum SelectEvent {
  Change(SelectItem),
}

pub struct SelectState {
  items: Vec<SelectItem>,
  selected: Option<usize>,
  open: bool,
  trigger_bounds: Bounds<Pixels>,
  placeholder: SharedString,
  disabled: bool,
}

impl SelectState {
  pub fn new(items: Vec<SelectItem>, placeholder: impl Into<SharedString>) -> Self {
    Self {
      items,
      selected: None,
      open: false,
      trigger_bounds: Bounds::default(),
      placeholder: placeholder.into(),
      disabled: false,
    }
  }

  pub fn set_items(&mut self, items: Vec<SelectItem>) {
    self.items = items;
    if let Some(ix) = self.selected {
      if ix >= self.items.len() {
        self.selected = None;
      }
    }
  }

  pub fn set_selected_index(&mut self, selected: Option<usize>) {
    self.selected = selected;
  }

  pub fn selected_index(&self) -> Option<usize> {
    self.selected
  }

  pub fn selected_item(&self) -> Option<&SelectItem> {
    self.selected.and_then(|ix| self.items.get(ix))
  }

  pub fn value(&self) -> Option<SharedString> {
    self.selected_item().map(|item| item.value.clone())
  }

  pub fn set_selected_value(&mut self, value: impl AsRef<str>) {
    let value = value.as_ref();
    self.selected = self.items.iter().position(|item| item.value.as_ref() == value);
  }

  pub fn set_disabled(&mut self, disabled: bool) {
    self.disabled = disabled;
    if disabled {
      self.open = false;
    }
  }

  pub fn set_trigger_bounds(&mut self, bounds: Bounds<Pixels>) {
    self.trigger_bounds = bounds;
  }
}

impl EventEmitter<SelectEvent> for SelectState {}

#[derive(IntoElement)]
pub struct Select {
  id: SharedString,
  state: Entity<SelectState>,
  style: StyleRefinement,
  size: Size,
  disabled: bool,
  icon: Option<Icon>,
  cleanable: bool,
  placeholder: Option<SharedString>,
  title_prefix: Option<SharedString>,
  empty: Option<AnyElement>,
  menu_width: Length,
  appearance: bool,
}

impl Select {
  pub fn new(id: impl Into<SharedString>, state: &Entity<SelectState>) -> Self {
    Self {
      id: id.into(),
      state: state.clone(),
      style: StyleRefinement::default(),
      size: Size::default(),
      disabled: false,
      icon: None,
      cleanable: false,
      placeholder: None,
      title_prefix: None,
      empty: None,
      menu_width: Length::Auto,
      appearance: true,
    }
  }

  pub fn menu_width(mut self, width: impl Into<Length>) -> Self {
    self.menu_width = width.into();
    self
  }

  pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
    self.placeholder = Some(placeholder.into());
    self
  }

  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.icon = Some(icon.into());
    self
  }

  pub fn title_prefix(mut self, prefix: impl Into<SharedString>) -> Self {
    self.title_prefix = Some(prefix.into());
    self
  }

  pub fn cleanable(mut self, cleanable: bool) -> Self {
    self.cleanable = cleanable;
    self
  }

  pub fn empty(mut self, el: impl IntoElement) -> Self {
    self.empty = Some(el.into_any_element());
    self
  }

  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }
}

impl Disableable for Select {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Sizable for Select {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Styled for Select {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Select {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    self.state.update(cx, |state, _| state.set_disabled(self.disabled));

    let icon = self.icon;
    let cleanable = self.cleanable;
    let placeholder_override = self.placeholder;
    let title_prefix = self.title_prefix;
    let empty = self.empty;
    let menu_width = self.menu_width;
    let appearance = self.appearance;

    let state = self.state.read(cx);
    let items = state.items.clone();
    let selected = state.selected;
    let open = state.open;
    let trigger_bounds = state.trigger_bounds;
    let placeholder = placeholder_override.unwrap_or_else(|| state.placeholder.clone());
    let disabled = state.disabled;
    let _ = state;

    let state_toggle = self.state.clone();

    let text = selected
      .and_then(|ix| items.get(ix).map(|item| item.label.clone()))
      .map(|label| {
        if let Some(prefix) = title_prefix.clone() {
          format!("{}{}", prefix, label).into()
        } else {
          label
        }
      })
      .unwrap_or(placeholder);

    let show_clean = cleanable && selected.is_some() && !disabled;

    v_flex()
      .id(self.id)
      .relative()
      .gap_1()
      .child(
        h_flex()
          .id(("select-trigger", self.state.entity_id().as_u64()))
          .justify_between()
          .items_center()
          .gap_2()
          .input_size(self.size)
          .border_1()
          .border_color(if appearance {
            cx.theme().input
          } else {
            Hsla::transparent_black()
          })
          .bg(if appearance {
            if disabled { cx.theme().muted } else { cx.theme().background }
          } else {
            Hsla::transparent_black()
          })
          .when(appearance && !disabled, |this| this.shadow_xs())
          .text_color(if selected.is_some() {
            cx.theme().foreground
          } else {
            cx.theme().muted_foreground
          })
          .child(div().w_full().truncate().child(text))
          .when(show_clean, |this| {
            this.child(
              Button::new(("select-clear", self.state.entity_id().as_u64()))
                .flat()
                .with_size(self.size)
                .icon(IconName::Dismiss)
                .on_click({
                  let state = self.state.clone();
                  move |_: &ClickEvent, _, cx| {
                    state.update(cx, |state, cx| {
                      state.selected = None;
                      state.open = false;
                      cx.notify();
                    });
                  }
                }),
            )
          })
          .when(!show_clean, |this| {
            this.child(
              icon
                .unwrap_or_else(|| Icon::new(IconName::ChevronDown))
                .with_size(self.size.smaller())
                .text_color(if disabled {
                  cx.theme().muted_foreground.opacity(0.5)
                } else {
                  cx.theme().muted_foreground
                }),
            )
          })
          .when(!disabled, |this| {
            this
              .cursor_pointer()
              .hover(|this| this.border_color(cx.theme().border))
              .on_click(move |_: &ClickEvent, _, cx| {
                state_toggle.update(cx, |state, cx| {
                  state.open = !state.open;
                  cx.notify();
                });
              })
          })
          .on_prepaint({
            let state = self.state.clone();
            move |bounds, _, cx| {
              state.update(cx, |state, _| {
                state.set_trigger_bounds(bounds);
              });
            }
          }),
      )
      .when(open, |this| {
        this.child(
          Popover::new(("select-popover", self.state.entity_id().as_u64()))
            .with_size(self.size)
            .menu_width(menu_width)
            .trigger_bounds(trigger_bounds)
            .flex()
            .flex_col()
            .when(!items.is_empty(), |this| {
              this.children(items.iter().enumerate().map(|(ix, item)| {
                let is_selected = Some(ix) == selected;
                Button::new(("select-item", ix as u64))
                  .flat()
                  .with_size(self.size)
                  .selected(is_selected)
                  .justify_start()
                  .w_full()
                  .label(item.label.clone())
                  .on_click({
                    let state = self.state.clone();
                    let item = item.clone();
                    move |_: &ClickEvent, _, cx| {
                      state.update(cx, |state, cx| {
                        state.selected = Some(ix);
                        state.open = false;
                        cx.emit(SelectEvent::Change(item.clone()));
                        cx.notify();
                      });
                    }
                  })
              }))
            })
            .when(items.is_empty(), |this| {
              if let Some(empty) = empty {
                this.child(empty)
              } else {
                this.child(
                  h_flex()
                    .justify_center()
                    .py_3()
                    .text_color(cx.theme().muted_foreground.opacity(0.6))
                    .child("No items"),
                )
              }
            }),
        )
      })
      .refine_style(&self.style)
  }
}
