use std::rc::Rc;

use gpui::{
  AnimationExt as _, AnyElement, AnyView, App, ClickEvent, Corners, ElementId, Hsla,
  InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Transformation, Window, percentage,
  prelude::FluentBuilder, px,
};

use crate::{
  ActiveTheme, ColorExt, Disableable, Icon, IconName, InteractionColors, Selectable, Sizable, Size,
  StyleSized, StyledExt, h_flex, opacity, spinner_animation,
};

type ButtonClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ButtonHoverHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
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
  expanded: bool,
  tab_stop: bool,
  tab_index: isize,
  outline: bool,
  loading: bool,
  loading_icon: Option<IconName>,
  on_click: Option<ButtonClickHandler>,
  on_hover: Option<ButtonHoverHandler>,
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
      expanded: false,
      tab_stop: true,
      tab_index: 0,
      outline: false,
      loading: false,
      loading_icon: None,
      on_click: None,
      on_hover: None,
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

  pub fn on_hover(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
    self.on_hover = Some(Rc::new(handler));
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

  pub fn expand(mut self, expanded: bool) -> Self {
    self.expanded = expanded;
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

    let interaction_colors = match self.variant {
      ButtonVariant::Primary => InteractionColors::solid(theme.primary, theme.primary_foreground),
      ButtonVariant::Success => InteractionColors::solid(theme.success, theme.primary_foreground),
      ButtonVariant::Warning => InteractionColors::solid(theme.warning, theme.primary_foreground),
      ButtonVariant::Info => InteractionColors::solid(theme.ring, theme.primary_foreground),
      ButtonVariant::Danger => InteractionColors::solid(theme.danger, theme.primary_foreground),
      ButtonVariant::Default => {
        InteractionColors::transparent(theme.foreground).with_border(theme.border)
      }
      ButtonVariant::Link => InteractionColors::transparent(theme.primary),
      ButtonVariant::Flat => InteractionColors::transparent(theme.foreground),
    };

    let transparent = Hsla::transparent_black();
    let (bg, fg, border) = if self.outline {
      if self.variant == ButtonVariant::Default {
        (transparent, theme.foreground, theme.foreground)
      } else {
        (
          transparent,
          interaction_colors.base,
          interaction_colors.base,
        )
      }
    } else {
      (
        interaction_colors.base,
        interaction_colors.foreground,
        interaction_colors.border,
      )
    };

    let background_hover = theme.background.darken(0.04);
    let background_active = theme.background.darken(0.08);

    let hover_bg = if self.outline {
      background_hover
    } else {
      interaction_colors.hover
    };
    let active_bg = if self.outline {
      background_active
    } else {
      interaction_colors.active
    };

    let selected_bg = if is_flat {
      theme.foreground.opacity(opacity::transparent::ACTIVE)
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
        interaction_colors.base
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
        interaction_colors.base
      }
    } else {
      border
    };

    let (bg, fg, border, hover_bg, active_bg, selected_bg, selected_fg, selected_border) =
      if self.disabled {
        (
          bg.opacity(opacity::DISABLED),
          fg.opacity(opacity::DISABLED),
          border.opacity(opacity::DISABLED),
          hover_bg.opacity(opacity::DISABLED),
          active_bg.opacity(opacity::DISABLED),
          selected_bg.opacity(opacity::DISABLED),
          selected_fg.opacity(opacity::DISABLED),
          selected_border.opacity(opacity::DISABLED),
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

    let (bg, border) =
      if self.disabled && matches!(self.variant, ButtonVariant::Link | ButtonVariant::Default) {
        (theme.foreground.opacity(0.1), transparent)
      } else {
        (bg, border)
      };

    let (bg, border, hover_bg, active_bg, selected_bg) =
      if self.disabled && self.variant == ButtonVariant::Flat {
        (
          transparent,
          transparent,
          theme.foreground.opacity(0.05),
          theme.foreground.opacity(0.05),
          theme.foreground.opacity(0.05),
        )
      } else {
        (bg, border, hover_bg, active_bg, selected_bg)
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
      .component_gap(self.size)
      .when(self.expanded, |this| this.flex_1().w_full().justify_start())
      .when_some(icon, |this, icon| {
        let icon = Icon::new(icon).with_size(self.size);
        if self.loading {
          this.child(icon.with_animation(
            "loading-spin",
            spinner_animation(),
            |this: Icon, delta| this.transform(Transformation::rotate(percentage(delta))),
          ))
        } else {
          this.child(icon)
        }
      })
      .when_some(self.label, |this, label| this.child(label))
      .children(self.children)
      .text_size(self.size.text_size());

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

    h_flex()
      .id(self.id)
      .justify_center()
      .when(!is_flat, |this| this.border_1())
      .bg(bg)
      .border_color(border)
      .text_color(fg)
      .component_size(self.size)
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
      .when(!has_only_icon, |this| {
        this
          .px(self.size.component_px())
          .min_w(self.size.component_height())
      })
      .when(has_only_icon, |this| {
        this
          .size(self.size.component_height())
          .p_0()
          .flex_shrink_0()
      })
      .when(self.expanded, |this| this.w_full())
      .when(!self.disabled, |this| {
        this.track_focus(
          &focus_handle
            .tab_stop(self.tab_stop)
            .tab_index(self.tab_index),
        )
      })
      .child(content)
      .when(hoverable && !self.disabled, |this| {
        this
          .cursor_pointer()
          .hover(move |this| this.bg(hover_bg).border_color(border))
          .active(move |this| this.bg(active_bg).border_color(border))
      })
      .when(self.disabled, |this| this.cursor_not_allowed())
      .when_some(self.on_hover, |this, on_hover| {
        this.on_hover(move |hovered, window, cx| on_hover(hovered, window, cx))
      })
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
