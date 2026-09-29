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
//! The day grids for the visible month window are cached on `CalendarState`
//! and rebuilt only when the visible year, month, or month count changes.
//! Month name labels are translated once per locale and reused across
//! renders. The disabled matcher is still evaluated per day cell, so keep
//! matcher predicates fast.

use std::{rc::Rc, sync::Mutex};

use chrono::{Datelike, NaiveDate};
use gpui::{
  App, ClickEvent, Context, Div, ElementId, Empty, Entity, EventEmitter, FocusHandle,
  InteractiveElement, IntoElement, ParentElement, Render, RenderOnce, SharedString, Stateful,
  StatefulInteractiveElement, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
  relative, rems,
};

use crate::{
  ActiveTheme, Button, ButtonVariants as _, Date, Disableable as _, Icon, IconName, Matcher,
  Selectable, Sizable, Size, StyledExt as _, h_flex, local_today, month_days, translate_woocraft,
  v_flex,
};

/// i18n keys of the twelve month names, January first.
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

/// Single-entry per-locale cache for render-path translations.
///
/// `translate_woocraft` normalizes the locale and allocates a fresh `String`
/// on every call, which adds up on widgets that translate labels every frame
/// (calendar and date picker). Cached values are rebuilt automatically when
/// the active locale changes.
pub(crate) struct LocaleCache<T: Clone> {
  cached: Mutex<Option<(String, T)>>,
}

impl<T: Clone> LocaleCache<T> {
  pub(crate) const fn new() -> Self {
    Self {
      cached: Mutex::new(None),
    }
  }

  /// Returns the cached value for the active locale, rebuilding it via
  /// `build` when the locale changed or nothing was cached yet.
  pub(crate) fn get(&self, build: impl FnOnce() -> T) -> T {
    let locale = crate::locale();
    let locale: &str = &locale;
    let mut cached = self
      .cached
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    match cached.as_ref() {
      Some((cached_locale, value)) if cached_locale.as_str() == locale => value.clone(),
      _ => {
        let value = build();
        *cached = Some((locale.to_string(), value.clone()));
        value
      }
    }
  }
}

static MONTH_NAMES: LocaleCache<[SharedString; 12]> = LocaleCache::new();

/// Translated month names for the active locale, cached per locale.
fn month_names() -> [SharedString; 12] {
  MONTH_NAMES.get(|| MONTH_KEYS.map(|key| SharedString::from(translate_woocraft(key))))
}

