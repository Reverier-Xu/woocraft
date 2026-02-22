use std::rc::Rc;

use chrono::NaiveDate;
use gpui::{
  AnyElement, App, AppContext, ClickEvent, Context, ElementId, Empty, Entity, EventEmitter,
  FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding, MouseButton,
  ParentElement as _, Render, RenderOnce, SharedString, StyleRefinement, Styled, Subscription,
  Window, div, prelude::FluentBuilder as _, px,
};

use crate::{
  ActiveTheme, Anchor, Button, ButtonVariants as _, Calendar, CalendarEvent, CalendarState, Date,
  Delete, Disableable, Icon, IconName, Matcher, Popover, Selectable, Sizable, Size, StyledExt as _,
  actions::{Cancel, Confirm},
  h_flex, translate, v_flex,
};

const CONTEXT: &str = "DatePicker";

pub(crate) fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("enter", Confirm { secondary: false }, Some(CONTEXT)),
    KeyBinding::new("escape", Cancel, Some(CONTEXT)),
    KeyBinding::new("delete", Delete, Some(CONTEXT)),
    KeyBinding::new("backspace", Delete, Some(CONTEXT)),
  ]);
}

/// Events emitted by the DatePicker.
#[derive(Clone)]
pub enum DatePickerEvent {
  Change(Date),
}

/// Preset value for DateRangePreset.
#[derive(Clone)]
pub enum DateRangePresetValue {
  Single(NaiveDate),
  Range(NaiveDate, NaiveDate),
}

/// Preset for date range selection.
#[derive(Clone)]
pub struct DateRangePreset {
  label: SharedString,
  value: DateRangePresetValue,
}

impl DateRangePreset {
  /// Creates a new DateRangePreset with a date.
  pub fn single(label: impl Into<SharedString>, date: NaiveDate) -> Self {
    Self {
      label: label.into(),
      value: DateRangePresetValue::Single(date),
    }
  }

  /// Creates a new DateRangePreset with a range of dates.
  pub fn range(label: impl Into<SharedString>, start: NaiveDate, end: NaiveDate) -> Self {
    Self {
      label: label.into(),
      value: DateRangePresetValue::Range(start, end),
    }
  }
}

/// Stores the state of the date picker.
pub struct DatePickerState {
  focus_handle: FocusHandle,
  date: Date,
  open: bool,
  calendar: Entity<CalendarState>,
  date_format: SharedString,
  number_of_months: usize,
  disabled_matcher: Option<Rc<Matcher>>,
  _subscriptions: Vec<Subscription>,
}

impl Focusable for DatePickerState {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl EventEmitter<DatePickerEvent> for DatePickerState {}

impl DatePickerState {
  /// Create a date state.
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    Self::new_with_range(false, window, cx)
  }

  /// Create a date state with range mode.
  pub fn range(window: &mut Window, cx: &mut Context<Self>) -> Self {
    Self::new_with_range(true, window, cx)
  }

  fn new_with_range(is_range: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let date = if is_range {
      Date::Range(None, None)
    } else {
      Date::Single(None)
    };

    let calendar = cx.new(|cx| {
      let mut this = CalendarState::new(window, cx);
      this.set_date(date, window, cx);
      this
    });

    let subscriptions = vec![cx.subscribe_in(
      &calendar,
      window,
      |this, _, event: &CalendarEvent, window, cx| match event {
        CalendarEvent::Selected(date) => {
          this.update_date(*date, true, window, cx);
          this.focus_handle.focus(window);
        }
      },
    )];

    Self {
      focus_handle: cx.focus_handle(),
      date,
      calendar,
      open: false,
      date_format: "%Y/%m/%d".into(),
      number_of_months: 1,
      disabled_matcher: None,
      _subscriptions: subscriptions,
    }
  }

  /// Set display format, default: `%Y/%m/%d`.
  pub fn date_format(mut self, format: impl Into<SharedString>) -> Self {
    self.date_format = format.into();
    self
  }

  /// Set number of months in calendar view, default is 1.
  pub fn number_of_months(mut self, number_of_months: usize) -> Self {
    self.number_of_months = number_of_months;
    self
  }

  /// Get current date value.
  pub fn date(&self) -> Date {
    self.date
  }

  /// Set date value.
  pub fn set_date(&mut self, date: impl Into<Date>, window: &mut Window, cx: &mut Context<Self>) {
    self.update_date(date.into(), false, window, cx);
  }

