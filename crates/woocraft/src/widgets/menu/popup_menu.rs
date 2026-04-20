use std::rc::Rc;

use gpuim::{
  Action, AnchoredPositionMode, AnyElement, App, AppContext, Bounds, ClickEvent, Context, Corner,
  DismissEvent, Edges, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
  IntoElement, KeyBinding, MouseDownEvent, OwnedMenuItem, ParentElement, Pixels, Point, Render,
  ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window,
  anchored, div, prelude::FluentBuilder, px, rems,
};

use crate::{
  ActiveTheme, CardStyle, Disableable, Divider, ElementExt, Icon, IconLabel, IconName, Kbd,
  ScrollableElement, Selectable, Size, StyleSized,
  actions::{Cancel, Confirm, SelectDown, SelectLeft, SelectRight, SelectUp},
  h_flex, v_flex,
  widgets::{Button, ButtonVariants},
};

const CONTEXT: &str = "PopupMenu";

type ClickHandler = dyn Fn(&ClickEvent, &mut Window, &mut App);
type ElementRenderer = dyn Fn(&mut Window, &mut App) -> AnyElement;

#[derive(Clone, Copy)]
pub enum Side {
  Left,
  Right,
}

impl Side {
  fn is_left(&self) -> bool {
    matches!(self, Self::Left)
  }

  fn is_right(&self) -> bool {
    matches!(self, Self::Right)
  }
}

pub fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("enter", Confirm { secondary: false }, Some(CONTEXT)),
    KeyBinding::new("escape", Cancel, Some(CONTEXT)),
    KeyBinding::new("up", SelectUp, Some(CONTEXT)),
    KeyBinding::new("down", SelectDown, Some(CONTEXT)),
    KeyBinding::new("left", SelectLeft, Some(CONTEXT)),
    KeyBinding::new("right", SelectRight, Some(CONTEXT)),
  ]);
}

/// An menu item in a popup menu.
pub enum PopupMenuItem {
  /// A menu separator item.
  Separator,
  /// A non-interactive label item.
  Label(SharedString),
  /// A standard menu item.
  Item {
    icon: Option<Icon>,
    label: SharedString,
    disabled: bool,
    checked: bool,
    is_link: bool,
    action: Option<Box<dyn Action>>,
    // For link item
    handler: Option<Rc<ClickHandler>>,
  },
  /// A menu item with custom element render.
  ElementItem {
    icon: Option<Icon>,
    disabled: bool,
    checked: bool,
    action: Option<Box<dyn Action>>,
    render: Box<ElementRenderer>,
    handler: Option<Rc<ClickHandler>>,
  },
  /// A submenu item that opens another popup menu.
  ///
  /// NOTE: This is only supported when the parent menu is not `scrollable`.
  Submenu {
    icon: Option<Icon>,
    label: SharedString,
    disabled: bool,
    menu: Entity<PopupMenu>,
  },
}

impl FluentBuilder for PopupMenuItem {}
impl PopupMenuItem {
  /// Create a new menu item with the given label.
  #[inline]
  pub fn new(label: impl Into<SharedString>) -> Self {
    PopupMenuItem::Item {
      icon: None,
      label: label.into(),
      disabled: false,
      checked: false,
      action: None,
      is_link: false,
      handler: None,
    }
  }

  /// Create a new menu item with custom element render.
  #[inline]
  pub fn element<F, E>(builder: F) -> Self
  where
    F: Fn(&mut Window, &mut App) -> E + 'static,
    E: IntoElement, {
    PopupMenuItem::ElementItem {
      icon: None,
      disabled: false,
      checked: false,
      action: None,
      render: Box::new(move |window, cx| builder(window, cx).into_any_element()),
      handler: None,
    }
  }

  /// Create a new submenu item that opens another popup menu.
  #[inline]
  pub fn submenu(label: impl Into<SharedString>, menu: Entity<PopupMenu>) -> Self {
    PopupMenuItem::Submenu {
      icon: None,
      label: label.into(),
      disabled: false,
      menu,
    }
  }

  /// Create a separator menu item.
  #[inline]
  pub fn separator() -> Self {
    PopupMenuItem::Separator
  }

  /// Creates a label menu item.
  #[inline]
  pub fn label(label: impl Into<SharedString>) -> Self {
    PopupMenuItem::Label(label.into())
  }

