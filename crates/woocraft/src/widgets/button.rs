use std::{rc::Rc, time::Duration};

use gpui::{
  Animation, AnimationExt as _, AnyElement, AnyView, App, ClickEvent, Corners, ElementId, Hsla,
  InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Transformation, Window, div, linear,
  percentage, prelude::FluentBuilder, px,
};

use crate::{
  ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable, Size, StyleSized, StyledExt,
  h_flex,
};

type ButtonClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type TooltipBuilder = Rc<dyn Fn(&mut Window, &mut App) -> AnyView>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonVariant {
  Primary,
  Success,
  Warning,
  Info,
  #[default]
  Default,
  Link,
  Flat,
  Danger,
}

#[derive(Default, Clone, Copy, Debug)]
pub enum ButtonRounded {
  None,
  Small,
  #[default]
  Medium,
  Large,
  Size(Pixels),
}

impl From<Pixels> for ButtonRounded {
  fn from(value: Pixels) -> Self {
    Self::Size(value)
  }
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

  fn info(self) -> Self {
    self.with_variant(ButtonVariant::Info)
  }

  fn default(self) -> Self {
    self.with_variant(ButtonVariant::Default)
  }

  fn flat(self) -> Self {
    self.with_variant(ButtonVariant::Flat)
  }

  fn link(self) -> Self {
    self.with_variant(ButtonVariant::Link)
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
  rounded: ButtonRounded,
  border_corners: Corners<bool>,
  dropdown_caret: bool,
  tab_stop: bool,
  tab_index: isize,
  outline: bool,
  loading: bool,
  loading_icon: Option<IconName>,
  on_click: Option<ButtonClickHandler>,
  tooltip_builder: Option<TooltipBuilder>,
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
      rounded: ButtonRounded::default(),
      border_corners: Corners::all(true),
      dropdown_caret: false,
      tab_stop: true,
      tab_index: 0,
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

  pub fn rounded(mut self, rounded: impl Into<ButtonRounded>) -> Self {
    self.rounded = rounded.into();
    self
  }

  pub fn border_corners(mut self, corners: impl Into<Corners<bool>>) -> Self {
    self.border_corners = corners.into();
    self
  }

  pub fn dropdown_caret(mut self, dropdown_caret: bool) -> Self {
    self.dropdown_caret = dropdown_caret;
    self
  }

  pub fn tab_stop(mut self, tab_stop: bool) -> Self {
    self.tab_stop = tab_stop;
    self
  }

  pub fn tab_index(mut self, tab_index: isize) -> Self {
    self.tab_index = tab_index;
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

  pub fn element_id(&self) -> ElementId {
    self.id.clone()
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
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let is_flat = matches!(self.variant, ButtonVariant::Flat | ButtonVariant::Link);

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
      ButtonVariant::Info => (
        theme.ring,
        theme.primary_foreground,
        theme.ring,
        Hsla {
          a: 0.9,
          ..theme.ring
        },
        Hsla {
          a: 0.8,
          ..theme.ring
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
      ButtonVariant::Link => (
        transparent,
        theme.primary,
        transparent,
        Hsla {
          a: 0.1,
          ..theme.primary
        },
        Hsla {
          a: 0.15,
          ..theme.primary
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

    let hover_bg = if self.outline {
      background_hover
    } else {
      hover_bg
    };
    let active_bg = if self.outline {
      background_active
    } else {
      active_bg
    };

    let selected_bg = if is_flat {
      bg
    } else if self.outline {
      background_hover
    } else {
      active_bg
    };
    let selected_fg = if is_flat {
      theme.primary
    } else if self.outline {
      if self.variant == ButtonVariant::Default {
        theme.foreground
      } else {
        base_bg
      }
    } else {
      fg
    };
    let selected_border = if is_flat {
      border
    } else if self.outline {
      if self.variant == ButtonVariant::Default {
        theme.foreground
      } else {
        base_bg
      }
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
      let (bg, border) = if self.disabled && matches!(self.variant, ButtonVariant::Flat | ButtonVariant::Link | ButtonVariant::Default) {
        (theme.foreground.alpha(0.2), transparent)
      } else {
        (bg, border)
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
      .when(self.dropdown_caret, |this| {
        this.child(Icon::new(IconName::ChevronDown).with_size(self.size.smaller()))
      })
      .button_text_size(self.size);

    let radius = match self.rounded {
      ButtonRounded::None => px(0.0),
      ButtonRounded::Small => theme.radius / 2.0,
      ButtonRounded::Medium => theme.radius,
      ButtonRounded::Large => theme.radius_container,
      ButtonRounded::Size(radius) => radius,
    };

    let focus_handle = window
      .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone();

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
      .rounded_tl(if self.border_corners.top_left {
        radius
      } else {
        px(0.0)
      })
      .rounded_tr(if self.border_corners.top_right {
        radius
      } else {
        px(0.0)
      })
      .rounded_bl(if self.border_corners.bottom_left {
        radius
      } else {
        px(0.0)
      })
      .rounded_br(if self.border_corners.bottom_right {
        radius
      } else {
        px(0.0)
      })
      .when(self.selected, |this| {
        this
          .bg(selected_bg)
          .text_color(selected_fg)
          .border_color(selected_border)
      })
      .when(!has_only_icon, |this| this.px(self.size.input_px()))
      .when(has_only_icon, |this| {
        this
          .size(self.size.component_height())
          .p_0()
          .flex_shrink_0()
      })
      .when(!self.disabled, |this| {
        this.track_focus(
          &focus_handle
            .tab_stop(self.tab_stop)
            .tab_index(self.tab_index),
        )
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
