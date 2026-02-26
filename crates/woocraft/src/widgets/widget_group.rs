use std::{cell::Cell, rc::Rc};

use gpui::{
  AnyElement, App, Axis, Corners, ElementId, Hsla, InteractiveElement as _, IntoElement,
  ParentElement as _, RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
  div, prelude::FluentBuilder as _, px,
};

use crate::{
  ActiveTheme, Button, ButtonVariant, ButtonVariants as _, Disableable, Icon, IconLabel, Input,
  Label, Selectable, Sizable, Size, StyleSized as _, StyledExt, h_flex,
};

type WidgetGroupClickHandler = Box<dyn Fn(&Vec<usize>, &mut Window, &mut App) + 'static>;

pub enum WidgetGroupChild {
  Button(Button),
  Input(Input),
  Element(AnyElement),
}

impl WidgetGroupChild {
  fn is_selected(&self) -> bool {
    match self {
      Self::Button(button) => button.is_selected(),
      Self::Input(input) => input.is_selected(),
      Self::Element(_) => false,
    }
  }
}

impl From<Button> for WidgetGroupChild {
  fn from(value: Button) -> Self {
    Self::Button(value)
  }
}

impl From<Input> for WidgetGroupChild {
  fn from(value: Input) -> Self {
    Self::Input(value)
  }
}

impl From<AnyElement> for WidgetGroupChild {
  fn from(value: AnyElement) -> Self {
    Self::Element(value)
  }
}

impl From<Icon> for WidgetGroupChild {
  fn from(value: Icon) -> Self {
    Self::Element(value.into_any_element())
  }
}

impl From<IconLabel> for WidgetGroupChild {
  fn from(value: IconLabel) -> Self {
    Self::Element(value.into_any_element())
  }
}

impl From<Label> for WidgetGroupChild {
  fn from(value: Label) -> Self {
    Self::Element(value.into_any_element())
  }
}

#[derive(IntoElement)]
pub struct WidgetGroup {
  id: ElementId,
  style: StyleRefinement,
  children: Vec<WidgetGroupChild>,
  multiple: bool,
  disabled: bool,
  layout: Axis,
  compact: bool,
  outline: bool,
  variant: Option<ButtonVariant>,
  size: Option<Size>,
  on_click: Option<WidgetGroupClickHandler>,
}

