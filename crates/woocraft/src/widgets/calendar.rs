//! Date picker calendar component with month/year navigation.
//!
//! Calendar provides an interactive month-view widget for selecting single
//! dates or date ranges. Users can navigate months, years, and decades with
//! dedicated buttons. Supports custom disabled date rules via matcher
//! predicates. Typically used as the underlying component for date picker
//! popups, though it can be embedded directly in forms.
//!
//! # Features
//! - **Single or Range Selection**: Pick one date or a continuous date range
//! - **Month/Year Navigation**: Arrows to move between months, year/decade
//!   picker for faster navigation
//! - **Disabled Dates**: Define custom rules to disable specific dates (e.g.,
//!   weekends, past dates)
//! - **Today Indicator**: Visual highlight of the current date
//! - **Multi-Month View**: Optionally display 2+ months side-by-side (for range
//!   selection)
//! - **Keyboard Accessible**: Tab support and focus management
//!
//! # Example
//! ```rust,ignore
//! use woocraft::{Calendar, Matcher, Date};
//!
//! // Single date picker - disable weekends
//! let calendar = Calendar::new("date_picker", cx)
//!   .with_disabled_matcher(Matcher::weekends());
//!
//! // Range picker spanning 2 months
//! let range_cal = Calendar::new("range_picker", cx)
//!   .set_number_of_months(2);
//! ```
//!
//! # Performance Notes
//! Calendar renders efficiently even for large date ranges. The disabled
//! matcher is evaluated per cell, so keep matcher predicates fast. Navigation
//! between months/years is instant as date calculations happen in-place without
//! re-rendering the entire grid.

use std::rc::Rc;

use chrono::{Datelike, NaiveDate};
use gpui::{
  App, ClickEvent, Context, Div, ElementId, Empty, Entity, EventEmitter, FocusHandle,
  InteractiveElement, IntoElement, ParentElement, Render, RenderOnce, SharedString, Stateful,
  StatefulInteractiveElement, StyleRefinement, Styled, Window, prelude::FluentBuilder as _, px,
  relative,
};

use crate::{
  ActiveTheme, Button, ButtonVariants as _, Date, Disableable as _, Icon, IconName, Matcher,
  Selectable, Sizable, Size, StyledExt as _, h_flex, local_today, month_days, translate_woocraft,
  v_flex,
};

/// Events emitted by the calendar component.
pub enum CalendarEvent {
  /// Emitted when the user selects or changes a date/date range.
  /// Contains the selected `Date` (single or range).
  Selected(Date),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
  Day,
  Month,
  Year,
}

impl ViewMode {
  fn is_day(&self) -> bool {
    matches!(self, Self::Day)
  }

  fn is_month(&self) -> bool {
    matches!(self, Self::Month)
  }

  fn is_year(&self) -> bool {
    matches!(self, Self::Year)
  }
}

#[derive(IntoElement)]
/// Interactive calendar widget for date selection.
///
/// `Calendar` provides a month-view grid with navigation controls for picking
/// single dates or continuous date ranges. The component manages internal state
/// (current month/year, selection, year page) while delegating styling and
/// sizing to the parent context via traits.
///
/// Most configuration happens through the associated `CalendarState`, accessed
/// via context updates. The `Calendar` itself is a relatively thin wrapper
/// around the renderer.
pub struct Calendar {
  id: ElementId,
  size: Size,
  state: Entity<CalendarState>,
  style: StyleRefinement,
  /// Number of the months view to show.
  number_of_months: usize,
}

/// Internal state management for the calendar component.
///
/// Manages the current view mode (day/month/year), selected date(s), current
/// month/year, year pagination, and disabled date rules. Updates to
/// CalendarState trigger calendar re-renders and emit `CalendarEvent` when
/// dates are selected.
///
/// # Configuration Methods
/// - `disabled_matcher()`: Define which dates cannot be selected
/// - `set_number_of_months()`: Display multiple months (useful for range
///   selection)
/// - `year_range()`: Set the range of years available for navigation
pub struct CalendarState {
  focus_handle: FocusHandle,
  view_mode: ViewMode,
  date: Date,
  current_year: i32,
  current_month: u8,
  years: Vec<Vec<i32>>,
  year_page: i32,
  today: NaiveDate,
  /// Number of the months view to show.
  number_of_months: usize,
  pub(crate) disabled_matcher: Option<Rc<Matcher>>,
}

