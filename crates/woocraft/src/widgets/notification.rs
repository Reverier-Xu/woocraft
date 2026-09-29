use std::{
  collections::VecDeque,
  rc::Rc,
  time::{Duration, Instant},
};

use gpui::{
  App, Context, ElementId, Entity, InteractiveElement as _, IntoElement, ParentElement, Pixels,
  Rems, Render, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled,
  Window, div, prelude::FluentBuilder as _, px, relative, rems,
};

use crate::{
  ActiveTheme, Button, ButtonVariants, CardStyle, ColorExt, Easing, ElementExt, Icon, IconName,
  Presence, Size, StyleSized, StyledExt, Transition, duration, h_flex, v_flex,
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

  fn is_left(self) -> bool {
    matches!(self, Self::TopLeft | Self::BottomLeft)
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

  fn color(&self, cx: &App) -> gpui::Hsla {
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
  started_at: Option<Instant>,
  hovered: bool,
  /// Set when the notification is closing: the card stays mounted while its
  /// exit animation plays and is purged once the animation completes.
  closing: bool,
  /// Last measured card height, recorded during prepaint while the card is
  /// not closing. Drives the height collapse that lets the remaining cards
  /// slide into the freed slot instead of jumping.
  measured_height: Pixels,
}

pub struct NotificationState {
  items: VecDeque<NotificationItem>,
  max_items: usize,
  next_id: usize,
  /// Whether the shared autohide tick loop is running. The loop clears it as
  /// it exits, so an idle queue arms no timer at all.
  is_advancing: bool,
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
      is_advancing: false,
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
    self.items.push_back(NotificationItem {
      id,
      key: notification.key.clone(),
      data: notification,
      started_at: None,
      hovered: false,
      closing: false,
      measured_height: px(0.),
    });

    while self.items.len() > self.max_items {
      self.items.pop_front();
    }

    if autohide {
      self.restart_timer(id, window, cx);
    }

    cx.notify();
  }

  /// Drive all autohide timers from a single shared tick loop.
  ///
  /// One windowed tick advances every running notification and closes the
  /// expired ones, so the whole queue pays for one timer regardless of its
  /// size. Hover pause/resume only flips per-item state and re-arms this
  /// loop, which is a no-op while it is already ticking.
  fn start_advancing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.is_advancing {
      return;
    }
    self.is_advancing = true;
    // Detached rather than kept as a `Task`: the loop ends itself once no
    // item needs animating, and holding a handle would keep the state entity
    // alive only to drop it from inside its own future.
    cx.spawn_in(window, async move |state, cx| {
      loop {
        cx.background_executor()
          .timer(duration::ANIMATION_FRAME)
          .await;

        let Ok(running) = state.update(cx, |state, cx| state.advance(cx)) else {
          break;
        };
        if !running {
          break;
        }
      }
    })
    .detach();
  }

  /// Advance every autohide item by one tick of the shared loop.
  ///
  /// Returns `true` while at least one notification still needs the loop:
  /// active (autohide, not hovered) items keep it alive; paused or sticky
  /// ones do not. `cx.notify()` fires only when something visible can
  /// change — an item was removed, or a running progress bar moved — so
  /// parked notifications cost no re-renders.
  fn advance(&mut self, cx: &mut Context<Self>) -> bool {
    let now = Instant::now();

    let mut expired_ids = Vec::new();
    for item in &self.items {
      if item.data.autohide
        && !item.hovered
        && !item.closing
        && item
          .started_at
          .is_some_and(|started_at| now.duration_since(started_at) >= item.data.duration)
      {
        expired_ids.push(item.id);
      }
    }

    let any_expired = !expired_ids.is_empty();
    for id in expired_ids {
      self.close(id);
    }

    let keep_running = self.items.iter().any(|item| {
      item.data.autohide && !item.hovered && !item.closing && item.started_at.is_some()
    });
    self.is_advancing = keep_running;

    if any_expired || keep_running {
      cx.notify();
    }

    keep_running
  }

  /// Restart (or start) the countdown for one notification and make sure the
  /// shared advancing loop is running.
  fn restart_timer(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
    let resumed = self
      .items
      .iter_mut()
      .find(|item| item.id == id)
      .is_some_and(|item| {
        if !item.data.autohide {
          return false;
        }

        item.hovered = false;
        item.started_at = Some(Instant::now());
        true
      });

    if resumed {
      self.start_advancing(window, cx);
    }
  }

  /// Pause the countdown while a notification is hovered; its progress bar
  /// stays parked at full width until the pointer leaves.
  fn pause_and_reset_timer(&mut self, id: usize) {
    if let Some(item) = self.items.iter_mut().find(|item| item.id == id)
      && item.data.autohide
    {
      item.hovered = true;
      item.started_at = None;
    }
  }

  fn progress_ratio(&self, id: usize) -> Option<f32> {
    let item = self.items.iter().find(|item| item.id == id)?;
    if !item.data.autohide {
      return None;
    }

    if item.hovered || item.started_at.is_none() {
      return Some(1.0);
    }

    let duration = item.data.duration.as_secs_f32();
    if duration <= f32::EPSILON {
      return Some(0.0);
    }

    let elapsed = item.started_at?.elapsed().as_secs_f32();
    Some((1.0 - elapsed / duration).clamp(0.0, 1.0))
  }

  /// Start closing a notification.
  ///
  /// The item is kept in the queue with `closing` set so the card can play
  /// its exit animation (fade, slide, and height collapse); the card purges
  /// itself from the queue once the animation completes. Callers must notify
  /// to trigger the render that starts the exit.
  pub fn close(&mut self, id: usize) {
    if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
      item.closing = true;
    }
  }

  /// Drop every notification immediately, skipping exit animations.
  pub fn clear(&mut self) {
    self.items.clear();
  }
}

