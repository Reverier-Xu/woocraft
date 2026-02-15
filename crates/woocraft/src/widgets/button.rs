use std::rc::Rc;
use std::time::Duration;

use gpui::{
  Animation, AnimationExt as _, AnyElement, AnyView, App, ClickEvent, ElementId, Hsla,
  InteractiveElement as _, IntoElement, ParentElement, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Transformation, Window, div, linear,
  percentage, prelude::FluentBuilder,
};

use crate::{
  ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable, Size, StyleSized, StyledExt,
  h_flex,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonVariant {
  Primary,
  Success,
  Warning,
  #[default]
  Default,
  Flat,
  Danger,
}

pub trait ButtonVariants: Sized {
  fn with_variant(self, variant: ButtonVariant) -> Self;

  fn primary(self) -> Self {
    self.with_variant(ButtonVariant::Primary)
  }

  fn success(self) -> Self {
    self.with_variant(ButtonVariant::Success)
  }

  fn warning(self) -> Self {
    self.with_variant(ButtonVariant::Warning)
  }

  fn default(self) -> Self {
    self.with_variant(ButtonVariant::Default)
  }

  fn flat(self) -> Self {
    self.with_variant(ButtonVariant::Flat)
  }

  fn ghost(self) -> Self {
    self.default()
  }

  fn danger(self) -> Self {
    self.with_variant(ButtonVariant::Danger)
  }
}

#[derive(IntoElement)]
pub struct Button {
  id: ElementId,
  label: Option<SharedString>,
  icon: Option<IconName>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
  variant: ButtonVariant,
  size: Size,
  disabled: bool,
  selected: bool,
  outline: bool,
  loading: bool,
  loading_icon: Option<IconName>,
  on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
  tooltip_builder: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyView>>,
}

impl Button {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      label: None,
      icon: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
      variant: ButtonVariant::default(),
      size: Size::Medium,
      disabled: false,
      selected: false,
      outline: false,
      loading: false,
      loading_icon: None,
      on_click: None,
      tooltip_builder: None,
    }
  }

  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  pub fn icon(mut self, icon: IconName) -> Self {
    self.icon = Some(icon);
    self
  }

  pub fn on_click(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Rc::new(handler));
    self
  }

  pub fn outline(mut self, outline: bool) -> Self {
    self.outline = outline;
    self
  }

  pub fn loading(mut self, loading: bool) -> Self {
    self.loading = loading;
    self
  }

  pub fn loading_icon(mut self, icon: IconName) -> Self {
    self.loading_icon = Some(icon);
    self
  }

  pub fn tooltip(mut self, builder: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
    self.tooltip_builder = Some(Rc::new(builder));
    self
  }

  fn clickable(&self) -> bool {
    !self.disabled && !self.loading && self.on_click.is_some()
  }

  fn hoverable(&self) -> bool {
    !self.disabled && !self.loading
  }
}

impl ButtonVariants for Button {
  fn with_variant(mut self, variant: ButtonVariant) -> Self {
    self.variant = variant;
    self
  }
}

impl Disableable for Button {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Selectable for Button {
  fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  fn is_selected(&self) -> bool {
    self.selected
  }
}

impl Sizable for Button {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Styled for Button {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Button {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Button {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let is_flat = matches!(self.variant, ButtonVariant::Flat);

    let transparent = Hsla::transparent_black();
    let (base_bg, base_fg, base_border, hover_bg, active_bg) = match self.variant {
      ButtonVariant::Primary => (
        theme.primary,
        theme.primary_foreground,
        theme.primary,
        Hsla {
          a: 0.9,
          ..theme.primary
        },
        Hsla {
          a: 0.8,
          ..theme.primary
        },
      ),
      ButtonVariant::Success => (
        theme.success,
        theme.primary_foreground,
        theme.success,
        Hsla {
          a: 0.9,
          ..theme.success
        },
        Hsla {
          a: 0.8,
          ..theme.success
        },
      ),
      ButtonVariant::Warning => (
        theme.warning,
        theme.primary_foreground,
        theme.warning,
        Hsla {
          a: 0.9,
          ..theme.warning
        },
        Hsla {
          a: 0.8,
          ..theme.warning
        },
      ),
      ButtonVariant::Default => (
        transparent,
        theme.foreground,
        theme.border,
        Hsla {
          a: 0.05,
          ..theme.foreground
        },
        Hsla {
          a: 0.1,
          ..theme.foreground
        },
      ),
      ButtonVariant::Flat => (
        transparent,
        theme.foreground,
        transparent,
        Hsla {
          a: 0.05,
          ..theme.foreground
        },
        Hsla {
          a: 0.1,
          ..theme.foreground
        },
      ),
      ButtonVariant::Danger => (
        theme.danger,
        theme.primary_foreground,
        theme.danger,
        Hsla {
          a: 0.9,
          ..theme.danger
        },
        Hsla {
          a: 0.8,
          ..theme.danger
        },
      ),
    };