  /// Set the icon for the menu item.
  ///
  /// Only works for [`PopupMenuItem::Item`], [`PopupMenuItem::ElementItem`] and
  /// [`PopupMenuItem::Submenu`].
  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    match &mut self {
      PopupMenuItem::Item { icon: i, .. } => {
        *i = Some(icon.into());
      }
      PopupMenuItem::ElementItem { icon: i, .. } => {
        *i = Some(icon.into());
      }
      PopupMenuItem::Submenu { icon: i, .. } => {
        *i = Some(icon.into());
      }
      _ => {}
    }
    self
  }

  /// Set the action for the menu item.
  ///
  /// Only works for [`PopupMenuItem::Item`] and [`PopupMenuItem::ElementItem`].
  pub fn action(mut self, action: Box<dyn Action>) -> Self {
    match &mut self {
      PopupMenuItem::Item { action: a, .. } => {
        *a = Some(action);
      }
      PopupMenuItem::ElementItem { action: a, .. } => {
        *a = Some(action);
      }
      _ => {}
    }
    self
  }

  /// Set the disabled state for the menu item.
  ///
  /// Only works for [`PopupMenuItem::Item`], [`PopupMenuItem::ElementItem`] and
  /// [`PopupMenuItem::Submenu`].
  pub fn disabled(mut self, disabled: bool) -> Self {
    match &mut self {
      PopupMenuItem::Item { disabled: d, .. } => {
        *d = disabled;
      }
      PopupMenuItem::ElementItem { disabled: d, .. } => {
        *d = disabled;
      }
      PopupMenuItem::Submenu { disabled: d, .. } => {
        *d = disabled;
      }
      _ => {}
    }
    self
  }

  /// Set checked state for the menu item.
  ///
  /// NOTE: If `check_side` is [`Side::Left`], the icon will replace with a
  /// check icon.
  pub fn checked(mut self, checked: bool) -> Self {
    match &mut self {
      PopupMenuItem::Item { checked: c, .. } => {
        *c = checked;
      }
      PopupMenuItem::ElementItem { checked: c, .. } => {
        *c = checked;
      }
      _ => {}
    }
    self
  }

  /// Add a click handler for the menu item.
  ///
  /// Only works for [`PopupMenuItem::Item`] and [`PopupMenuItem::ElementItem`].
  pub fn on_click<F>(mut self, handler: F) -> Self
  where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static, {
    match &mut self {
      PopupMenuItem::Item { handler: h, .. } => {
        *h = Some(Rc::new(handler));
      }
      PopupMenuItem::ElementItem { handler: h, .. } => {
        *h = Some(Rc::new(handler));
      }
      _ => {}
    }
    self
  }

  /// Create a link menu item.
  #[inline]
  pub fn link(label: impl Into<SharedString>, href: impl Into<String>) -> Self {
    let href = href.into();
    PopupMenuItem::Item {
      icon: None,
      label: label.into(),
      disabled: false,
      checked: false,
      action: None,
      is_link: true,
      handler: Some(Rc::new(move |_, _, cx| cx.open_url(&href))),
    }
  }

  #[inline]
  fn is_clickable(&self) -> bool {
    !matches!(self, PopupMenuItem::Separator)
      && matches!(
        self,
        PopupMenuItem::Item {
          disabled: false,
          ..
        } | PopupMenuItem::ElementItem {
          disabled: false,
          ..
        } | PopupMenuItem::Submenu {
          disabled: false,
          ..
        }
      )
  }

  #[inline]
  fn is_separator(&self) -> bool {
    matches!(self, PopupMenuItem::Separator)
  }

  fn has_left_icon(&self, check_side: Side) -> bool {
    match self {
      PopupMenuItem::Item { icon, checked, .. } => {
        icon.is_some() || (check_side.is_left() && *checked)
      }
      PopupMenuItem::ElementItem { icon, checked, .. } => {
        icon.is_some() || (check_side.is_left() && *checked)
      }
      PopupMenuItem::Submenu { icon, .. } => icon.is_some(),
      _ => false,
    }
  }

  #[inline]
  fn is_checked(&self) -> bool {
    match self {
      PopupMenuItem::Item { checked, .. } => *checked,
      PopupMenuItem::ElementItem { checked, .. } => *checked,
      _ => false,
    }
  }
}

pub struct PopupMenu {
  pub(crate) focus_handle: FocusHandle,
  pub(crate) menu_items: Vec<PopupMenuItem>,
  /// The focus handle of Entity to handle actions.
  pub(crate) action_context: Option<FocusHandle>,
  selected_index: Option<usize>,
  min_width: Option<Pixels>,
  max_width: Option<Pixels>,
  max_height: Option<Pixels>,
  bounds: Bounds<Pixels>,
  size: Size,
  check_side: Side,
  self_entity: WeakEntity<Self>,

  /// The parent menu of this menu, if this is a submenu
  parent_menu: Option<WeakEntity<Self>>,
  scrollable: bool,
  external_link_icon: bool,
  scroll_handle: ScrollHandle,
  // This will update on render
  submenu_anchor: (Corner, Point<Pixels>),
  // Item bounds in window coordinates, updated on prepaint.
  item_bounds: Vec<Bounds<Pixels>>,

