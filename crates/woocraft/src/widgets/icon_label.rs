use std::{rc::Rc, time::Duration};

use gpui::{
  Animation, AnimationExt as _, AnyElement, App, ClickEvent, ElementId, Hsla,
  InteractiveElement as _, IntoElement, ParentElement, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Transformation, Window, div, linear,
  percentage, prelude::FluentBuilder as _,
};

use crate::{
  ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable, Size, StyleSized, StyledExt,
  h_flex,
};

type IconLabelClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct IconLabel {
  id: ElementId,
  label: Option<SharedString>,
  icon: Option<IconName>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
  size: Size,
  disabled: bool,
  selected: bool,
  expanded: bool,
  label_expanded: bool,
  loading: bool,
  loading_icon: Option<IconName>,
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
      size: Size::Medium,
      disabled: false,
      selected: false,
      expanded: false,
      label_expanded: false,
      loading: false,
      loading_icon: None,
      on_click: None,
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

  pub fn loading(mut self, loading: bool) -> Self {
    self.loading = loading;
    self
  }

  pub fn loading_icon(mut self, icon: IconName) -> Self {
    self.loading_icon = Some(icon);
    self
  }

  /// Expand IconLabel as a flex item and let its internal row stretch to full width.
  pub fn flex_1(mut self) -> Self {
    self.expanded = true;
    self
  }

  /// Make the label text container flexible so trailing children can align to the far edge.
  pub fn label_flex_1(mut self) -> Self {
    self.label_expanded = true;
    self
  }

  fn clickable(&self) -> bool {
    !self.disabled && !self.loading && self.on_click.is_some()
  }
}

impl Disableable for IconLabel {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Selectable for IconLabel {
  fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  fn is_selected(&self) -> bool {
    self.selected
  }
}

impl Sizable for IconLabel {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
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
    let clickable = self.clickable();
    let expanded = self.expanded;
    let label_expanded = self.label_expanded;
    let has_children = !self.children.is_empty();
    let icon = if self.loading {
      self.loading_icon.or(Some(IconName::SpinnerIos))
    } else {
      self.icon
    };

    let mut text_color = if self.selected {
      theme.primary
    } else {
      theme.foreground
    };

    if self.disabled {
      text_color = Hsla {
        a: 0.6,
        ..text_color
      };
    }

    let content = h_flex()
      .items_center()
      .component_gap(self.size)
      .when(expanded, |this| this.w_full().flex_1())
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
      .when_some(self.label, |this, label| {
        if label_expanded {
          this.child(div().flex_1().child(label))
        } else {
          this.child(label)
        }
      })
      .children(self.children)
      .text_size(self.size.text_size());

    div()
      .id(self.id)
      .h_flex()
      .pl(self.size.component_px())
      .pr(if has_children {
        self.size.container_px()
      } else {
        self.size.component_px()
      })
      .items_center()
      .when(expanded, |this| this.w_full().flex_1())
      .text_color(text_color)
      .when(clickable, |this| this.cursor_pointer())
      .child(content)
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