impl CalendarState {
  /// Create a new calendar state with today's date as the initial view.
  ///
  /// Defaults to single-month view, Day mode, and a 100-year range (±50 years
  /// from current year).
  pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
    let today = local_today();
    Self {
      focus_handle: cx.focus_handle(),
      view_mode: ViewMode::Day,
      date: Date::Single(None),
      current_month: today.month() as u8,
      current_year: today.year(),
      years: vec![],
      year_page: 0,
      today,
      number_of_months: 1,
      disabled_matcher: None,
    }
    .year_range((today.year() - 50, today.year() + 50))
  }

  /// Set a matcher for disabling specific dates.
  ///
  /// Any date matching the `Matcher` predicate cannot be selected. Builder
  /// method.
  pub fn disabled_matcher(mut self, matcher: impl Into<Matcher>) -> Self {
    self.disabled_matcher = Some(Rc::new(matcher.into()));
    self
  }

  /// Update the disabled date matcher on an existing calendar.
  ///
  /// The disabled matcher determines which dates appear grayed out and cannot
  /// be clicked. Use `Matcher::weekends()` to disable weekends, or construct
  /// custom matchers.
  pub fn set_disabled_matcher(
    &mut self, disabled: impl Into<Matcher>, _: &mut Window, _: &mut Context<Self>,
  ) {
    self.disabled_matcher = Some(Rc::new(disabled.into()));
  }

  /// Set the selected date(s) and update the calendar view to show the
  /// selection.
  ///
  /// For `Date::Single(Some(date))`, the calendar jumps to that month.
  /// For `Date::Range(Some(start), Some(end))`, jumps to the month of the start
  /// date. If the date matches the disabled matcher, the change is ignored.
  pub fn set_date(&mut self, date: impl Into<Date>, _: &mut Window, cx: &mut Context<Self>) {
    let date = date.into();
    let invalid = self
      .disabled_matcher
      .as_ref()
      .is_some_and(|matcher| matcher.is_match(&date));
    if invalid {
      return;
    }

    self.date = date;
    match self.date {
      Date::Single(Some(date)) => {
        self.current_month = date.month() as u8;
        self.current_year = date.year();
      }
      Date::Range(Some(start), _) => {
        self.current_month = start.month() as u8;
        self.current_year = start.year();
      }
      _ => {}
    }

    cx.notify();
  }

  /// Get the currently selected date(s).
  pub fn date(&self) -> Date {
    self.date
  }

  /// Set the number of months to display horizontally.
  ///
  /// Useful for range pickers where showing 2 months side-by-side helps users
  /// compare dates. Default: 1.
  pub fn set_number_of_months(
    &mut self, number_of_months: usize, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.number_of_months = number_of_months;
    cx.notify();
  }

  /// Set the year range available for navigation.
  ///
  /// Years are grouped into pages of 20. Default range: ±50 years from current
  /// year. # Arguments
  /// * `range` - Tuple of (start_year, end_year) inclusive
  pub fn year_range(mut self, range: (i32, i32)) -> Self {
    self.years = (range.0..range.1)
      .collect::<Vec<_>>()
      .chunks(20)
      .map(|chunk| chunk.to_vec())
      .collect::<Vec<_>>();
    self.year_page = self
      .years
      .iter()
      .position(|years| years.contains(&self.current_year))
      .unwrap_or(0) as i32;
    self
  }

  /// Get year and month by month offset.
  fn offset_year_month(&self, offset_month: usize) -> (i32, u32) {
    let mut month = self.current_month as i32 + offset_month as i32;
    let mut year = self.current_year;
    while month < 1 {
      month += 12;
      year -= 1;
    }
    while month > 12 {
      month -= 12;
      year += 1;
    }

    (year, month as u32)
  }

  /// Returns the month day matrices for rendering.
  fn days(&self) -> Vec<Vec<NaiveDate>> {
    (0..self.number_of_months)
      .flat_map(|offset| month_days(self.current_year, self.current_month as u32 + offset as u32))
      .collect()
  }

  fn has_prev_year_page(&self) -> bool {
    self.year_page > 0
  }

  fn has_next_year_page(&self) -> bool {
    self.year_page < self.years.len() as i32 - 1
  }

  fn prev_year_page(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    if !self.has_prev_year_page() {
      return;
    }

    self.year_page -= 1;
    cx.notify();
  }

  fn next_year_page(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    if !self.has_next_year_page() {
      return;
    }

    self.year_page += 1;
    cx.notify();
  }

  fn prev_month(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.current_month = if self.current_month == 1 {
      12
    } else {
      self.current_month - 1
    };
    self.current_year = if self.current_month == 12 {
      self.current_year - 1
    } else {
      self.current_year
    };
    cx.notify();
  }

  fn next_month(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.current_month = if self.current_month == 12 {
      1
    } else {
      self.current_month + 1
    };
    self.current_year = if self.current_month == 1 {
      self.current_year + 1
    } else {
      self.current_year
    };
    cx.notify();
  }

  fn month_name(&self, offset_month: usize) -> SharedString {
    const MONTH_KEYS: [&str; 12] = [
      "calendar.month.january",
      "calendar.month.february",
      "calendar.month.march",
      "calendar.month.april",
      "calendar.month.may",
      "calendar.month.june",
      "calendar.month.july",
      "calendar.month.august",
      "calendar.month.september",
      "calendar.month.october",
      "calendar.month.november",
      "calendar.month.december",
    ];

    let (_, month) = self.offset_year_month(offset_month);
    SharedString::from(translate_woocraft(
      MONTH_KEYS[(month.saturating_sub(1)) as usize],
    ))
  }

  fn year_name(&self, offset_month: usize) -> SharedString {
    let (year, _) = self.offset_year_month(offset_month);
    year.to_string().into()
  }

  fn set_view_mode(&mut self, mode: ViewMode, _: &mut Window, cx: &mut Context<Self>) {
    self.view_mode = mode;
    cx.notify();
  }

  fn months(&self) -> Vec<SharedString> {
    const MONTH_KEYS: [&str; 12] = [
      "calendar.month.january",
      "calendar.month.february",
      "calendar.month.march",
      "calendar.month.april",
      "calendar.month.may",
      "calendar.month.june",
      "calendar.month.july",
      "calendar.month.august",
      "calendar.month.september",
      "calendar.month.october",
      "calendar.month.november",
      "calendar.month.december",
    ];

    MONTH_KEYS
      .iter()
      .map(|key| SharedString::from(translate_woocraft(*key)))
      .collect()
  }
}

