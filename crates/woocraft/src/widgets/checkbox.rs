use std::rc::Rc;

use gpui::{
  AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement,
  RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _,
};

use crate::{
  ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable, Size, StyledExt, h_flex,
};

#[derive(IntoElement)]
pub struct Checkbox {
  id: ElementId,
  style: StyleRefinement,
  label: Option<AnyElement>,
  children: Vec<AnyElement>,
  checked: bool,
  disabled: bool,
  size: Size,
  on_click: Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
}

impl Checkbox {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      label: None,
      children: Vec::new(),
      checked: false,
      disabled: false,
      size: Size::default(),
      on_click: None,
    }
  }

  pub fn label(mut self, label: impl IntoElement) -> Self {
    self.label = Some(label.into_any_element());
    self
  }

  pub fn checked(mut self, checked: bool) -> Self {
    self.checked = checked;
    self
  }

  pub fn on_click(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
    self.on_click = Some(Rc::new(handler));
    self
  }
}

impl Disableable for Checkbox {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Selectable for Checkbox {
  fn selected(self, selected: bool) -> Self {
    self.checked(selected)
  }

  fn is_selected(&self) -> bool {
    self.checked
  }
}

impl Sizable for Checkbox {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Styled for Checkbox {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Checkbox {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Checkbox {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let checked = self.checked;
    let indicator_color = if checked {
      cx.theme().primary
    } else {
      cx.theme().background
    };
    let border_color = if checked {
      cx.theme().primary
    } else {
      cx.theme().input
    };

    h_flex()
      .id(self.id)
      .items_center()
      .gap_2()
      .text_color(if self.disabled {
        cx.theme().muted_foreground
      } else {
        cx.theme().foreground
      })
      .child(
        div()
          .flex_none()
          .size(self.size.component_height() * 0.5)
          .rounded(self.size.component_radius())
          .border_1()
          .border_color(border_color)
          .bg(indicator_color)
          .child(
            div().size_full().items_center().justify_center().child(
              Icon::new(IconName::Checkmark)
                .with_size(self.size.smaller())
                .text_color(cx.theme().primary_foreground)
                .when(!checked, |this| this.opacity(0.0)),
            ),
          ),
      )
      .when_some(self.label, |this, label| this.child(label))
      .children(self.children)
      .when(!self.disabled, |this| {
        this
          .cursor_pointer()
          .hover(|this| this.opacity(0.9))
          .active(|this| this.opacity(0.8))
      })
      .when_some(
        self.on_click.filter(|_| !self.disabled),
        |this, on_click| {
          this.on_click(move |_event: &ClickEvent, window, cx| on_click(&!checked, window, cx))
        },
      )
      .refine_style(&self.style)
  }
}