    let (bg, fg, border) = if self.outline {
      if self.variant == ButtonVariant::Default {
        (transparent, theme.foreground, theme.foreground)
      } else {
        (transparent, base_bg, base_bg)
      }
    } else {
      (base_bg, base_fg, base_border)
    };

    let background_hover = Hsla {
      l: (theme.background.l - 0.04).clamp(0.0, 1.0),
      ..theme.background
    };
    let background_active = Hsla {
      l: (theme.background.l - 0.08).clamp(0.0, 1.0),
      ..theme.background
    };

    let hover_bg = if self.outline { background_hover } else { hover_bg };
    let active_bg = if self.outline { background_active } else { active_bg };

    let selected_bg = if is_flat {
      bg
    } else if self.outline {
      transparent
    } else {
      active_bg
    };
    let selected_fg = if is_flat {
      theme.primary
    } else if self.outline {
      base_bg
    } else {
      fg
    };
    let selected_border = if is_flat {
      border
    } else if self.outline {
      base_bg
    } else {
      border
    };

    let disabled_alpha = 0.6;
    let (bg, fg, border, hover_bg, active_bg, selected_bg, selected_fg, selected_border) =
      if self.disabled {
        (
          Hsla {
            a: disabled_alpha,
            ..bg
          },
          Hsla {
            a: disabled_alpha,
            ..fg
          },
          Hsla {
            a: disabled_alpha,
            ..border
          },
          Hsla {
            a: disabled_alpha,
            ..hover_bg
          },
          Hsla {
            a: disabled_alpha,
            ..active_bg
          },
          Hsla {
            a: disabled_alpha,
            ..selected_bg
          },
          Hsla {
            a: disabled_alpha,
            ..selected_fg
          },
          Hsla {
            a: disabled_alpha,
            ..selected_border
          },
        )
      } else {
        (
          bg,
          fg,
          border,
          hover_bg,
          active_bg,
          selected_bg,
          selected_fg,
          selected_border,
        )
      };

    let has_only_icon = self.label.is_none() && self.children.is_empty() && self.icon.is_some();
    let clickable = self.clickable();
    let hoverable = self.hoverable();
    let icon = if self.loading {
      self.loading_icon.or(Some(IconName::SpinnerIos))
    } else {
      self.icon
    };

    let content = h_flex()
      .items_center()
      .justify_center()
      .gap_2()
      .when_some(icon, |this, icon| {
        let icon = Icon::new(icon).with_size(self.size);
        if self.loading {
          this.child(
            icon.with_animation(
              "loading-spin",
              Animation::new(Duration::from_secs_f64(0.8))
                .repeat()
                .with_easing(linear),
              |this, delta| this.transform(Transformation::rotate(percentage(delta))),
            ),
          )
        } else {
          this.child(icon)
        }
      })
      .when_some(self.label, |this, label| this.child(label))
      .children(self.children)
      .button_text_size(self.size);

    div()
      .id(self.id)
      .h_flex()
      .justify_center()
      .items_center()
      .when(!is_flat, |this| this.border_1())
      .bg(bg)
      .border_color(border)
      .text_color(fg)
      .input_size(self.size)
      .when(self.selected, |this| {
        this
          .bg(selected_bg)
          .text_color(selected_fg)
          .border_color(selected_border)
      })
      .when(!has_only_icon, |this| this.px(self.size.input_px()))
      .when(has_only_icon, |this| {
        this.size(self.size.component_height()).p_0()
      })
      .child(content)
      .when(hoverable, |this| {
        this
          .cursor_pointer()
          .hover(move |this| this.bg(hover_bg).border_color(border))
          .active(move |this| this.bg(active_bg).border_color(border))
      })
      .when(self.disabled, |this| this.cursor_not_allowed())
      .when_some(self.on_click.filter(|_| clickable), |this, on_click| {
        this.on_click(move |event, window, cx| on_click(event, window, cx))
      })
      .when_some(self.tooltip_builder, |this, tooltip_builder| {
        this.tooltip(move |window, cx| tooltip_builder(window, cx))
      })
      .refine_style(&self.style)
  }
}

impl From<Button> for AnyElement {
  fn from(value: Button) -> Self {
    value.into_any_element()
  }
}