  /// Set disabled matcher for the calendar.
  pub fn disabled_matcher(mut self, disabled: impl Into<Matcher>) -> Self {
    self.disabled_matcher = Some(Rc::new(disabled.into()));
    self
  }

  fn update_date(&mut self, date: Date, emit: bool, window: &mut Window, cx: &mut Context<Self>) {
    self.date = date;
    self.calendar.update(cx, |calendar, cx| {
      calendar.set_date(date, window, cx);
    });
    self.open = false;
    if emit {
      cx.emit(DatePickerEvent::Change(date));
    }
    cx.notify();
  }

  fn set_calendar_disabled_matcher(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    let matcher = self.disabled_matcher.clone();
    self.calendar.update(cx, |state, _| {
      state.disabled_matcher = matcher;
    });
  }

  fn on_escape(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
    if !self.open {
      cx.propagate();
      return;
    }

    self.open = false;
    self.focus_back_if_need(window, cx);
    cx.notify();
  }

  fn on_enter(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
    if !self.open {
      self.open = true;
      cx.notify();
    }
  }

  fn on_delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
    self.clean(&ClickEvent::default(), window, cx);
  }

  // If focus stays inside date picker when closing popover, focus back to input.
  fn focus_back_if_need(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.focus_handle.contains_focused(window, cx) {
      self.focus_handle.focus(window);
    }
  }

  fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
    if self.open == open {
      return;
    }

    self.open = open;
    if !open {
      self.focus_back_if_need(window, cx);
    }
    cx.notify();
  }

  fn clean(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
    cx.stop_propagation();
    match self.date {
      Date::Single(_) => self.update_date(Date::Single(None), true, window, cx),
      Date::Range(..) => self.update_date(Date::Range(None, None), true, window, cx),
    }
  }

  fn select_preset(
    &mut self, preset: &DateRangePreset, window: &mut Window, cx: &mut Context<Self>,
  ) {
    match preset.value {
      DateRangePresetValue::Single(single) => {
        self.update_date(Date::Single(Some(single)), true, window, cx)
      }
      DateRangePresetValue::Range(start, end) => {
        self.update_date(Date::Range(Some(start), Some(end)), true, window, cx)
      }
    }
  }
}

/// A DatePicker element.
#[derive(IntoElement)]
pub struct DatePicker {
  id: ElementId,
  style: StyleRefinement,
  state: Entity<DatePickerState>,
  cleanable: bool,
  placeholder: Option<SharedString>,
  size: Size,
  number_of_months: usize,
  presets: Option<Vec<DateRangePreset>>,
  appearance: bool,
  disabled: bool,
}

impl Sizable for DatePicker {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Focusable for DatePicker {
  fn focus_handle(&self, cx: &App) -> FocusHandle {
    self.state.focus_handle(cx)
  }
}

impl Styled for DatePicker {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl Disableable for DatePicker {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Render for DatePickerState {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    Empty
  }
}

impl DatePicker {
  /// Create a new DatePicker with the given [`DatePickerState`].
  pub fn new(state: &Entity<DatePickerState>) -> Self {
    Self {
      id: ("date-picker", state.entity_id()).into(),
      state: state.clone(),
      cleanable: false,
      placeholder: None,
      size: Size::default(),
      style: StyleRefinement::default(),
      number_of_months: 2,
      presets: None,
      appearance: true,
      disabled: false,
    }
  }

  /// Set placeholder.
  pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
    self.placeholder = Some(placeholder.into());
    self
  }

  /// Set whether clear button should be shown when value exists.
  pub fn cleanable(mut self, cleanable: bool) -> Self {
    self.cleanable = cleanable;
    self
  }

  /// Set preset ranges for date selection.
  pub fn presets(mut self, presets: Vec<DateRangePreset>) -> Self {
    self.presets = Some(presets);
    self
  }

  /// Set number of months to display in calendar.
  pub fn number_of_months(mut self, number_of_months: usize) -> Self {
    self.number_of_months = number_of_months;
    self
  }

  /// Set appearance style.
  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }
}

fn clear_button(id: impl Into<ElementId>, cx: &App) -> Button {
  Button::new(id)
    .icon(IconName::DismissCircle)
    .flat()
    .small()
    .tab_stop(false)
    .text_color(cx.theme().muted_foreground)
}