impl WidgetGroup {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      children: Vec::new(),
      multiple: false,
      disabled: false,
      layout: Axis::Horizontal,
      compact: false,
      outline: false,
      variant: None,
      size: None,
      on_click: None,
    }
  }

  pub fn child(mut self, child: impl Into<WidgetGroupChild>) -> Self {
    self.children.push(child.into());
    self
  }

  pub fn children(mut self, children: impl IntoIterator<Item = WidgetGroupChild>) -> Self {
    self.children.extend(children);
    self
  }

  pub fn multiple(mut self, multiple: bool) -> Self {
    self.multiple = multiple;
    self
  }

  pub fn layout(mut self, layout: Axis) -> Self {
    self.layout = layout;
    self
  }

  pub fn compact(mut self) -> Self {
    self.compact = true;
    self
  }

  pub fn outline(mut self) -> Self {
    self.outline = true;
    self
  }

  pub fn on_click(
    mut self, handler: impl Fn(&Vec<usize>, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Box::new(handler));
    self
  }

  fn corners_for(ix: usize, len: usize, layout: Axis) -> Corners<bool> {
    let is_first = ix == 0;
    let is_last = ix + 1 == len;
    if len == 1 {
      return Corners::all(true);
    }

    if matches!(layout, Axis::Horizontal) {
      Corners {
        top_left: is_first,
        top_right: is_last,
        bottom_left: is_first,
        bottom_right: is_last,
      }
    } else {
      Corners {
        top_left: is_first,
        top_right: is_first,
        bottom_left: is_last,
        bottom_right: is_last,
      }
    }
  }

  fn corner_pixels(corners: Corners<bool>, radius: gpui::Pixels) -> Corners<gpui::Pixels> {
    Corners {
      top_left: if corners.top_left { radius } else { px(0.) },
      top_right: if corners.top_right { radius } else { px(0.) },
      bottom_left: if corners.bottom_left { radius } else { px(0.) },
      bottom_right: if corners.bottom_right { radius } else { px(0.) },
    }
  }

  fn divider_color(
    left_active: Option<Hsla>, right_active: Option<Hsla>, disabled: bool, cx: &App,
  ) -> Hsla {
    if let Some(color) = right_active {
      return color;
    }

    if let Some(color) = left_active {
      return color;
    }

    let mut base = cx.theme().input;
    if disabled {
      base.a *= 0.6;
    }
    base
  }

  fn button_active_border_color(
    variant: ButtonVariant, outline: bool, disabled: bool, cx: &App,
  ) -> Hsla {
    let theme = cx.theme();
    let transparent = Hsla::transparent_black();

    let base_border = match variant {
      ButtonVariant::Primary => theme.primary,
      ButtonVariant::Success => theme.success,
      ButtonVariant::Warning => theme.warning,
      ButtonVariant::Info => theme.ring,
      ButtonVariant::Default => theme.border,
      ButtonVariant::Link | ButtonVariant::Flat => transparent,
      ButtonVariant::Danger => theme.danger,
    };

    let mut color = if matches!(variant, ButtonVariant::Flat | ButtonVariant::Link) {
      base_border
    } else if outline {
      if variant == ButtonVariant::Default {
        theme.foreground
      } else {
        base_border
      }
    } else {
      base_border
    };

    if disabled {
      color.a *= 0.6;
    }

    color
  }

  fn input_active_border_color(disabled: bool, cx: &App) -> Hsla {
    let mut color = cx.theme().ring;
    if disabled {
      color.a *= 0.6;
    }
    color
  }

  fn active_border_color(
    child: &WidgetGroupChild, variant: ButtonVariant, outline: bool, disabled: bool,
    window: &Window, cx: &App,
  ) -> Option<Hsla> {
    match child {
      WidgetGroupChild::Button(button) if button.is_selected() => Some(
        Self::button_active_border_color(variant, outline, disabled, cx),
      ),
      WidgetGroupChild::Input(input) if input.is_active(window, cx) => {
        Some(Self::input_active_border_color(disabled, cx))
      }
      _ => None,
    }
  }

  fn divider(layout: Axis, color: Hsla, size: Size) -> AnyElement {
    match layout {
      Axis::Horizontal => div()
        .w(px(1.))
        .h(size.component_height())
        .bg(color)
        .ml(px(-1.))
        .into_any_element(),
      Axis::Vertical => div()
        .h(px(1.))
        .w_full()
        .bg(color)
        .mt(px(-1.))
        .into_any_element(),
    }
  }
}

impl_disableable!(WidgetGroup);

impl Sizable for WidgetGroup {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = Some(size.into());
    self
  }
}

impl_styled!(WidgetGroup);

impl crate::ButtonVariants for WidgetGroup {
  fn with_variant(mut self, variant: ButtonVariant) -> Self {
    self.variant = Some(variant);
    self
  }
}

impl RenderOnce for WidgetGroup {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let effective_variant = self.variant.unwrap_or_default();
    let active_border_colors = self
      .children
      .iter()
      .map(|child| {
        Self::active_border_color(
          child,
          effective_variant,
          self.outline,
          self.disabled,
          window,
          cx,
        )
      })
      .collect::<Vec<_>>();
    let selected_ixs = self
      .children
      .iter()
      .enumerate()
      .filter_map(|(ix, child)| child.is_selected().then_some(ix))
      .collect::<Vec<_>>();
    let clicked_ix = Rc::new(Cell::new(None));

    let children_len = self.children.len();
    let effective_size = self.size.unwrap_or_default();

