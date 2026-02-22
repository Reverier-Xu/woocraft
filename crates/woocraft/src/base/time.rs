use chrono::{Datelike, Duration, Local, NaiveDate};
use gpui::SharedString;

#[cfg(test)]
trait NaiveDateExt {
  fn days_in_month(&self) -> i32;
  fn is_leap_year(&self) -> bool;
}

#[cfg(test)]
impl NaiveDateExt for NaiveDate {
  fn days_in_month(&self) -> i32 {
    match self.month() {
      1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
      4 | 6 | 9 | 11 => 30,
      2 => {
        if self.is_leap_year() {
          29
        } else {
          28
        }
      }
      month => panic!("invalid month: {month}"),
    }
  }

  fn is_leap_year(&self) -> bool {
    let year = self.year();
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
  }
}

/// Selected date value for calendar-like widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Date {
  Single(Option<NaiveDate>),
  Range(Option<NaiveDate>, Option<NaiveDate>),
}

impl std::fmt::Display for Date {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Single(Some(date)) => write!(f, "{date}"),
      Self::Single(None) => write!(f, "nil"),
      Self::Range(Some(start), Some(end)) => write!(f, "{start} - {end}"),
      Self::Range(None, None) => write!(f, "nil"),
      Self::Range(Some(start), None) => write!(f, "{start} - nil"),
      Self::Range(None, Some(end)) => write!(f, "nil - {end}"),
    }
  }
}

impl From<NaiveDate> for Date {
  fn from(date: NaiveDate) -> Self {
    Self::Single(Some(date))
  }
}

impl From<(NaiveDate, NaiveDate)> for Date {
  fn from((start, end): (NaiveDate, NaiveDate)) -> Self {
    Self::Range(Some(start), Some(end))
  }
}

impl Date {
  pub fn is_some(&self) -> bool {
    matches!(self, Self::Single(Some(_)) | Self::Range(Some(_), _))
  }

  pub fn is_complete(&self) -> bool {
    matches!(self, Self::Single(Some(_)) | Self::Range(Some(_), Some(_)))
  }

  pub fn start(&self) -> Option<NaiveDate> {
    match self {
      Self::Single(Some(date)) => Some(*date),
      Self::Range(Some(start), _) => Some(*start),
      _ => None,
    }
  }

  pub fn end(&self) -> Option<NaiveDate> {
    match self {
      Self::Range(_, Some(end)) => Some(*end),
      _ => None,
    }
  }

  pub fn format(&self, format: &str) -> Option<SharedString> {
    match self {
      Self::Single(Some(date)) => Some(date.format(format).to_string().into()),
      Self::Range(Some(start), Some(end)) => {
        Some(format!("{} - {}", start.format(format), end.format(format)).into())
      }
      _ => None,
    }
  }

  pub fn is_single(&self) -> bool {
    matches!(self, Self::Single(_))
  }

  pub fn is_active(&self, value: &NaiveDate) -> bool {
    let value = *value;
    match self {
      Self::Single(date) => Some(value) == *date,
      Self::Range(start, end) => Some(value) == *start || Some(value) == *end,
    }
  }

  pub fn is_in_range(&self, value: &NaiveDate) -> bool {
    let value = *value;
    match self {
      Self::Range(Some(start), Some(end)) => value >= *start && value <= *end,
      _ => false,
    }
  }
}

/// Matcher to match dates before and after the interval.
pub struct IntervalMatcher {
  before: Option<NaiveDate>,
  after: Option<NaiveDate>,
}

/// Matcher to match dates within the range.
pub struct RangeMatcher {
  from: Option<NaiveDate>,
  to: Option<NaiveDate>,
}

/// Matcher to match dates.
pub enum Matcher {
  /// Match weekdays by index from Sunday (0..=6).
  DayOfWeek(Vec<u32>),
  /// Match values outside of `[before, after]`.
  Interval(IntervalMatcher),
  /// Match values inside of `[from, to]`.
  Range(RangeMatcher),
  /// Match values via custom predicate.
  Custom(Box<dyn Fn(&NaiveDate) -> bool + Send + Sync>),
}

impl From<Vec<u32>> for Matcher {
  fn from(days: Vec<u32>) -> Self {
    Self::DayOfWeek(days)
  }
}

impl<F> From<F> for Matcher
where
  F: Fn(&NaiveDate) -> bool + Send + Sync + 'static,
{
  fn from(f: F) -> Self {
    Self::Custom(Box::new(f))
  }
}

impl Matcher {
  pub fn interval(before: Option<NaiveDate>, after: Option<NaiveDate>) -> Self {
    Self::Interval(IntervalMatcher { before, after })
  }

  pub fn range(from: Option<NaiveDate>, to: Option<NaiveDate>) -> Self {
    Self::Range(RangeMatcher { from, to })
  }

  pub fn custom<F>(f: F) -> Self
  where
    F: Fn(&NaiveDate) -> bool + Send + Sync + 'static, {
    Self::Custom(Box::new(f))
  }

  pub fn is_match(&self, date: &Date) -> bool {
    match date {
      Date::Single(Some(date)) => self.matched(date),
      Date::Range(Some(start), Some(end)) => self.matched(start) || self.matched(end),
      _ => false,
    }
  }

  pub fn matched(&self, date: &NaiveDate) -> bool {
    match self {
      Self::DayOfWeek(days) => days.contains(&date.weekday().num_days_from_sunday()),
      Self::Interval(interval) => {
        let before_check = interval.before.is_some_and(|before| date < &before);
        let after_check = interval.after.is_some_and(|after| date > &after);
        before_check || after_check
      }
      Self::Range(range) => {
        let from_check = range.from.is_some_and(|from| date < &from);
        let to_check = range.to.is_some_and(|to| date > &to);
        !from_check && !to_check
      }
      Self::Custom(f) => f(date),
    }
  }
}