#[derive(IntoElement)]
pub struct NotificationCenter {
  state: Entity<NotificationState>,
  style: StyleRefinement,
  placement: NotificationPlacement,
  margin_top: Rems,
  margin_right: Rems,
  margin_bottom: Rems,
  margin_left: Rems,
  width: Rems,
  size: Size,
}

impl NotificationCenter {
  pub fn new(state: &Entity<NotificationState>) -> Self {
    Self {
      state: state.clone(),
      style: StyleRefinement::default(),
      placement: NotificationPlacement::TopRight,
      margin_top: rems(1.),
      margin_right: rems(1.),
      margin_bottom: rems(1.),
      margin_left: rems(1.),
      width: rems(23.),
      size: Size::Medium,
    }
  }

  pub fn placement(mut self, placement: NotificationPlacement) -> Self {
    self.placement = placement;
    self
  }

  pub fn margins(mut self, top: Rems, right: Rems, bottom: Rems, left: Rems) -> Self {
    self.margin_top = top;
    self.margin_right = right;
    self.margin_bottom = bottom;
    self.margin_left = left;
    self
  }

  pub fn width(mut self, width: Rems) -> Self {
    self.width = width;
    self
  }
}

impl_sizable!(NotificationCenter);
impl_styled!(NotificationCenter);

impl RenderOnce for NotificationCenter {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // Bridge `NotificationState` notifications to the view that is rendering
    // this center: without it, pushes, autohide closes and progress ticks
    // would only repaint when the parent view happens to re-render for other
    // reasons. This mirrors the wiring gpui does inside `use_keyed_state` —
    // the observer is registered exactly once per element-state lifetime
    // (keyed by the state entity id), detached, and forwards each state
    // notification to the current view, which keeps the refresh chain
    // explicit instead of coincidental.
    let current_view = window.current_view();
    let observed_state = self.state.clone();
    window.use_keyed_state::<()>(
      ElementId::named_usize(
        "notification-center-observer",
        self.state.entity_id().as_u64() as usize,
      ),
      cx,
      |_, cx| {
        App::observe(cx, &observed_state, move |_, cx| cx.notify(current_view)).detach();
      },
    );

    let item_count = self.state.read(cx).items.len();
    let placement = self.placement;
    let half_width = rems(-(self.width.0 / 2.0));

    let container = v_flex()
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
        |this| this.top(self.margin_top).left_1_2().ml(half_width),
      )
      .when(
        matches!(placement, NotificationPlacement::BottomCenter),
        |this| this.bottom(self.margin_bottom).left_1_2().ml(half_width),
      )
      .w(self.width)
      .max_h(rems(35.))
      .container_gap(self.size)
      .when(placement.is_bottom(), |this| this.flex_col_reverse())
      .refine_style(&self.style);

    if item_count == 0 {
      return container.into_any_element();
    }

    // Snapshot the items before building cards: card rendering samples
    // presence state and records measurements through `state.update`, which
    // would conflict with a read borrow held across the build.
    let items: Vec<_> = self
      .state
      .read(cx)
      .items
      .iter()
      .map(|item| {
        (
          item.id,
          item.data.clone(),
          item.closing,
          item.measured_height,
        )
      })
      .collect();

    let cards: Vec<_> = items
      .into_iter()
      .map(|(id, data, closing, measured_height)| {
        NotificationCard::new(
          id,
          data,
          &self.state,
          self.size,
          placement.is_left(),
          placement.is_bottom(),
          closing,
          measured_height,
        )
        .into_any_element()
      })
      .collect();

    container.children(cards).into_any_element()
  }
}