  _subscriptions: Vec<Subscription>,
}

impl PopupMenu {
  pub(crate) fn new(cx: &mut Context<Self>) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      action_context: None,
      self_entity: cx.entity().downgrade(),
      parent_menu: None,
      menu_items: Vec::new(),
      selected_index: None,
      min_width: None,
      max_width: None,
      max_height: None,
      check_side: Side::Left,
      bounds: Bounds::default(),
      scrollable: false,
      scroll_handle: ScrollHandle::default(),
      external_link_icon: true,
      size: Size::default(),
      submenu_anchor: (Corner::TopLeft, Point::default()),
      item_bounds: vec![],
      _subscriptions: vec![],
    }
  }

  pub fn build(
    window: &mut Window, cx: &mut App,
    f: impl FnOnce(Self, &mut Window, &mut Context<PopupMenu>) -> Self,
  ) -> Entity<Self> {
    cx.new(|cx| f(Self::new(cx), window, cx))
  }

  /// Set the focus handle of Entity to handle actions.
  ///
  /// When the menu is dismissed or before an action is triggered, the focus
  /// will be returned to this handle.
  ///
  /// Then the action will be dispatched to this handle.
  pub fn action_context(mut self, handle: FocusHandle) -> Self {
    self.action_context = Some(handle);
    self
  }

  /// Set min width of the popup menu, default is 120px
  pub fn min_w(mut self, width: impl Into<Pixels>) -> Self {
    self.min_width = Some(width.into());
    self
  }

  /// Set max width of the popup menu, default is 500px
  pub fn max_w(mut self, width: impl Into<Pixels>) -> Self {
    self.max_width = Some(width.into());
    self
  }

  /// Set max height of the popup menu, default is half of the window height
  pub fn max_h(mut self, height: impl Into<Pixels>) -> Self {
    self.max_height = Some(height.into());
    self
  }

  /// Set the menu to be scrollable to show vertical scrollbar.
  ///
  /// NOTE: If this is true, the sub-menus will cannot be support.
  pub fn scrollable(mut self, scrollable: bool) -> Self {
    self.scrollable = scrollable;
    self
  }

  /// Set the side to show check icon, default is `Side::Left`.
  pub fn check_side(mut self, side: Side) -> Self {
    self.check_side = side;
    self
  }

  /// Set the menu to show external link icon, default is true.
  pub fn external_link_icon(mut self, visible: bool) -> Self {
    self.external_link_icon = visible;
    self
  }

  /// Add Menu Item
  pub fn menu(self, label: impl Into<SharedString>, action: Box<dyn Action>) -> Self {
    self.menu_with_disabled(label, action, false)
  }

  /// Add Menu Item with enable state
  pub fn menu_with_enable(
    mut self, label: impl Into<SharedString>, action: Box<dyn Action>, enable: bool,
  ) -> Self {
    self.add_menu_item(label, None, action, !enable, false);
    self
  }

  /// Add Menu Item with disabled state
  pub fn menu_with_disabled(
    mut self, label: impl Into<SharedString>, action: Box<dyn Action>, disabled: bool,
  ) -> Self {
    self.add_menu_item(label, None, action, disabled, false);
    self
  }

  /// Add label
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.menu_items.push(PopupMenuItem::label(label.into()));
    self
  }

  /// Add Menu to open link
  pub fn link(self, label: impl Into<SharedString>, href: impl Into<String>) -> Self {
    self.link_with_disabled(label, href, false)
  }

  /// Add Menu to open link with disabled state
  pub fn link_with_disabled(
    mut self, label: impl Into<SharedString>, href: impl Into<String>, disabled: bool,
  ) -> Self {
    let href = href.into();
    self
      .menu_items
      .push(PopupMenuItem::link(label, href).disabled(disabled));
    self
  }

  /// Add Menu to open link
  pub fn link_with_icon(
    self, label: impl Into<SharedString>, icon: impl Into<Icon>, href: impl Into<String>,
  ) -> Self {
    self.link_with_icon_and_disabled(label, icon, href, false)
  }

  /// Add Menu to open link with icon and disabled state
  fn link_with_icon_and_disabled(
    mut self, label: impl Into<SharedString>, icon: impl Into<Icon>, href: impl Into<String>,
    disabled: bool,
  ) -> Self {
    let href = href.into();
    self.menu_items.push(
      PopupMenuItem::link(label, href)
        .icon(icon)
        .disabled(disabled),
    );
    self
  }

  /// Add Menu Item with Icon.
  pub fn menu_with_icon(
    self, label: impl Into<SharedString>, icon: impl Into<Icon>, action: Box<dyn Action>,
  ) -> Self {
    self.menu_with_icon_and_disabled(label, icon, action, false)
  }

  /// Add Menu Item with Icon and disabled state
  pub fn menu_with_icon_and_disabled(
    mut self, label: impl Into<SharedString>, icon: impl Into<Icon>, action: Box<dyn Action>,
    disabled: bool,
  ) -> Self {
    self.add_menu_item(label, Some(icon.into()), action, disabled, false);
    self
  }

  /// Add Menu Item with check icon
  pub fn menu_with_check(
    self, label: impl Into<SharedString>, checked: bool, action: Box<dyn Action>,
  ) -> Self {
    self.menu_with_check_and_disabled(label, checked, action, false)
  }

  /// Add Menu Item with check icon and disabled state
  pub fn menu_with_check_and_disabled(
    mut self, label: impl Into<SharedString>, checked: bool, action: Box<dyn Action>,
    disabled: bool,
  ) -> Self {
    self.add_menu_item(label, None, action, disabled, checked);
    self
  }

  /// Add Menu Item with custom element render.
  pub fn menu_element<F, E>(self, action: Box<dyn Action>, builder: F) -> Self
  where
    F: Fn(&mut Window, &mut App) -> E + 'static,
    E: IntoElement, {
    self.menu_element_with_check(false, action, builder)
  }

  /// Add Menu Item with custom element render with disabled state.
  pub fn menu_element_with_disabled<F, E>(
    self, action: Box<dyn Action>, disabled: bool, builder: F,
  ) -> Self
  where
    F: Fn(&mut Window, &mut App) -> E + 'static,
    E: IntoElement, {
    self.menu_element_with_check_and_disabled(false, action, disabled, builder)
  }

  /// Add Menu Item with custom element render with icon.
  pub fn menu_element_with_icon<F, E>(
    self, icon: impl Into<Icon>, action: Box<dyn Action>, builder: F,
  ) -> Self
  where
    F: Fn(&mut Window, &mut App) -> E + 'static,
    E: IntoElement, {
    self.menu_element_with_icon_and_disabled(icon, action, false, builder)
  }

  /// Add Menu Item with custom element render with check state
  pub fn menu_element_with_check<F, E>(
    self, checked: bool, action: Box<dyn Action>, builder: F,
  ) -> Self
  where
    F: Fn(&mut Window, &mut App) -> E + 'static,
    E: IntoElement, {
    self.menu_element_with_check_and_disabled(checked, action, false, builder)
  }

  /// Add Menu Item with custom element render with icon and disabled state
  fn menu_element_with_icon_and_disabled<F, E>(
    mut self, icon: impl Into<Icon>, action: Box<dyn Action>, disabled: bool, builder: F,
  ) -> Self
  where
    F: Fn(&mut Window, &mut App) -> E + 'static,
    E: IntoElement, {
    self.menu_items.push(
      PopupMenuItem::element(builder)
        .action(action)
        .icon(icon)
        .disabled(disabled),
    );
    self
  }

  /// Add Menu Item with custom element render with check state and disabled
  /// state
  fn menu_element_with_check_and_disabled<F, E>(
    mut self, checked: bool, action: Box<dyn Action>, disabled: bool, builder: F,
  ) -> Self
  where
    F: Fn(&mut Window, &mut App) -> E + 'static,
    E: IntoElement, {
    self.menu_items.push(
      PopupMenuItem::element(builder)
        .action(action)
        .checked(checked)
        .disabled(disabled),
    );
    self
  }

  /// Add a separator Menu Item
  pub fn separator(mut self) -> Self {
    if self.menu_items.is_empty() {
      return self;
    }

    if let Some(PopupMenuItem::Separator) = self.menu_items.last() {
      return self;
    }

    self.menu_items.push(PopupMenuItem::separator());
    self
  }

  /// Add a Submenu.
  ///
  /// This only needs app access, so it can also be used from widget-specific
  /// context menu builders such as table and list delegates.
  pub fn submenu(
    self, label: impl Into<SharedString>, window: &mut Window, cx: &mut App,
    f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> Self {
    self.submenu_with_icon(None, label, window, cx, f)
  }

  /// Add a Submenu item with icon.
  pub fn submenu_with_icon(
    mut self, icon: Option<Icon>, label: impl Into<SharedString>, window: &mut Window,
    cx: &mut App,
    f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> Self {
    let submenu = PopupMenu::build(window, cx, f);
    let parent_menu = self.self_entity.clone();
    submenu.update(cx, |view, _| {
      view.parent_menu = Some(parent_menu);
    });

    self
      .menu_items
      .push(PopupMenuItem::submenu(label, submenu).when_some(icon, |this, icon| this.icon(icon)));
    self
  }

  /// Add menu item.
  pub fn item(mut self, item: impl Into<PopupMenuItem>) -> Self {
    let item: PopupMenuItem = item.into();
    self.menu_items.push(item);
    self
  }

  /// Use small size, the menu item will have smaller height.
  pub fn small(mut self) -> Self {
    self.size = Size::Small;
    self
  }

  fn add_menu_item(
    &mut self, label: impl Into<SharedString>, icon: Option<Icon>, action: Box<dyn Action>,
    disabled: bool, checked: bool,
  ) -> &mut Self {
    self.menu_items.push(
      PopupMenuItem::new(label)
        .when_some(icon, |item, icon| item.icon(icon))
        .disabled(disabled)
        .checked(checked)
        .action(action),
    );
    self
  }

  pub(super) fn with_menu_items<I>(
    mut self, items: impl IntoIterator<Item = I>, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self
  where
    I: Into<OwnedMenuItem>, {
    for item in items {
      match item.into() {
        OwnedMenuItem::Action { name, action, .. } => {
          self = self.menu(name.as_str().to_owned(), action.boxed_clone())
        }
        OwnedMenuItem::Separator => {
          self = self.separator();
        }
        OwnedMenuItem::Submenu(submenu) => {
          let translated_name = submenu.name.as_str().to_owned();
          self = self.submenu(translated_name, window, cx, move |menu, window, cx| {
            menu.with_menu_items(submenu.items.clone(), window, cx)
          })
        }
        OwnedMenuItem::SystemMenu(_) => {}
      }
    }

    if self.menu_items.len() > 20 {
      self.scrollable = true;
    }

    self
  }

  pub(crate) fn active_submenu(&self) -> Option<Entity<PopupMenu>> {
    if let Some(ix) = self.selected_index
      && let Some(item) = self.menu_items.get(ix)
    {
      return match item {
        PopupMenuItem::Submenu { menu, .. } => Some(menu.clone()),
        _ => None,
      };
    }

    None
  }

  pub fn is_empty(&self) -> bool {
    self.menu_items.is_empty()
  }

  fn clickable_menu_items(&self) -> impl Iterator<Item = (usize, &PopupMenuItem)> {
    self
      .menu_items
      .iter()
      .enumerate()
      .filter(|(_, item)| item.is_clickable())
  }

  fn on_click(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
    cx.stop_propagation();
    window.prevent_default();
    self.selected_index = Some(ix);
    self.confirm(&Confirm { secondary: false }, window, cx);
  }

  fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(index) = self.selected_index {
      let item = self.menu_items.get(index);
      match item {
        Some(PopupMenuItem::Item {
          handler, action, ..
        }) => {
          if let Some(handler) = handler {
            handler(&ClickEvent::default(), window, cx);
          } else if let Some(action) = action.as_ref() {
            self.dispatch_confirm_action(action.as_ref(), window, cx);
          }

          self.dismiss(&Cancel, window, cx)
        }
        Some(PopupMenuItem::ElementItem {
          handler, action, ..
        }) => {
          if let Some(handler) = handler {
            handler(&ClickEvent::default(), window, cx);
          } else if let Some(action) = action.as_ref() {
            self.dispatch_confirm_action(action.as_ref(), window, cx);
          }
          self.dismiss(&Cancel, window, cx)
        }
        _ => {}
      }
    }
  }

  fn dispatch_confirm_action(
    &self, action: &dyn Action, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if let Some(context) = self.action_context.as_ref() {
      context.focus(window, cx);
      context.dispatch_action(action, window, cx);
      return;
    }

    window.dispatch_action(action.boxed_clone(), cx);
  }

  fn set_selected_index(&mut self, ix: usize, cx: &mut Context<Self>) {
    if self.selected_index != Some(ix) {
      self.selected_index = Some(ix);
      self.scroll_handle.scroll_to_item(ix);
      cx.notify();
    }
  }

  fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
    cx.stop_propagation();
    let ix = self.selected_index.unwrap_or(0);

    if let Some((prev_ix, _)) = self
      .menu_items
      .iter()
      .enumerate()
      .rev()
      .find(|(i, item)| *i < ix && item.is_clickable())
    {
      self.set_selected_index(prev_ix, cx);
      return;
    }

    let last_clickable_ix = self.clickable_menu_items().last().map(|(ix, _)| ix);
    self.set_selected_index(last_clickable_ix.unwrap_or(0), cx);
  }

  fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
    cx.stop_propagation();
    let Some(ix) = self.selected_index else {
      self.set_selected_index(0, cx);
      return;
    };

    if let Some((next_ix, _)) = self
      .menu_items
      .iter()
      .enumerate()
      .find(|(i, item)| *i > ix && item.is_clickable())
    {
      self.set_selected_index(next_ix, cx);
      return;
    }

    self.set_selected_index(0, cx);
  }

  fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
    let handled = if matches!(self.submenu_anchor.0, Corner::TopLeft | Corner::BottomLeft) {
      self.unselect_submenu(window, cx)
    } else {
      self.select_submenu(window, cx)
    };

    if self.parent_side(cx).is_left() {
      self.focus_parent_menu(window, cx);
    }

    if handled {
      return;
    }

    // For parent AppMenuBar to handle.
    if self.parent_menu.is_none() {
      cx.propagate();
    }
  }

  fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
    let handled = if matches!(self.submenu_anchor.0, Corner::TopLeft | Corner::BottomLeft) {
      self.select_submenu(window, cx)
    } else {
      self.unselect_submenu(window, cx)
    };

    if self.parent_side(cx).is_right() {
      self.focus_parent_menu(window, cx);
    }

    if handled {
      return;
    }

    // For parent AppMenuBar to handle.
    if self.parent_menu.is_none() {
      cx.propagate();
    }
  }

  fn select_submenu(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
    if let Some(active_submenu) = self.active_submenu() {
      // Focus the submenu, so that can be handle the action.
      active_submenu.update(cx, |view, cx| {
        view.set_selected_index(0, cx);
        view.focus_handle.focus(window, cx);
      });
      cx.notify();
      return true;
    }

    false
  }

  fn unselect_submenu(&mut self, _: &mut Window, cx: &mut Context<Self>) -> bool {
    if let Some(active_submenu) = self.active_submenu() {
      active_submenu.update(cx, |view, cx| {
        view.selected_index = None;
        cx.notify();
      });
      return true;
    }

    false
  }

  fn focus_parent_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let Some(parent) = self.parent_menu.as_ref() else {
      return;
    };
    let Some(parent) = parent.upgrade() else {
      return;
    };

    self.selected_index = None;
    parent.update(cx, |view, cx| {
      view.focus_handle.focus(window, cx);
      cx.notify();
    });
  }

  fn parent_side(&self, cx: &App) -> Side {
    let Some(parent) = self.parent_menu.as_ref() else {
      return Side::Left;
    };

    let Some(parent) = parent.upgrade() else {
      return Side::Left;
    };

    match parent.read(cx).submenu_anchor.0 {
      Corner::TopLeft | Corner::BottomLeft => Side::Left,
      Corner::TopRight | Corner::BottomRight => Side::Right,
    }
  }

  fn dismiss(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
    if self.active_submenu().is_some() {
      return;
    }

    cx.emit(DismissEvent);

    // Focus back to the previous focused handle.
    if let Some(action_context) = self.action_context.as_ref() {
      window.focus(action_context, cx);
    }

    let Some(parent_menu) = self.parent_menu.clone() else {
      return;
    };

    // Dismiss parent menu, when this menu is dismissed
    _ = parent_menu.update(cx, |view, cx| {
      view.selected_index = None;
      view.dismiss(&Cancel, window, cx);
    });
  }

  fn handle_dismiss(
    &mut self, position: &Point<Pixels>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    // Do not dismiss, if click inside the parent menu
    if let Some(parent) = self.parent_menu.as_ref()
      && let Some(parent) = parent.upgrade()
      && parent.read(cx).bounds.contains(position)
    {
      return;
    }

    self.dismiss(&Cancel, window, cx);
  }

  fn on_mouse_down_out(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    self.handle_dismiss(&e.position, window, cx);
  }

  fn render_key_binding(
    &self, action: Option<Box<dyn Action>>, window: &mut Window, _: &mut Context<Self>,
  ) -> Option<Kbd> {
    let action = action?;

    match self
      .action_context
      .as_ref()
      .and_then(|handle| Kbd::binding_for_action_in(action.as_ref(), handle, window))
    {
      Some(kbd) => Some(kbd),
      // Fallback to App level key binding
      None => Kbd::binding_for_action(action.as_ref(), None, window),
    }
    .map(|this| this.appearance(false))
  }

  fn render_icon(has_icon: bool, checked: bool, icon: Option<Icon>) -> Option<Icon> {
    if !has_icon {
      return None;
    }

    let icon = if let Some(icon) = icon {
      icon.clone()
    } else if checked {
      Icon::new(IconName::Checkmark)
    } else {
      Icon::new(IconName::Empty)
    };

    Some(icon)
  }

  #[inline]
  fn max_width(&self) -> Pixels {
    self.max_width.unwrap_or(px(500.))
  }

  /// Calculate the anchor corner and offset for child submenu
  fn update_submenu_menu_anchor(&mut self, window: &Window) {
    let bounds = self.bounds;
    let max_width = self.max_width();
    let horizontal_gap = self.size.container_px();
    let vertical_shift = self.size.container_py();

    let (anchor, mut offset) = if max_width + bounds.origin.x > window.bounds().size.width {
      (
        Corner::TopRight,
        Point {
          x: -horizontal_gap,
          y: Pixels::ZERO,
        },
      )
    } else {
      (
        Corner::TopLeft,
        Point {
          x: bounds.size.width + horizontal_gap,
          y: Pixels::ZERO,
        },
      )
    };

    offset.y = -vertical_shift;
    self.submenu_anchor = (anchor, offset);
  }

  fn render_item(
    &self, ix: usize, item: &PopupMenuItem, options: RenderOptions, window: &mut Window,
    cx: &mut Context<Self>,
  ) -> AnyElement {
    let has_left_icon = options.has_left_icon;
    let is_left_check = options.check_side.is_left() && item.is_checked();
    let right_check_icon = if options.check_side.is_right() && item.is_checked() {
      Some(Icon::new(IconName::Checkmark))
    } else {
      None
    };

    let selected = self.selected_index == Some(ix);
    const EDGE_PADDING: Pixels = px(4.);

    let left_icon = Self::render_icon(has_left_icon, is_left_check, Self::item_icon(item));

    match item {
      PopupMenuItem::Separator => Divider::horizontal().my_1().into_any_element(),
      PopupMenuItem::Label(label) => {
        let label = label.clone();
        IconLabel::new(("label", ix))
          .justify_start()
          .children(left_icon)
          .child(label)
          .children(right_check_icon)
          .into_any_element()
      }
      PopupMenuItem::ElementItem {
        render,
        icon,
        disabled,
        ..
      } => {
        let left_icon = Self::render_icon(has_left_icon, is_left_check, icon.clone());
        let disabled = *disabled;

        Button::new(("element-item", ix))
          .flat()
          .expand(true)
          .selected(selected)
          .disabled(disabled)
          .on_hover(cx.listener(move |this, hovered, _, cx| {
            if *hovered {
              this.selected_index = Some(ix);
            } else if this.selected_index == Some(ix) {
              this.selected_index = None;
            }
            cx.notify();
          }))
          .when(!disabled, |this: Button| {
            this.on_click(cx.listener(move |this, _, window, cx| this.on_click(ix, window, cx)))
          })
          .children(left_icon)
          .child(
            div()
              .flex_1()
              .min_w_0()
              .truncate()
              .child((render)(window, cx)),
          )
          .children(right_check_icon)
          .into_any_element()
      }
      PopupMenuItem::Item {
        icon,
        label,
        action,
        disabled,
        is_link,
        ..
      } => {
        let show_link_icon = *is_link && self.external_link_icon;
        let action = action.as_ref().map(|action| action.boxed_clone());
        let key = self.render_key_binding(action, window, cx);
        let has_key = key.is_some();
        let has_right_check = right_check_icon.is_some();
        let label = label.clone();
        let left_icon = Self::render_icon(has_left_icon, is_left_check, icon.clone());
        let disabled = *disabled;
        let has_trailing = show_link_icon || has_key || has_right_check;

        Button::new(("item", ix))
          .flat()
          .expand(true)
          .selected(selected)
          .disabled(disabled)
          .on_hover(cx.listener(move |this, hovered, _, cx| {
            if *hovered {
              this.selected_index = Some(ix);
            } else if this.selected_index == Some(ix) {
              this.selected_index = None;
            }
            cx.notify();
          }))
          .when(!disabled, |this: Button| {
            this.on_click(cx.listener(move |this, _, window, cx| this.on_click(ix, window, cx)))
          })
          .children(left_icon)
          .child(div().flex_1().min_w_0().truncate().child(label))
          .when(has_trailing, |this| {
            this
              .when_some(key, |this, key| this.child(key))
              .when(show_link_icon, |this| this.child(Icon::new(IconName::Link)))
              .children(right_check_icon)
          })
          .into_any_element()
      }
      PopupMenuItem::Submenu {
        icon,
        label,
        menu,
        disabled,
      } => {
        let label = label.clone();
        let left_icon = Self::render_icon(has_left_icon, false, icon.clone());
        h_flex()
          .relative()
          .w_full()
          .child(
            Button::new(("submenu", ix))
              .flat()
              .expand(true)
              .flex_1()
              .selected(selected)
              .disabled(*disabled)
              .on_hover(cx.listener(move |this, hovered, _, cx| {
                if *hovered {
                  this.selected_index = Some(ix);
                }
                cx.notify();
              }))
              .children(left_icon)
              .child(div().flex_1().min_w_0().truncate().child(label))
              .child(Icon::new(IconName::ChevronRight).text_color(cx.theme().muted_foreground)),
          )
          .when(selected, |this| {
            let horizontal_gap = self.size.container_px();
            let vertical_shift = self.size.container_py();

            let (position_mode, position, anchor, offset) = if let Some(item_bounds) =
              self.item_bounds.get(ix).copied()
            {
              let submenu_max_width = menu.read(cx).max_width();
              let open_left =
                item_bounds.origin.x + item_bounds.size.width + horizontal_gap + submenu_max_width
                  > window.bounds().right();

              let (anchor, mut offset) = if open_left {
                (
                  Corner::TopRight,
                  Point {
                    x: -horizontal_gap,
                    y: Pixels::ZERO,
                  },
                )
              } else {
                (
                  Corner::TopLeft,
                  Point {
                    x: item_bounds.size.width + horizontal_gap,
                    y: Pixels::ZERO,
                  },
                )
              };

              offset.y = -vertical_shift;
              (
                AnchoredPositionMode::Window,
                item_bounds.origin,
                anchor,
                offset,
              )
            } else {
              let (anchor, offset) = self.submenu_anchor;
              (
                AnchoredPositionMode::Local,
                Point::default(),
                anchor,
                offset,
              )
            };

            this.child(
              anchored()
                .position_mode(position_mode)
                .position(position)
                .anchor(anchor)
                .offset(offset)
                .child(
                  v_flex()
                    .id("submenu")
                    .occlude()
                    .popover_style(cx.theme())
                    .container_padding(self.size)
                    .child(menu.clone()),
                )
                .snap_to_window_with_margin(Edges::all(EDGE_PADDING)),
            )
          })
          .into_any_element()
      }
    }
  }

  fn item_icon(item: &PopupMenuItem) -> Option<Icon> {
    match item {
      PopupMenuItem::Item { icon, .. } => icon.clone(),
      PopupMenuItem::ElementItem { icon, .. } => icon.clone(),
      PopupMenuItem::Submenu { icon, .. } => icon.clone(),
      _ => None,
    }
  }
}

