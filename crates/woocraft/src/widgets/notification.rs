use std::{collections::VecDeque, rc::Rc, time::Duration};

use gpui::{
  App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Render, RenderOnce,
  SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _, px,
};

use crate::{ActiveTheme, Button, ButtonVariants, Icon, IconName, StyledExt, h_flex, v_flex};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NotificationPlacement {
  TopLeft,
  #[default]
  TopRight,
  BottomLeft,
  BottomRight,
  TopCenter,
  BottomCenter,
}

impl NotificationPlacement {
  fn is_bottom(self) -> bool {
    matches!(
      self,
      Self::BottomLeft | Self::BottomRight | Self::BottomCenter
    )
  }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum NotificationType {
  #[default]
  Info,
  Success,
  Warning,
  Error,
}

impl NotificationType {
  fn icon(&self) -> IconName {
    match self {
      Self::Info => IconName::Alert,
      Self::Success => IconName::CheckmarkCircle,
      Self::Warning => IconName::AlertUrgent,
      Self::Error => IconName::DismissCircle,
    }
  }

  fn color(&self, cx: &App) -> gpui::Hsla {
    match self {
      Self::Info => cx.theme().primary,
      Self::Success => cx.theme().success,
      Self::Warning => cx.theme().warning,
      Self::Error => cx.theme().danger,
    }
  }
}

#[derive(Clone)]
pub struct Notification {
  key: Option<SharedString>,
  type_: NotificationType,
  title: Option<SharedString>,
  message: Option<SharedString>,
  icon: Option<IconName>,
  autohide: bool,
  duration: Duration,
  on_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
  action_label: Option<SharedString>,
  action_on_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl Notification {
  pub fn new() -> Self {
    Self {
      key: None,
      type_: NotificationType::Info,
      title: None,
      message: None,
      icon: None,
      autohide: true,
      duration: Duration::from_secs(5),
      on_click: None,
      action_label: None,
      action_on_click: None,
    }
  }

  pub fn key(mut self, key: impl Into<SharedString>) -> Self {
    self.key = Some(key.into());
    self
  }

  pub fn with_type(mut self, type_: NotificationType) -> Self {
    self.type_ = type_;
    self
  }

  pub fn info(message: impl Into<SharedString>) -> Self {
    Self::new()
      .with_type(NotificationType::Info)
      .message(message)
  }

  pub fn success(message: impl Into<SharedString>) -> Self {
    Self::new()
      .with_type(NotificationType::Success)
      .message(message)
  }

  pub fn warning(message: impl Into<SharedString>) -> Self {
    Self::new()
      .with_type(NotificationType::Warning)
      .message(message)
  }

  pub fn error(message: impl Into<SharedString>) -> Self {
    Self::new()
      .with_type(NotificationType::Error)
      .message(message)
  }

  pub fn title(mut self, title: impl Into<SharedString>) -> Self {
    self.title = Some(title.into());
    self
  }

  pub fn message(mut self, message: impl Into<SharedString>) -> Self {
    self.message = Some(message.into());
    self
  }

  pub fn autohide(mut self, autohide: bool) -> Self {
    self.autohide = autohide;
    self
  }

  pub fn icon(mut self, icon: IconName) -> Self {
    self.icon = Some(icon);
    self
  }

  pub fn duration(mut self, duration: Duration) -> Self {
    self.duration = duration;
    self
  }

  pub fn on_click(mut self, on_click: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_click = Some(Rc::new(on_click));
    self
  }

  pub fn action(
    mut self, label: impl Into<SharedString>, on_click: impl Fn(&mut Window, &mut App) + 'static,
  ) -> Self {
    self.action_label = Some(label.into());
    self.action_on_click = Some(Rc::new(on_click));
    self.autohide = false;
    self
  }
}

impl From<&str> for Notification {
  fn from(value: &str) -> Self {
    Self::new().message(value.to_string())
  }
}

impl From<String> for Notification {
  fn from(value: String) -> Self {
    Self::new().message(value)
  }
}

impl From<SharedString> for Notification {
  fn from(value: SharedString) -> Self {
    Self::new().message(value)
  }
}

#[derive(Clone)]
struct NotificationItem {
  id: usize,
  key: Option<SharedString>,
  data: Notification,
}

pub struct NotificationState {
  items: VecDeque<NotificationItem>,
  max_items: usize,
  next_id: usize,
}

impl NotificationState {
  pub fn new() -> Self {
    Self {
      items: VecDeque::new(),
      max_items: 10,
      next_id: 1,
    }
  }

  pub fn max_items(mut self, max_items: usize) -> Self {
    self.max_items = max_items.max(1);
    self
  }

  pub fn push(
    &mut self, notification: impl Into<Notification>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    let notification = notification.into();

    if let Some(key) = notification.key.as_ref() {
      self.items.retain(|item| item.key.as_ref() != Some(key));
    }

    let id = self.next_id;
    self.next_id += 1;

    let autohide = notification.autohide;
    let duration = notification.duration;

    self.items.push_back(NotificationItem {
      id,
      key: notification.key.clone(),
      data: notification,
    });

    while self.items.len() > self.max_items {
      self.items.pop_front();
    }

    if autohide {
      let state = cx.entity().downgrade();
      cx.spawn_in(window, async move |_, cx| {
        cx.background_executor().timer(duration).await;
        if let Some(state) = state.upgrade() {
          _ = state.update(cx, |state, cx| {
            state.close(id);
            cx.notify();
          });
        }
      })
      .detach();
    }

    cx.notify();
  }

