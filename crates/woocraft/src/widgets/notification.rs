use std::{
  collections::VecDeque,
  rc::Rc,
  time::{Duration, Instant},
};

use gpuim::{
  App, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Render, RenderOnce,
  SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _, px, relative,
};

use crate::{
  ActiveTheme, Button, ButtonVariants, CardStyle, ColorExt, Icon, IconName, Size, StyleSized,
  StyledExt, duration, h_flex, v_flex,
};

/// Handler type invoked when a notification (or its action) is clicked.
///
/// The handler receives a mutable reference to the current `Window` and `App`.
type NotificationClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Where the notification center will be placed on screen.
///
/// Controls alignment of the notification container relative to the window.
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
/// Semantic type of a notification.
///
/// Determines the icon and color used when rendering the notification.
pub enum NotificationType {
  #[default]
  Info,
  Success,
  Warning,
  Error,
}

impl NotificationType {
  fn icon(&self) -> Icon {
    match self {
      Self::Info => Icon::new(IconName::Alert),
      Self::Success => Icon::new(IconName::CheckmarkCircle),
      Self::Warning => Icon::new(IconName::AlertUrgent),
      Self::Error => Icon::new(IconName::DismissCircle),
    }
  }

  fn color(&self, cx: &App) -> gpuim::Hsla {
    match self {
      Self::Info => cx.theme().primary,
      Self::Success => cx.theme().success,
      Self::Warning => cx.theme().warning,
      Self::Error => cx.theme().danger,
    }
  }
}

/// A single notification description used to create notifications.
///
/// Construct with `Notification::new()` or helper constructors like
/// `Notification::info(...)` / `Notification::success(...)`.
#[derive(Clone)]
pub struct Notification {
  key: Option<SharedString>,
  type_: NotificationType,
  title: Option<SharedString>,
  message: Option<SharedString>,
  icon: Option<Icon>,
  autohide: bool,
  duration: Duration,
  on_click: Option<NotificationClickHandler>,
  action_label: Option<SharedString>,
  action_on_click: Option<NotificationClickHandler>,
}

impl Default for Notification {
  fn default() -> Self {
    Self::new()
  }
}

impl Notification {
  /// Create a new notification with default settings.
  ///
  /// Defaults to `Info` type, autohide enabled and default duration.
  pub fn new() -> Self {
    Self {
      key: None,
      type_: NotificationType::Info,
      title: None,
      message: None,
      icon: None,
      autohide: true,
      duration: duration::NOTIFICATION_DEFAULT,
      on_click: None,
      action_label: None,
      action_on_click: None,
    }
  }

  /// Set a unique key for the notification.
  ///
  /// If a key is provided and another pending notification with the same key
  /// exists, the previous one will be removed before pushing the new one.
  pub fn key(mut self, key: impl Into<SharedString>) -> Self {
    self.key = Some(key.into());
    self
  }

  /// Set the semantic `NotificationType` for this notification.
  pub fn with_type(mut self, type_: NotificationType) -> Self {
    self.type_ = type_;
    self
  }

  /// Create an `Info` notification with a message.
  pub fn info(message: impl Into<SharedString>) -> Self {
    Self::new()
      .with_type(NotificationType::Info)
      .message(message)
  }

  /// Create a `Success` notification with a message.
  pub fn success(message: impl Into<SharedString>) -> Self {
    Self::new()
      .with_type(NotificationType::Success)
      .message(message)
  }

  /// Create a `Warning` notification with a message.
  pub fn warning(message: impl Into<SharedString>) -> Self {
    Self::new()
      .with_type(NotificationType::Warning)
      .message(message)
  }

  /// Create an `Error` notification with a message.
  pub fn error(message: impl Into<SharedString>) -> Self {
    Self::new()
      .with_type(NotificationType::Error)
      .message(message)
  }

  /// Set the title for the notification.
  pub fn title(mut self, title: impl Into<SharedString>) -> Self {
    self.title = Some(title.into());
    self
  }

  /// Set the main message body for the notification.
  pub fn message(mut self, message: impl Into<SharedString>) -> Self {
    self.message = Some(message.into());
    self
  }

  /// Enable or disable automatic hiding for the notification.
  ///
  /// When `false`, the notification will remain until explicitly acted on.
  pub fn autohide(mut self, autohide: bool) -> Self {
    self.autohide = autohide;
    self
  }

  /// Override the default icon for the notification.
  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// Set the autohide duration for the notification.
  pub fn duration(mut self, duration: Duration) -> Self {
    self.duration = duration;
    self
  }

  /// Register a click handler invoked when the notification is clicked.
  pub fn on_click(mut self, on_click: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_click = Some(Rc::new(on_click));
    self
  }

  /// Add an action button to the notification.
  ///
  /// The `label` will be displayed as the action text and `on_click` will be
  /// called when the action is triggered. Adding an action disables autohide
  /// by default so users can interact with the notification.
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
  autohide: bool,
  duration: Duration,
  timer_epoch: u64,
  started_at: Option<Instant>,
  hovered: bool,
}

pub struct NotificationState {
  items: VecDeque<NotificationItem>,
  max_items: usize,
  next_id: usize,
}