impl FluentBuilder for PopupMenu {}
impl EventEmitter<DismissEvent> for PopupMenu {}
impl Focusable for PopupMenu {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

#[derive(Clone, Copy)]
struct RenderOptions {
  has_left_icon: bool,
  check_side: Side,
}

impl Render for PopupMenu {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    self.update_submenu_menu_anchor(window);

    let view = cx.entity().clone();
    let items_count = self.menu_items.len();

    let max_height = self.max_height.unwrap_or_else(|| {
      let window_half_height = window.window_bounds().get_bounds().size.height * 0.5;
      window_half_height.min(px(450.))
    });

    let has_left_icon = self
      .menu_items
      .iter()
      .any(|item| item.has_left_icon(self.check_side));

    let max_width = self.max_width();
    let options = RenderOptions {
      has_left_icon,
      check_side: self.check_side,
    };

    v_flex()
      .id("popup-menu")
      .key_context(CONTEXT)
      .track_focus(&self.focus_handle)
      .on_action(cx.listener(Self::select_up))
      .on_action(cx.listener(Self::select_down))
      .on_action(cx.listener(Self::select_left))
      .on_action(cx.listener(Self::select_right))
      .on_action(cx.listener(Self::confirm))
      .on_action(cx.listener(Self::dismiss))
      .on_mouse_down_out(cx.listener(Self::on_mouse_down_out))
      .id("items")
      .min_w(rems(8.))
      .when_some(self.min_width, |this, min_width| this.min_w(min_width))
      .max_w(max_width)
      .when(self.scrollable, |this| {
        this
          .max_h(max_height)
          .overflow_y_scroll()
          .track_scroll(&self.scroll_handle)
      })
      .children(
        self
          .menu_items
          .iter()
          .enumerate()
          // Ignore last separator
          .filter(|(ix, item)| !(*ix + 1 == items_count && item.is_separator()))
          .map(|(ix, item)| {
            let view = view.clone();
            div()
              .w_full()
              .on_prepaint(move |bounds, _, cx| {
                view.update(cx, |menu, _| {
                  if menu.item_bounds.len() <= ix {
                    menu.item_bounds.resize(ix + 1, Bounds::default());
                  }
                  menu.item_bounds[ix] = bounds;
                })
              })
              .child(self.render_item(ix, item, options, window, cx))
          }),
      )
      .on_prepaint(move |bounds, _, cx| {
        view.update(cx, |r, cx| {
          if r.bounds != bounds {
            r.bounds = bounds;
            cx.notify();
          }
        })
      })
      .when(self.scrollable, |this| {
        // TODO: When the menu is limited by `overflow_y_scroll`, the sub-menu will
        // cannot be displayed.
        this.vertical_scrollbar(&self.scroll_handle)
      })
  }
}
