use std::{ops::Range, rc::Rc};

use gpui::{
  App, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, SharedString,
  StyleRefinement, Styled, Window, prelude::FluentBuilder as _,
};
use rust_i18n::t;

use crate::{
  Button, ButtonVariants as _, Disableable, DropdownMenu as _, Icon, IconName, PopupMenuItem,
  Sizable, Size, StyleSized, StyledExt, Tooltip, h_flex,
};

#[derive(IntoElement)]
pub struct Pagination {
  id: ElementId,
  style: StyleRefinement,
  size: Size,
  current_page: usize,
  total_pages: usize,
  disabled: bool,
  compact: bool,
  visible_pages: usize,
  on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App)>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum PageItem {
  Page(usize),
  Ellipsis(Range<usize>),
}

impl Pagination {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      size: Size::default(),
      current_page: 1,
      total_pages: 1,
      visible_pages: 5,
      disabled: false,
      compact: false,
      on_click: None,
    }
  }

  pub fn current_page(mut self, page: usize) -> Self {
    self.current_page = page.max(1);
    self
  }

  pub fn total_pages(mut self, pages: usize) -> Self {
    self.total_pages = pages.max(1);
    if self.current_page > self.total_pages {
      self.current_page = self.total_pages;
    }
    self
  }

  pub fn on_click(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
    self.on_click = Some(Rc::new(handler));
    self
  }

  pub fn compact(mut self) -> Self {
    self.compact = true;
    self
  }

  pub fn visible_pages(mut self, max: usize) -> Self {
    self.visible_pages = max;
    self
  }

  fn render_nav_button(&self, is_prev: bool, current_page: usize, total_pages: usize) -> Button {
    let (id, label, icon, disabled) = if is_prev {
      (
        "prev",
        SharedString::from(t!("pagination.previous").to_string()),
        IconName::ChevronLeft,
        current_page <= 1,
      )
    } else {
      (
        "next",
        SharedString::from(t!("pagination.next").to_string()),
        IconName::ChevronRight,
        current_page >= total_pages,
      )
    };

    let target_page = if is_prev {
      current_page.saturating_sub(1)
    } else {
      current_page.saturating_add(1).min(total_pages)
    };

    Button::new(id)
      .default()
      .with_size(self.size)
      .disabled(self.disabled || disabled)
      .tooltip({
        let tooltip_label = label.clone();
        move |window, cx| Tooltip::new(tooltip_label.clone()).build(window, cx)
      })
      .when(self.compact, |this| this.icon(icon))
      .when(!self.compact, |this| {
        this
          .when(is_prev, |this| this.flex_row_reverse())
          .child(label.clone())
          .child(Icon::new(icon))
      })
      .when_some(self.on_click.clone(), |this, handler| {
        this.on_click(move |_, window, cx| {
          handler(&target_page, window, cx);
        })
      })
  }
}

impl Disableable for Pagination {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Sizable for Pagination {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Styled for Pagination {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Pagination {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let current_page = self.current_page.max(1).min(self.total_pages.max(1));
    let total_pages = self.total_pages.max(1);

    let page_numbers = if !self.compact {
      calculate_page_range(current_page, total_pages, self.visible_pages)
    } else {
      vec![]
    };

    let is_disabled = self.disabled;
    let on_click = self.on_click.clone();
    let size = self.size;

    h_flex()
      .id(self.id.clone())
      .container_size(size)
      .gap_2()
      .items_center()
      .refine_style(&self.style)
      .child(self.render_nav_button(true, current_page, total_pages))
      .children({
        page_numbers.into_iter().map(move |item| match item {
          PageItem::Page(page) => {
            let is_selected = page == current_page;

            Button::new(("pagination-page", page))
              .with_size(size)
              .map(|this| {
                if is_selected {
                  this.primary()
                } else {
                  this.default()
                }
              })
              .label(page.to_string())
              .disabled(is_disabled)
              .when(!is_selected, |this| {
                this.when_some(on_click.clone(), |this, handler| {
                  this.on_click(move |_, window, cx| {
                    handler(&page, window, cx);
                  })
                })
              })
              .into_any_element()
          }
          PageItem::Ellipsis(range) => {
            let ellipsis_id = SharedString::from(format!("ellipsis-{}-{}", range.start, range.end));
            let on_click_for_popover = on_click.clone();

            Button::new((ellipsis_id.clone(), 0usize))
              .flat()
              .with_size(size)
              .disabled(is_disabled)
              .icon(IconName::MoreHorizontal)
              .dropdown_menu(move |mut menu, _, _| {
                let on_click_for_page = on_click_for_popover.clone();

                for page in range.clone() {
                  let is_selected = page == current_page;
                  let mut item = PopupMenuItem::new(page.to_string())
                    .checked(is_selected)
                    .disabled(is_disabled || is_selected);

                  if !is_selected {
                    if let Some(handler) = on_click_for_page.clone() {
                      item = item.on_click(move |_, window, cx| {
                        handler(&page, window, cx);
                      });
                    }
                  }
                  menu = menu.item(item);
                }
                menu
              })
              .into_any_element()
          }
        })
      })
      .child(self.render_nav_button(false, current_page, total_pages))
  }
}

fn calculate_page_range(current: usize, total: usize, max_visible: usize) -> Vec<PageItem> {
  if total <= 1 {
    return vec![];
  }

  let max_visible = max_visible.max(5);

  if total <= max_visible {
    return (1..=total).map(PageItem::Page).collect();
  }

  let mut pages = vec![];
  let side_pages = (max_visible - 3) / 2;

  pages.push(PageItem::Page(1));

  let start = if current <= side_pages + 1 {
    2
  } else if current > total - side_pages - 1 {
    total - side_pages - 1
  } else {
    current - side_pages
  };

  if start > 2 {
    pages.push(PageItem::Ellipsis(2..start));
  }

  let end = if current >= total - side_pages {
    total - 1
  } else if current <= side_pages + 1 {
    side_pages + 2
  } else {
    current + side_pages
  };

  for page in start..=end {
    pages.push(PageItem::Page(page));
  }

  if end < total - 1 {
    pages.push(PageItem::Ellipsis(end + 1..total));
  }

  pages.push(PageItem::Page(total));

  pages
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_calculate_page_range() {
    use super::{PageItem, calculate_page_range};

    let result = calculate_page_range(1, 10, 7);
    let expected = vec![
      PageItem::Page(1),
      PageItem::Page(2),
      PageItem::Page(3),
      PageItem::Page(4),
      PageItem::Ellipsis(5..10),
      PageItem::Page(10),
    ];
    assert_eq!(result, expected);

    let result = calculate_page_range(5, 10, 7);
    let expected = vec![
      PageItem::Page(1),
      PageItem::Ellipsis(2..3),
      PageItem::Page(3),
      PageItem::Page(4),
      PageItem::Page(5),
      PageItem::Page(6),
      PageItem::Page(7),
      PageItem::Ellipsis(8..10),
      PageItem::Page(10),
    ];
    assert_eq!(result, expected);

    let result = calculate_page_range(10, 10, 7);
    let expected = vec![
      PageItem::Page(1),
      PageItem::Ellipsis(2..7),
      PageItem::Page(7),
      PageItem::Page(8),
      PageItem::Page(9),
      PageItem::Page(10),
    ];
    assert_eq!(result, expected);
  }
}