impl Render for CalendarState {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    Empty
  }
}

impl Calendar {
  /// Create a new calendar element with [`CalendarState`].
  pub fn new(state: &Entity<CalendarState>) -> Self {
    Self {
      id: ("calendar", state.entity_id()).into(),
      size: Size::default(),
      state: state.clone(),
      style: StyleRefinement::default(),
      number_of_months: 1,
    }
  }

  /// Set number of months to show, default is 1.
  pub fn number_of_months(mut self, number_of_months: usize) -> Self {
    self.number_of_months = number_of_months;
    self
  }

  fn render_day(
    &self, day_date: &NaiveDate, offset_month: usize, window: &mut Window, cx: &mut App,
  ) -> Stateful<Div> {
    let state = self.state.read(cx);
    let (_, month) = state.offset_year_month(offset_month);
    let day = day_date.day();
    let is_current_month = day_date.month() == month;
    let is_active = state.date.is_active(day_date);
    let is_in_range = state.date.is_in_range(day_date);

    let date = *day_date;
    let is_today = *day_date == state.today;
    let disabled = state
      .disabled_matcher
      .as_ref()
      .is_some_and(|matcher| matcher.matched(&date));

    let date_id: SharedString = format!("{}_{}", date.format("%Y-%m-%d"), offset_month).into();

    self
      .item_button(
        date_id,
        day.to_string(),
        is_active,
        is_in_range,
        !is_current_month || disabled,
        disabled,
        window,
        cx,
      )
      .when(is_today && !is_active, |this| {
        this.border_1().border_color(cx.theme().border)
      })
      .when(!disabled, |this| {
        this.on_click(
          window.listener_for(&self.state, move |state, _: &ClickEvent, window, cx| {
            if state.date.is_single() {
              state.set_date(date, window, cx);
              cx.emit(CalendarEvent::Selected(state.date()));
              return;
            }

            let start = state.date.start();
            let end = state.date.end();
            if start.is_none() && end.is_none() {
              state.set_date(Date::Range(Some(date), None), window, cx);
            } else if let Some(start) = start {
              if end.is_none() {
                if date < start {
                  state.set_date(Date::Range(Some(date), None), window, cx);
                } else {
                  state.set_date(Date::Range(Some(start), Some(date)), window, cx);
                }
              } else {
                state.set_date(Date::Range(Some(date), None), window, cx);
              }
            }

            if state.date.is_complete() {
              cx.emit(CalendarEvent::Selected(state.date()));
            }
          }),
        )
      })
  }