  pub fn close(&mut self, id: usize) {
    self.items.retain(|item| item.id != id);
  }

  pub fn clear(&mut self) {
    self.items.clear();
  }
}

#[derive(IntoElement)]
pub struct NotificationCenter {
  state: Entity<NotificationState>,
  style: StyleRefinement,
  placement: NotificationPlacement,
  margin_top: gpui::Pixels,
  margin_right: gpui::Pixels,
  margin_bottom: gpui::Pixels,
  margin_left: gpui::Pixels,
  width: gpui::Pixels,
}

impl NotificationCenter {
  pub fn new(state: &Entity<NotificationState>) -> Self {
    Self {
      state: state.clone(),
      style: StyleRefinement::default(),
      placement: NotificationPlacement::TopRight,
      margin_top: px(16.0),
      margin_right: px(16.0),
      margin_bottom: px(16.0),
      margin_left: px(16.0),
      width: px(360.0),
    }
  }

  pub fn placement(mut self, placement: NotificationPlacement) -> Self {
    self.placement = placement;
    self
  }

  pub fn margins(
    mut self, top: gpui::Pixels, right: gpui::Pixels, bottom: gpui::Pixels, left: gpui::Pixels,
  ) -> Self {
    self.margin_top = top;
    self.margin_right = right;
    self.margin_bottom = bottom;
    self.margin_left = left;
    self
  }

  pub fn width(mut self, width: gpui::Pixels) -> Self {
    self.width = width;
    self
  }
}

impl Styled for NotificationCenter {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for NotificationCenter {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let items = self
      .state
      .read(cx)
      .items
      .iter()
      .cloned()
      .collect::<Vec<_>>();
    let placement = self.placement;

    v_flex()
      .id("notification-center")
      .absolute()
      .when(
        matches!(placement, NotificationPlacement::TopLeft),
        |this| this.top(self.margin_top).left(self.margin_left),
      )
      .when(
        matches!(placement, NotificationPlacement::TopRight),
        |this| this.top(self.margin_top).right(self.margin_right),
      )
      .when(
        matches!(placement, NotificationPlacement::BottomLeft),
        |this| this.bottom(self.margin_bottom).left(self.margin_left),
      )
      .when(
        matches!(placement, NotificationPlacement::BottomRight),
        |this| this.bottom(self.margin_bottom).right(self.margin_right),
      )
      .when(
        matches!(placement, NotificationPlacement::TopCenter),
        |this| this.top(self.margin_top).left_1_2().ml(-(self.width / 2.0)),
      )
      .when(
        matches!(placement, NotificationPlacement::BottomCenter),
        |this| {
          this
            .bottom(self.margin_bottom)
            .left_1_2()
            .ml(-(self.width / 2.0))
        },
      )
      .w(self.width)
      .max_h(px(560.0))
      .gap_2()
      .when(placement.is_bottom(), |this| this.flex_col_reverse())
      .children(
        items
          .into_iter()
          .map(|item| NotificationCard::new(item.id, item.data, &self.state).into_any_element()),
      )
      .refine_style(&self.style)
  }
}

#[derive(IntoElement)]
struct NotificationCard {
  id: usize,
  data: Notification,
  state: Entity<NotificationState>,
}

impl NotificationCard {
  fn new(id: usize, data: Notification, state: &Entity<NotificationState>) -> Self {
    Self {
      id,
      data,
      state: state.clone(),
    }
  }
}

impl RenderOnce for NotificationCard {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let icon_color = self.data.type_.color(cx);
    let icon_name = self.data.icon.unwrap_or_else(|| self.data.type_.icon());

    v_flex()
      .id(("notification-card", self.id as u64))
      .w_full()
      .border_1()
      .border_color(cx.theme().border)
      .bg(cx.theme().popover)
      .rounded(cx.theme().radius_container)
      .p_1()
      .gap_1()
      .items_start()
      .child(
        h_flex()
          .w_full()
          .items_center()
          .bg(icon_color.opacity(0.2))
          .pl_2()
          .gap_2()
          .rounded(cx.theme().radius)
          .child(Icon::new(icon_name).text_color(icon_color))
          .child(
            v_flex()
              .flex_1()
              .when_some(self.data.title.clone(), |this, title| {
                this.child(div().font_semibold().child(title))
              }),
          )
          .child(
            Button::new(("notification-close", self.id as u64))
              .flat()
              .icon(IconName::Dismiss)
              .on_click({
                let state = self.state.clone();
                let id = self.id;
                move |_, _, cx| {
                  state.update(cx, |state, cx| {
                    state.close(id);
                    cx.notify();
                  });
                }
              }),
          ),
      )
      .child(v_flex().w_full().px_2().flex_1().when_some(
        self.data.message.clone(),
        |this, message| {
          this.child(
            div()
              .text_color(cx.theme().muted_foreground)
              .child(message),
          )
        },
      ))
      .child(h_flex().w_full().justify_end().when_some(
        self.data.action_label.clone(),
        |this, action_label| {
          this.child(
            Button::new(("notification-action", self.id as u64))
              .flat()
              .label(action_label)
              .on_click({
                let action = self.data.action_on_click.clone();
                move |_, window, cx| {
                  if let Some(action) = action.as_ref() {
                    action(window, cx);
                  }
                }
              }),
          )
        },
      ))
      .when_some(self.data.on_click.clone(), |this, on_click| {
        this.cursor_pointer().on_click(move |_, window, cx| {
          on_click(window, cx);
        })
      })
  }
}

impl Render for NotificationState {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
  }
}