    div()
      .id(self.id)
      .flex()
      .when(matches!(self.layout, Axis::Vertical), |this| {
        this
          .flex_col()
          .items_start()
          .when(!self.compact, |this| this.gap_0())
      })
      .when(matches!(self.layout, Axis::Horizontal), |this| {
        this
          .flex_row()
          .items_center()
          .when(!self.compact, |this| this.gap_0())
      })
      .refine_style(&self.style)
      .children(
        self
          .children
          .into_iter()
          .enumerate()
          .flat_map(|(ix, child)| {
            let clicked_ix = clicked_ix.clone();
            let is_first = ix == 0;
            let is_last = ix + 1 == children_len;
            let hide_leading_border = !is_first;
            let hide_trailing_border = !is_last;
            let corners = Self::corners_for(ix, children_len, self.layout);
            let mut elements = Vec::with_capacity(2);

            if ix > 0 {
              let color = Self::divider_color(
                active_border_colors[ix - 1],
                active_border_colors[ix],
                self.disabled,
                cx,
              );
              elements.push(Self::divider(self.layout, color, effective_size));
            }

            let child = match child {
              WidgetGroupChild::Button(button) => button
                .disabled(self.disabled)
                .border_corners(corners)
                .when_some(self.variant, |this, variant| this.with_variant(variant))
                .when_some(self.size, |this, size| this.with_size(size))
                .when(self.outline, |this| this.outline(true))
                .when(
                  hide_leading_border && matches!(self.layout, Axis::Horizontal),
                  |this| this.ml(px(0.)).border_l_0(),
                )
                .when(
                  hide_leading_border && matches!(self.layout, Axis::Vertical),
                  |this| this.mt(px(0.)).border_t_0(),
                )
                .when(
                  hide_trailing_border && matches!(self.layout, Axis::Horizontal),
                  |this| this.border_r(px(0.)),
                )
                .when(
                  hide_trailing_border && matches!(self.layout, Axis::Vertical),
                  |this| this.border_b(px(0.)),
                )
                .when(self.on_click.is_some() && !self.disabled, |this| {
                  this.on_click(move |_, _, _| {
                    clicked_ix.set(Some(ix));
                  })
                })
                .into_any_element(),
              WidgetGroupChild::Input(input) => input
                .disabled(self.disabled)
                .border_corners(corners)
                .with_size(effective_size)
                .when(
                  hide_leading_border && matches!(self.layout, Axis::Horizontal),
                  |this| this.ml(px(0.)).border_l_0(),
                )
                .when(
                  hide_leading_border && matches!(self.layout, Axis::Vertical),
                  |this| this.mt(px(0.)).border_t_0(),
                )
                .when(
                  hide_trailing_border && matches!(self.layout, Axis::Horizontal),
                  |this| this.border_r(px(0.)),
                )
                .when(
                  hide_trailing_border && matches!(self.layout, Axis::Vertical),
                  |this| this.border_b(px(0.)),
                )
                .into_any_element(),
              WidgetGroupChild::Element(element) => h_flex()
                .items_center()
                .component_h(effective_size)
                .px(effective_size.component_px())
                .bg(if self.disabled {
                  cx.theme().muted
                } else {
                  cx.theme().background
                })
                .border_1()
                .border_color(cx.theme().input)
                .corner_radius(Self::corner_pixels(corners, cx.theme().radius))
                .when(
                  hide_leading_border && matches!(self.layout, Axis::Horizontal),
                  |this| this.ml(px(0.)).border_l_0(),
                )
                .when(
                  hide_leading_border && matches!(self.layout, Axis::Vertical),
                  |this| this.mt(px(0.)).border_t_0(),
                )
                .when(
                  hide_trailing_border && matches!(self.layout, Axis::Horizontal),
                  |this| this.border_r(px(0.)),
                )
                .when(
                  hide_trailing_border && matches!(self.layout, Axis::Vertical),
                  |this| this.border_b(px(0.)),
                )
                .child(element)
                .into_any_element(),
            };

            elements.push(child);
            elements
          }),
      )
      .when_some(
        self.on_click.filter(|_| !self.disabled),
        move |this, on_click| {
          this.on_click(move |_, window, cx| {
            let mut next = selected_ixs.clone();
            if let Some(ix) = clicked_ix.get() {
              if self.multiple {
                if let Some(pos) = next.iter().position(|x| *x == ix) {
                  next.remove(pos);
                } else {
                  next.push(ix);
                }
              } else {
                next.clear();
                next.push(ix);
              }
              next.sort_unstable();
            }
            on_click(&next, window, cx);
          })
        },
      )
  }
}