  fn render_header(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state = self.state.read(cx);
    let current_year = state.current_year;
    let view_mode = state.view_mode;
    let disable_month_nav = view_mode.is_month();
    let multiple_months = self.number_of_months > 1;

    h_flex()
      .gap_0p5()
      .justify_between()
      .items_center()
      .child(
        Button::new("prev")
          .icon(Icon::new(IconName::ChevronLeft))
          .tab_stop(false)
          .flat()
          .disabled(disable_month_nav)
          .with_size(self.size)
          .when(view_mode.is_day(), |this| {
            this.on_click(window.listener_for(&self.state, CalendarState::prev_month))
          })
          .when(view_mode.is_year(), |this| {
            this
              .when(!state.has_prev_year_page(), |this| this.disabled(true))
              .on_click(window.listener_for(&self.state, CalendarState::prev_year_page))
          }),
      )
      .when(!multiple_months, |this| {
        this.child(
          h_flex()
            .justify_center()
            .gap_3()
            .child(
              Button::new("month")
                .flat()
                .label(state.month_name(0))
                .tab_stop(false)
                .with_size(self.size)
                .selected(view_mode.is_month())
                .on_click(
                  window.listener_for(&self.state, move |state, _, window, cx| {
                    if view_mode.is_month() {
                      state.set_view_mode(ViewMode::Day, window, cx);
                    } else {
                      state.set_view_mode(ViewMode::Month, window, cx);
                    }
                    cx.notify();
                  }),
                ),
            )
            .child(
              Button::new("year")
                .flat()
                .label(current_year.to_string())
                .tab_stop(false)
                .with_size(self.size)
                .selected(view_mode.is_year())
                .on_click(window.listener_for(&self.state, |state, _, window, cx| {
                  if state.view_mode.is_year() {
                    state.set_view_mode(ViewMode::Day, window, cx);
                  } else {
                    state.set_view_mode(ViewMode::Year, window, cx);
                  }
                  cx.notify();
                })),
            ),
        )
      })
      .when(multiple_months, |this| {
        this.child(
          h_flex()
            .flex_1()
            .justify_around()
            .children((0..self.number_of_months).map(|n| {
              h_flex()
                .justify_center()
                .map(|this| match self.size {
                  Size::Small => this.gap_2(),
                  Size::Large => this.gap_4(),
                  _ => this.gap_3(),
                })
                .child(state.month_name(n))
                .child(state.year_name(n))
            })),
        )
      })
      .child(
        Button::new("next")
          .icon(Icon::new(IconName::ChevronRight))
          .flat()
          .tab_stop(false)
          .disabled(disable_month_nav)
          .with_size(self.size)
          .when(view_mode.is_day(), |this| {
            this.on_click(window.listener_for(&self.state, CalendarState::next_month))
          })
          .when(view_mode.is_year(), |this| {
            this
              .when(!state.has_next_year_page(), |this| this.disabled(true))
              .on_click(window.listener_for(&self.state, CalendarState::next_year_page))
          }),
      )
  }

  #[allow(clippy::too_many_arguments)]
  fn item_button(
    &self, id: impl Into<ElementId>, label: impl Into<SharedString>, active: bool,
    secondary_active: bool, muted: bool, disabled: bool, _: &mut Window, cx: &mut App,
  ) -> Stateful<Div> {
    h_flex()
      .id(id.into())
      .map(|this| match self.size {
        Size::Small => this.size_7().rounded(cx.theme().radius / 2.0),
        Size::Large => this.size_10().rounded(cx.theme().radius * 2.0),
        _ => this.size_9().rounded(cx.theme().radius),
      })
      .justify_center()
      .when(muted, |this| {
        this.text_color(if disabled {
          cx.theme().muted_foreground.opacity(0.3)
        } else {
          cx.theme().muted_foreground
        })
      })
      .when(secondary_active, |this| {
        this
          .bg(if muted {
            cx.theme().accent.opacity(0.5)
          } else {
            cx.theme().accent
          })
          .text_color(cx.theme().accent_foreground)
      })
      .when(!active && !disabled, |this| {
        this.hover(|this| {
          this
            .bg(cx.theme().accent)
            .text_color(cx.theme().accent_foreground)
        })
      })
      .when(active, |this| {
        this
          .bg(cx.theme().primary)
          .text_color(cx.theme().primary_foreground)
      })
      .child(label.into())
  }