pub fn local_today() -> NaiveDate {
  Local::now().date_naive()
}

/// Build a month matrix for calendar display, including adjacent month days.
///
/// Returns a fixed 5-week grid with 7 days each.
pub fn month_days(year: i32, month: u32) -> Vec<Vec<NaiveDate>> {
  let mut year = year;
  let mut month = month as i32;

  while month > 12 {
    month -= 12;
    year += 1;
  }
  while month < 1 {
    month += 12;
    year -= 1;
  }

  let month = month as u32;
  let date = NaiveDate::from_ymd_opt(year, month, 1).expect("invalid month start");
  let start_weekday = date.weekday().num_days_from_sunday();

  let mut days = Vec::with_capacity(5);
  for week in 0..5 {
    let mut week_days = Vec::with_capacity(7);
    for weekday in 0..7 {
      let day = week * 7 + weekday - start_weekday as i32;
      let current = date
        .checked_add_signed(Duration::days(day as i64))
        .expect("invalid shifted day in calendar grid");
      week_days.push(current);
    }
    days.push(week_days);
  }

  days
}

#[cfg(test)]
mod tests {
  use chrono::{Datelike, NaiveDate};

  use super::{Date, NaiveDateExt, month_days};

  #[test]
  fn test_date_to_string() {
    let date = Date::Single(Some(NaiveDate::from_ymd_opt(2024, 8, 3).unwrap()));
    assert_eq!(date.to_string(), "2024-08-03");

    let date = Date::Single(None);
    assert_eq!(date.to_string(), "nil");

    let date = Date::Range(
      Some(NaiveDate::from_ymd_opt(2024, 8, 3).unwrap()),
      Some(NaiveDate::from_ymd_opt(2024, 8, 5).unwrap()),
    );
    assert_eq!(date.to_string(), "2024-08-03 - 2024-08-05");

    let date = Date::Range(Some(NaiveDate::from_ymd_opt(2024, 8, 3).unwrap()), None);
    assert_eq!(date.to_string(), "2024-08-03 - nil");

    let date = Date::Range(None, Some(NaiveDate::from_ymd_opt(2024, 8, 5).unwrap()));
    assert_eq!(date.to_string(), "nil - 2024-08-05");

    let date = Date::Range(None, None);
    assert_eq!(date.to_string(), "nil");
  }

  #[test]
  fn test_days_in_month() {
    assert_eq!(
      NaiveDate::from_ymd_opt(2024, 2, 1).unwrap().days_in_month(),
      29
    );
    assert_eq!(
      NaiveDate::from_ymd_opt(2023, 2, 1).unwrap().days_in_month(),
      28
    );
    assert_eq!(
      NaiveDate::from_ymd_opt(2023, 1, 1).unwrap().days_in_month(),
      31
    );
    assert_eq!(
      NaiveDate::from_ymd_opt(2023, 4, 1).unwrap().days_in_month(),
      30
    );
  }

  #[test]
  fn test_month_days_grid() {
    #[track_caller]
    fn assert_case(date: NaiveDate, expected: Vec<&str>) {
      let out = month_days(date.year(), date.month())
        .iter()
        .map(|week| {
          week
            .iter()
            .map(|d| {
              if d.year() == date.year() && d.month() == date.month() {
                format!("{:2}", d.day())
              } else if d.year() == date.year() {
                format!("{}-{}", d.month(), d.day())
              } else {
                format!("{}-{}-{}", d.year(), d.month(), d.day())
              }
            })
            .collect::<Vec<_>>()
            .join("|")
        })
        .collect::<Vec<_>>();

      assert_eq!(out, expected);
    }

    assert_case(
      NaiveDate::from_ymd_opt(2024, 8, 1).unwrap(),
      vec![
        "7-28|7-29|7-30|7-31| 1| 2| 3",
        " 4| 5| 6| 7| 8| 9|10",
        "11|12|13|14|15|16|17",
        "18|19|20|21|22|23|24",
        "25|26|27|28|29|30|31",
      ],
    );
    assert_case(
      NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
      vec![
        "2024-12-29|2024-12-30|2024-12-31| 1| 2| 3| 4",
        " 5| 6| 7| 8| 9|10|11",
        "12|13|14|15|16|17|18",
        "19|20|21|22|23|24|25",
        "26|27|28|29|30|31|2-1",
      ],
    );
    assert_case(
      NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
      vec![
        "1-28|1-29|1-30|1-31| 1| 2| 3",
        " 4| 5| 6| 7| 8| 9|10",
        "11|12|13|14|15|16|17",
        "18|19|20|21|22|23|24",
        "25|26|27|28|29|3-1|3-2",
      ],
    );
    assert_case(
      NaiveDate::from_ymd_opt(2023, 2, 20).unwrap(),
      vec![
        "1-29|1-30|1-31| 1| 2| 3| 4",
        " 5| 6| 7| 8| 9|10|11",
        "12|13|14|15|16|17|18",
        "19|20|21|22|23|24|25",
        "26|27|28|3-1|3-2|3-3|3-4",
      ],
    );
  }
}