#[derive(IntoElement)]
struct NotificationCard {
  id: usize,
  data: Notification,
  state: Entity<NotificationState>,
  size: Size,
  /// Cards anchored at the left edge slide in from the left.
  from_left: bool,
  /// Cards anchored at the bottom stack upward (column-reverse), which flips
  /// which margin reclaims the flex gap during the exit collapse.
  from_bottom: bool,
  /// Whether the card is playing its exit animation.
  closing: bool,
  /// Natural card height measured while the card was not closing.
  measured_height: Pixels,
}

impl NotificationCard {
  #[allow(clippy::too_many_arguments)]
  fn new(
    id: usize, data: Notification, state: &Entity<NotificationState>, size: Size, from_left: bool,
    from_bottom: bool, closing: bool, measured_height: Pixels,
  ) -> Self {
    Self {
      id,
      data,
      state: state.clone(),
      size,
      from_left,
      from_bottom,
      closing,
      measured_height,
    }
  }
}

impl RenderOnce for NotificationCard {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let icon_color = self.data.type_.color(cx);
    let icon = self.data.icon.unwrap_or_else(|| self.data.type_.icon());
    let progress_ratio = self.state.read(cx).progress_ratio(self.id);

    // Enter fades and slides in from the anchored edge; close runs the same
    // presence in reverse (fade + slide back out) while the wrapper below
    // collapses the card's slot to zero height, so the remaining cards slide
    // into place instead of jumping. When the exit completes the card purges
    // itself from the queue.
    let transition = if self.closing {
      Transition::new(duration::NOTIFICATION_EXIT).easing(Easing::EaseIn)
    } else {
      Transition::new(duration::NOTIFICATION_ENTER).easing(Easing::EaseOut)
    };
    let sample = Presence::new((self.id, "notification-card"), !self.closing)
      .transition(transition)
      .sample(window, cx);

    if self.closing && !sample.should_render() {
      // Exit finished: drop the item from the queue and notify, so the next
      // frame rebuilds without this card's (empty) placeholder and its flex
      // gap does not linger.
      self.state.update(cx, |state, cx| {
        state.items.retain(|item| item.id != self.id);
        cx.notify();
      });
      return div().into_any_element();
    }

    let presence = sample.progress;
    let slide = if self.from_left { -24.0 } else { 24.0 } * (1.0 - presence);
    let gap = self.size.container_gap().to_pixels(window.rem_size());

    let card = v_flex()
      .w_full()
      .popover_style(cx.theme())
      .container_padding(self.size)
      .items_start()
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
            .h(rems(0.125))
            .rounded(px(1.))
            .bg(cx.theme().border.opacity(0.35))
            .child(
              div()
                .h_full()
                .rounded(px(1.))
                .bg(icon_color)
                .w(relative(progress_ratio)),
            ),
        )
      });

    // The wrapper owns the presence-driven opacity/slide and the exit
    // collapse; the card inside keeps its natural height so its content is
    // clipped by the shrinking slot instead of reflowing mid-animation.
    div()
      .id(("notification-card", self.id as u64))
      .w_full()
      .opacity(presence)
      .relative()
      .left(px(slide))
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
      .on_prepaint({
        let state = self.state.clone();
        let id = self.id;
        let closing = self.closing;
        move |bounds, _, cx| {
          // Record the natural card height for the exit collapse. Skipped
          // while closing: the wrapper's explicit collapsing height would
          // otherwise feed back into the measurement.
          if closing {
            return;
          }
          state.update(cx, |state, _| {
            if let Some(item) = state.items.iter_mut().find(|item| item.id == id)
              && item.measured_height != bounds.size.height
            {
              item.measured_height = bounds.size.height;
            }
          });
        }
      })
      .when(self.closing, |this| {
        // Reclaim the flex gap around this slot in step with the height
        // collapse, so removing the card at the end of the exit leaves the
        // remaining cards exactly where the animation put them.
        let collapse = -gap * (1.0 - presence);
        this
          .h(self.measured_height * presence)
          .overflow_hidden()
          .when(self.from_bottom, |this| this.mt(collapse))
          .when(!self.from_bottom, |this| this.mb(collapse))
      })
      .when(!self.closing, |this| {
        this.when_some(self.data.on_click.clone(), |this, on_click| {
          this.cursor_pointer().on_click(move |_, window, cx| {
            on_click(window, cx);
          })
        })
      })
      .child(card)
      .into_any_element()
  }
}

// `NotificationState` is a plain state entity and is never mounted in a
// window's view tree: the UI refreshes through the observer bridged by
// `NotificationCenter::render`. This empty `Render` impl is kept only so the
// entity stays renderable for callers that hold it as a view handle; it plays
// no part in the refresh chain.
impl Render for NotificationState {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
  }
}