impl Default for NotificationState {
  fn default() -> Self {
    Self::new()
  }
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
      autohide,
      duration,
      timer_epoch: 0,
      started_at: None,
      hovered: false,
    });

    while self.items.len() > self.max_items {
      self.items.pop_front();
    }

    if autohide {
      self.restart_timer(id, window, cx);
    }

    cx.notify();
  }

  fn spawn_timer(
    state: gpuim::WeakEntity<Self>, id: usize, duration: Duration, epoch: u64, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    cx.spawn_in(window, async move |_, cx| {
      loop {
        cx.background_executor()
          .timer(duration::ANIMATION_FRAME)
          .await;

        let keep_running = if let Some(state) = state.upgrade() {
          state.update(cx, |state, cx| {
            let Some(item) = state.items.iter_mut().find(|item| item.id == id) else {
              return false;
            };

            if !item.autohide || item.timer_epoch != epoch {
              return false;
            }

            let Some(started_at) = item.started_at else {
              return false;
            };

            if started_at.elapsed() >= duration {
              state.close(id);
              cx.notify();
              return false;
            }

            cx.notify();
            true
          })
        } else {
          false
        };

        if !keep_running {
          break;
        }
      }
    })
    .detach();
  }

  fn restart_timer(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
    let Some((duration, epoch)) =
      self
        .items
        .iter_mut()
        .find(|item| item.id == id)
        .and_then(|item| {
          if !item.autohide {
            return None;
          }

          item.hovered = false;
          item.started_at = Some(Instant::now());
          item.timer_epoch = item.timer_epoch.wrapping_add(1);

          Some((item.duration, item.timer_epoch))
        })
    else {
      return;
    };

    let state = cx.entity().downgrade();
    Self::spawn_timer(state, id, duration, epoch, window, cx);
  }

  fn pause_and_reset_timer(&mut self, id: usize) {
    if let Some(item) = self.items.iter_mut().find(|item| item.id == id)
      && item.autohide
    {
      item.hovered = true;
      item.started_at = None;
      item.timer_epoch = item.timer_epoch.wrapping_add(1);
    }
  }

  fn progress_ratio(&self, id: usize) -> Option<f32> {
    let item = self.items.iter().find(|item| item.id == id)?;
    if !item.autohide {
      return None;
    }

    if item.hovered || item.started_at.is_none() {
      return Some(1.0);
    }

    let duration = item.duration.as_secs_f32();
    if duration <= f32::EPSILON {
      return Some(0.0);
    }

    let elapsed = item.started_at?.elapsed().as_secs_f32();
    Some((1.0 - elapsed / duration).clamp(0.0, 1.0))
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
  margin_top: gpuim::Pixels,
  margin_right: gpuim::Pixels,
  margin_bottom: gpuim::Pixels,
  margin_left: gpuim::Pixels,
  width: gpuim::Pixels,
  size: Size,
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
      size: Size::Medium,
    }
  }

  pub fn placement(mut self, placement: NotificationPlacement) -> Self {
    self.placement = placement;
    self
  }

  pub fn margins(
    mut self, top: gpuim::Pixels, right: gpuim::Pixels, bottom: gpuim::Pixels, left: gpuim::Pixels,
  ) -> Self {
    self.margin_top = top;
    self.margin_right = right;
    self.margin_bottom = bottom;
    self.margin_left = left;
    self
  }

  pub fn width(mut self, width: gpuim::Pixels) -> Self {
    self.width = width;
    self
  }
}

impl_sizable!(NotificationCenter);
impl_styled!(NotificationCenter);

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
      .container_gap(self.size)
      .when(placement.is_bottom(), |this| this.flex_col_reverse())
      .children(items.into_iter().map(|item| {
        NotificationCard::new(item.id, item.data, &self.state, self.size).into_any_element()
      }))
      .refine_style(&self.style)
  }
}

#[derive(IntoElement)]
struct NotificationCard {
  id: usize,
  data: Notification,
  state: Entity<NotificationState>,
  size: Size,
}

impl NotificationCard {
  fn new(id: usize, data: Notification, state: &Entity<NotificationState>, size: Size) -> Self {
    Self {
      id,
      data,
      state: state.clone(),
      size,
    }
  }
}

impl RenderOnce for NotificationCard {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let icon_color = self.data.type_.color(cx);
    let icon = self.data.icon.unwrap_or_else(|| self.data.type_.icon());
    let progress_ratio = self.state.read(cx).progress_ratio(self.id);

    v_flex()
      .id(("notification-card", self.id as u64))
      .w_full()
      .popover_style(cx.theme())
      .container_padding(self.size)
      .items_start()
      .on_hover({
        let state = self.state.clone();
        let id = self.id;
        move |is_hovered, window, cx| {
          state.update(cx, |state, cx| {
            if *is_hovered {
              state.pause_and_reset_timer(id);
            } else {
              state.restart_timer(id, window, cx);
            }
            cx.notify();
          });
        }
      })
      .child(
        h_flex()
          .w_full()
          .items_center()
          .bg(icon_color.opacity(0.2))
          .pl_2()
          .container_gap(self.size)
          .rounded(cx.theme().radius)
          .child(icon.text_color(icon_color))
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
              .icon(Icon::new(IconName::Dismiss))
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
      .child(
        v_flex()
          .w_full()
          .container_padding(self.size)
          .flex_1()
          .text_color(cx.theme().muted_foreground)
          .when_some(self.data.message.clone(), |this, message| {
            this.child(message)
          }),
      )
      .child(h_flex().w_full().justify_end().when_some(
        self.data.action_label.clone(),
        |this, action_label| {
          this.child(
            Button::new(("notification-action", self.id as u64))
              .primary()
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
      .when_some(progress_ratio, |this, progress_ratio| {
        this.child(
          div()
            .w_full()
            .h(px(2.0))
            .rounded(px(1.0))
            .bg(cx.theme().border.opacity(0.35))
            .child(
              div()
                .h_full()
                .rounded(px(1.0))
                .bg(icon_color)
                .w(relative(progress_ratio)),
            ),
        )
      })
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