  fn render_days(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state = self.state.read(cx);
    let week_keys = [
      "calendar.week.sunday",
      "calendar.week.monday",
      "calendar.week.tuesday",
      "calendar.week.wednesday",
      "calendar.week.thursday",
      "calendar.week.friday",
      "calendar.week.saturday",
    ];

    h_flex()
      .map(|this| match self.size {
        Size::Small => this.gap_3().text_sm(),
        Size::Large => this.gap_5().text_base(),
        _ => this.gap_4().text_sm(),
      })
      .justify_between()
      .children(
        state
          .days()
          .chunks(5)
          .enumerate()
          .map(|(offset_month, days)| {
            v_flex()
              .gap_0p5()
              .child(
                h_flex().gap_0p5().justify_between().children(
                  week_keys
                    .iter()
                    .map(|week| self.render_week(translate_woocraft(*week), window, cx)),
                ),
              )
              .children(days.iter().map(|week| {
                h_flex().gap_0p5().justify_between().children(
                  week
                    .iter()
                    .map(|day_date| self.render_day(day_date, offset_month, window, cx)),
                )
              }))
          }),
      )
  }

  fn render_week(&self, week: impl Into<SharedString>, _: &mut Window, cx: &mut App) -> Div {
    h_flex()
      .map(|this| match self.size {
        Size::Small => this.size_7().rounded(cx.theme().radius / 2.0),
        Size::Large => this.size_10().rounded(cx.theme().radius),
        _ => this.size_9().rounded(cx.theme().radius),
      })
      .justify_center()
      .text_color(cx.theme().muted_foreground)
      .text_sm()
      .child(week.into())
  }

  fn render_months(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state = self.state.read(cx);
    let months = state.months();
    let current_month = state.current_month;

    h_flex()
      .mt_3()
      .gap_0p5()
      .gap_y_3()
      .map(|this| match self.size {
        Size::Small => this.mt_2().gap_y_2().w(px(208.0)),
        Size::Large => this.mt_4().gap_y_4().w(px(292.0)),
        _ => this.mt_3().gap_y_3().w(px(264.0)),
      })
      .justify_between()
      .flex_wrap()
      .children(months.iter().enumerate().map(|(ix, month)| {
        let active = (ix + 1) as u8 == current_month;
        self
          .item_button(
            ix,
            month.to_string(),
            active,
            false,
            false,
            false,
            window,
            cx,
          )
          .w(relative(0.3))
          .text_sm()
          .on_click(
            window.listener_for(&self.state, move |state, _, window, cx| {
              state.current_month = (ix + 1) as u8;
              state.set_view_mode(ViewMode::Day, window, cx);
              cx.notify();
            }),
          )
      }))
  }

  fn render_years(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state = self.state.read(cx);
    let current_year = state.current_year;
    let current_page_years = &self.state.read(cx).years[state.year_page as usize].clone();

    h_flex()
      .id("years")
      .gap_0p5()
      .map(|this| match self.size {
        Size::Small => this.mt_2().gap_y_2().w(px(208.0)),
        Size::Large => this.mt_4().gap_y_4().w(px(292.0)),
        _ => this.mt_3().gap_y_3().w(px(264.0)),
      })
      .justify_between()
      .flex_wrap()
      .children(current_page_years.iter().enumerate().map(|(ix, year)| {
        let year = *year;
        let active = year == current_year;
        self
          .item_button(
            ix,
            year.to_string(),
            active,
            false,
            false,
            false,
            window,
            cx,
          )
          .w(relative(0.2))
          .on_click(
            window.listener_for(&self.state, move |state, _, window, cx| {
              state.current_year = year;
              state.set_view_mode(ViewMode::Day, window, cx);
              cx.notify();
            }),
          )
      }))
  }
}

impl_sizable!(Calendar);
impl_styled!(Calendar);

impl EventEmitter<CalendarEvent> for CalendarState {}

impl RenderOnce for Calendar {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let view_mode = self.state.read(cx).view_mode;
    let number_of_months = self.number_of_months;
    self.state.update(cx, |state, _| {
      state.number_of_months = number_of_months;
    });

    v_flex()
      .id(self.id.clone())
      .track_focus(&self.state.read(cx).focus_handle)
      .border_1()
      .border_color(cx.theme().border)
      .rounded(cx.theme().radius_container)
      .p_3()
      .gap_0p5()
      .refine_style(&self.style)
      .child(self.render_header(window, cx))
      .when(view_mode.is_day(), |this| {
        this.child(self.render_days(window, cx))
      })
      .when(view_mode.is_month(), |this| {
        this.child(self.render_months(window, cx))
      })
      .when(view_mode.is_year(), |this| {
        this.child(self.render_years(window, cx))
      })
  }
}