/// Cached month day matrices for the calendar day grid.
///
/// `key` is the `(current_year, current_month, number_of_months)` window the
/// `months` grids were built for. [`CalendarState::sync_days_cache`] rebuilds
/// `months` whenever the key differs, which covers month navigation,
/// `set_date`, and month-count changes without dedicated invalidation call
/// sites.
struct DaysCache {
  key: (i32, u8, usize),
  months: Vec<Vec<Vec<NaiveDate>>>,
}

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
  /// Day grids for the currently visible month window.
  days_cache: DaysCache,
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
      days_cache: DaysCache {
        key: (today.year(), today.month() as u8, 1),
        months: vec![month_days(today.year(), today.month())],
      },
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

  /// Returns the cached month day matrices, one grid per visible month.
  fn days(&self) -> &[Vec<Vec<NaiveDate>>] {
    &self.days_cache.months
  }

  /// Rebuilds the cached day grids when the visible month window changed.
  ///
  /// Called from `Calendar::render` before the day grid is rendered; the key
  /// comparison picks up every mutation of `current_year`/`current_month`/
  /// `number_of_months`, including direct field writes from click handlers.
  /// Never notifies: this is pure cache maintenance.
  fn sync_days_cache(&mut self) {
    let key = (self.current_year, self.current_month, self.number_of_months);
    if self.days_cache.key == key {
      return;
    }

    self.days_cache.months = (0..self.number_of_months)
      .map(|offset| {
        let (year, month) = self.offset_year_month(offset);
        month_days(year, month)
      })
      .collect();
    self.days_cache.key = key;
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
    let (_, month) = self.offset_year_month(offset_month);
    month_names()[(month.saturating_sub(1)) as usize].clone()
  }

  fn year_name(&self, offset_month: usize) -> SharedString {
    let (year, _) = self.offset_year_month(offset_month);
    year.to_string().into()
  }

  fn set_view_mode(&mut self, mode: ViewMode, _: &mut Window, cx: &mut Context<Self>) {
    self.view_mode = mode;
    cx.notify();
  }

  fn months(&self) -> [SharedString; 12] {
    month_names()
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
    &self, day_date: &NaiveDate, offset_month: usize, month: u32, window: &mut Window, cx: &App,
  ) -> Stateful<Div> {
    let state = self.state.read(cx);
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
        // Today marker: a small primary dot under the day number. Absolutely
        // positioned so it does not shift the cell layout.
        this.child(
          div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(rems(0.25))
            .flex()
            .justify_center()
            .child(
              div()
                .size(rems(0.125))
                .rounded_full()
                .bg(cx.theme().primary),
            ),
        )
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

  fn render_header(&self, window: &mut Window, cx: &App) -> impl IntoElement {
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
            // Same tier-relative gap (0.75em) as the multi-month header so
            // both views space the month/year cluster identically.
            .gap(self.size.em(0.75))
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
                // Gap between the month and year names tracks the size tier
                // (0.75em) instead of hard-coded rem steps.
                .gap(self.size.em(0.75))
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
    secondary_active: bool, muted: bool, disabled: bool, _: &mut Window, cx: &App,
  ) -> Stateful<Div> {
    h_flex()
      .id(id.into())
      // Day/month/year cell edge is 2.25em of the size tier (2.25rem at
      // Medium) so the cell grid scales with the calendar's size instead of
      // hard-coded rem steps.
      .size(self.size.em(2.25))
      .map(|this| match self.size {
        Size::Small => this.rounded(cx.theme().radius / 2.0),
        Size::Large => this.rounded(cx.theme().radius * 2.0),
        _ => this.rounded(cx.theme().radius),
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
        // In-range days reuse the primary hue at low alpha so the range reads
        // as one continuous band with the selected endpoints.
        this
          .bg(if muted {
            cx.theme().primary.opacity(0.12)
          } else {
            cx.theme().primary.opacity(0.25)
          })
          .text_color(if muted {
            cx.theme().muted_foreground
          } else {
            cx.theme().foreground
          })
      })
      .when(!active && !disabled, |this| {
        // Hover stays on the primary hue like the range band, just fainter
        // than the in-range fill so the two states remain distinguishable.
        this.hover(|this| {
          if secondary_active {
            this
              .bg(cx.theme().primary.opacity(0.4))
              .text_color(cx.theme().foreground)
          } else {
            this
              .bg(cx.theme().primary.opacity(0.15))
              .text_color(cx.theme().foreground)
          }
        })
      })
      .when(active, |this| {
        this
          .bg(cx.theme().primary)
          .text_color(cx.theme().primary_foreground)
      })
      .child(label.into())
  }

  fn render_days(&self, window: &mut Window, cx: &App) -> impl IntoElement {
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
        Size::Large => this.text_base(),
        _ => this.text_sm(),
      })
      // Gap between the month grids equals the tier text size (0.75/1/1.25rem),
      // the same values the previous hard-coded gap_3/gap_4/gap_5 steps used.
      .gap(self.size.text_size())
      .justify_between()
      .children(state.days().iter().enumerate().map(|(offset_month, days)| {
        // The month number only depends on the offset; resolve it once per
        // visible month instead of once per day cell.
        let (_, month) = state.offset_year_month(offset_month);
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
                .map(|day_date| self.render_day(day_date, offset_month, month, window, cx)),
            )
          }))
      }))
  }

  fn render_week(&self, week: impl Into<SharedString>, _: &mut Window, cx: &App) -> Div {
    h_flex()
      // Same tier-relative cell edge (2.25em) as the day cells so the
      // weekday header row aligns with the day grid.
      .size(self.size.em(2.25))
      .rounded(cx.theme().radius)
      .justify_center()
      .text_color(cx.theme().muted_foreground)
      .text_sm()
      .child(week.into())
  }

  fn render_months(&self, window: &mut Window, cx: &App) -> impl IntoElement {
    let state = self.state.read(cx);
    let months = state.months();
    let current_month = state.current_month;

    h_flex()
      .gap_0p5()
      // Row gap and top margin track the size tier (0.75em; the previous
      // hard-coded steps were 0.5/0.75/1rem), and the grid width is 17em of
      // the tier (13/17/18rem before) so it stays in step with the day grid.
      .mt(self.size.em(0.75))
      .gap_y(self.size.em(0.75))
      .w(self.size.em(17.))
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

  fn render_years(&self, window: &mut Window, cx: &App) -> impl IntoElement {
    let state = self.state.read(cx);
    let current_year = state.current_year;
    let current_page_years = &state.years[state.year_page as usize];

    h_flex()
      .id("years")
      .gap_0p5()
      // Same tier-relative metrics as the months grid (0.75em row gap and
      // margin, 17em width).
      .mt(self.size.em(0.75))
      .gap_y(self.size.em(0.75))
      .w(self.size.em(17.))
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
      // Write back only on change to avoid a redundant state update every
      // frame, then refresh the cached day grids for the new month window.
      if state.number_of_months != number_of_months {
        state.number_of_months = number_of_months;
      }
      state.sync_days_cache();
    });

    v_flex()
      .id(self.id.clone())
      .track_focus(&self.state.read(cx).focus_handle)
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