impl RenderOnce for DatePicker {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    self.state.update(cx, |state, cx| {
      state.set_calendar_disabled_matcher(window, cx);
      state.number_of_months = self.number_of_months.max(1);
    });

    let state_view = self.state.read(cx);
    let show_clean = self.cleanable && state_view.date.is_some() && !self.disabled;
    let placeholder = self
      .placeholder
      .clone()
      .unwrap_or_else(|| SharedString::from(translate("date_picker.placeholder")));
    let display_title = state_view
      .date
      .format(&state_view.date_format)
      .unwrap_or_else(|| placeholder.clone());
    let open = state_view.open;
    let is_focused = state_view.focus_handle.is_focused(window) && !self.disabled;
    let number_of_months = state_view.number_of_months;
    let _ = state_view;

    let trigger_state = self.state.clone();
    let trigger = Button::new(("date-picker-trigger", self.state.entity_id()))
      .with_size(self.size)
      .map(|this| {
        if self.appearance {
          this.default()
        } else {
          this.flat()
        }
      })
      .selected(open || is_focused)
      .disabled(self.disabled)
      .expand(true)
      .child(
        h_flex()
          .w_full()
          .items_center()
          .justify_between()
          .min_w_0()
          .child(
            div()
              .min_w_0()
              .overflow_hidden()
              .when(!trigger_state.read(cx).date.is_some(), |this| {
                this.text_color(cx.theme().muted_foreground)
              })
              .child(display_title),
          )
          .child(
            h_flex()
              .items_center()
              .gap_1()
              .when(show_clean, |this| {
                this.child(
                  div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                      cx.stop_propagation();
                    })
                    .child(
                      clear_button(("date-picker-clean", self.state.entity_id()), cx).on_click({
                        let state = self.state.clone();
                        move |_, window, cx| {
                          state.update(cx, |state, cx| {
                            state.clean(&ClickEvent::default(), window, cx);
                            state.focus_handle.focus(window);
                          });
                        }
                      }),
                    ),
                )
              })
              .when(!show_clean, |this| {
                this.child(
                  Icon::new(IconName::Calendar)
                    .with_size(self.size.smaller())
                    .text_color(cx.theme().muted_foreground),
                )
              }),
          ),
      );

    let state_for_popover = self.state.clone();
    let presets = self.presets.clone();
    let size = self.size;

    let field: AnyElement = if self.disabled {
      trigger.into_any_element()
    } else {
      Popover::new(("date-picker-popover", self.state.entity_id()))
        .anchor(Anchor::TopLeft)
        .open(open)
        .track_focus(&self.focus_handle(cx))
        .overlay_closable(true)
        .on_open_change({
          let state = self.state.clone();
          move |open, window, cx| {
            state.update(cx, |state, cx| {
              state.set_open(*open, window, cx);
            });
          }
        })
        .trigger(trigger)
        .content(move |_, _window, cx| {
          h_flex()
            .gap_3()
            .items_start()
            .when_some(presets.clone(), |this, presets| {
              this.child(v_flex().my_1().gap_2().justify_end().children(
                presets.into_iter().enumerate().map(|(ix, preset)| {
                  Button::new(("preset", ix))
                    .small()
                    .flat()
                    .tab_stop(false)
                    .label(preset.label.clone())
                    .on_click({
                      let state = state_for_popover.clone();
                      move |_, window, cx| {
                        state.update(cx, |state, cx| {
                          state.select_preset(&preset, window, cx);
                        });
                      }
                    })
                }),
              ))
            })
            .child(
              Calendar::new(&state_for_popover.read(cx).calendar)
                .number_of_months(number_of_months)
                .border_0()
                .rounded(px(0.0))
                .p_0()
                .with_size(size),
            )
        })
        .into_any_element()
    };

    div()
      .id(self.id.clone())
      .w_full()
      .key_context(CONTEXT)
      .track_focus(&self.focus_handle(cx).tab_stop(true))
      .when(!self.disabled, |this| {
        this
          .on_action(window.listener_for(&self.state, DatePickerState::on_enter))
          .on_action(window.listener_for(&self.state, DatePickerState::on_delete))
      })
      .when(open && !self.disabled, |this| {
        this.on_action(window.listener_for(&self.state, DatePickerState::on_escape))
      })
      .refine_style(&self.style)
      .child(field)
  }
}
