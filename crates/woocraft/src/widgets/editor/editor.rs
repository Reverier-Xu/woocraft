use std::rc::Rc;

use gpui::{
  AnyElement, App, Context, DefiniteLength, Edges, EdgesRefinement, Entity, Focusable,
  InteractiveElement as _, IntoElement, IsZero, MouseButton, ParentElement as _, Rems, RenderOnce,
  StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
};

use super::state::{CONTEXT, Copy, Cut, InputState, Paste, SelectAll};
use crate::{
  ActiveTheme, ContextMenuExt, IconName, PopupMenu, Selectable, Size, StyleSized as _, StyledExt,
  translate_woocraft, v_flex,
  widgets::{
    editor::element::{LINE_NUMBER_TEXT_GAP, RIGHT_MARGIN},
    scroll::Scrollbar,
  },
};

type ContextMenuBuilder =
  Rc<dyn Fn(PopupMenu, &Entity<InputState>, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

/// Editor element bind to an [`InputState`].
#[derive(IntoElement)]
pub struct Editor {
  state: Entity<InputState>,
  style: StyleRefinement,
  size: Size,
  prefix: Option<AnyElement>,
  height: Option<DefiniteLength>,
  appearance: bool,
  disabled: bool,
  bordered: bool,
  focus_bordered: bool,
  tab_index: isize,
  selected: bool,
  context_menu_enabled: bool,
  default_context_menu: bool,
  context_menu_builder: Option<ContextMenuBuilder>,
}

impl_sizable!(Editor);

impl Selectable for Editor {
  fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  fn is_selected(&self) -> bool {
    self.selected
  }
}

impl Editor {
  /// Create a new [`Editor`] element bind to the [`InputState`].
  pub fn new(state: &Entity<InputState>) -> Self {
    Self {
      state: state.clone(),
      size: Size::default(),
      style: StyleRefinement::default(),
      prefix: None,
      height: None,
      appearance: true,
      disabled: false,
      bordered: true,
      focus_bordered: true,
      tab_index: 0,
      selected: false,
      context_menu_enabled: true,
      default_context_menu: true,
      context_menu_builder: None,
    }
  }

  pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
    self.prefix = Some(prefix.into_any_element());
    self
  }

  /// Set full height of the editor.
  pub fn h_full(mut self) -> Self {
    self.height = Some(relative(1.));
    self
  }

  /// Set height of the editor.
  pub fn h(mut self, height: impl Into<DefiniteLength>) -> Self {
    self.height = Some(height.into());
    self
  }

  /// Set the appearance of the editor, if false the editor will no border,
  /// background.
  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }

  /// Set the bordered for the editor, default: true
  pub fn bordered(mut self, bordered: bool) -> Self {
    self.bordered = bordered;
    self
  }

  /// Set focus border for the editor, default is true.
  pub fn focus_bordered(mut self, bordered: bool) -> Self {
    self.focus_bordered = bordered;
    self
  }

  /// Set to disable the editor.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  /// Set the tab index for the editor, default is 0.
  pub fn tab_index(mut self, index: isize) -> Self {
    self.tab_index = index;
    self
  }

  /// Enable or disable context menu support.
  pub fn context_menu_enabled(mut self, enabled: bool) -> Self {
    self.context_menu_enabled = enabled;
    self
  }

  /// Enable or disable default context menu items.
  ///
  /// Default items: Cut, Copy, Paste, Select All.
  pub fn default_context_menu(mut self, enabled: bool) -> Self {
    self.default_context_menu = enabled;
    self
  }

  /// Extend context menu items using an external builder.
  pub fn context_menu(
    mut self,
    builder: impl Fn(PopupMenu, &Entity<InputState>, &mut Window, &mut Context<PopupMenu>) -> PopupMenu
    + 'static,
  ) -> Self {
    self.context_menu_builder = Some(Rc::new(builder));
    self
  }

  /// This method must after the refine_style.
  fn render_editor(
    paddings: EdgesRefinement<DefiniteLength>, input_state: &Entity<InputState>,
    state: &InputState, window: &Window, _cx: &App,
  ) -> impl IntoElement {
    let base_size = window.text_style().font_size;
    let rem_size = window.rem_size();

    let paddings = Edges {
      left: paddings
        .left
        .map(|v| v.to_pixels(base_size, rem_size))
        .unwrap_or(px(0.)),
      right: paddings
        .right
        .map(|v| v.to_pixels(base_size, rem_size))
        .unwrap_or(px(0.)),
      top: paddings
        .top
        .map(|v| v.to_pixels(base_size, rem_size))
        .unwrap_or(px(0.)),
      bottom: paddings
        .bottom
        .map(|v| v.to_pixels(base_size, rem_size))
        .unwrap_or(px(0.)),
    };

    v_flex()
      .size_full()
      .children(state.search_panel.clone())
      .child(div().flex_1().child(input_state.clone()).map(|this| {
        if let Some(last_layout) = state.last_layout.as_ref() {
          let left = if last_layout.line_number_width.is_zero() {
            px(0.)
          } else {
            // Align left edge to the line number.
            paddings.left + last_layout.line_number_width - LINE_NUMBER_TEXT_GAP
          };

          let scroll_size = gpui::Size {
            width: state.scroll_size.width - left + paddings.right + RIGHT_MARGIN,
            height: state.scroll_size.height,
          };

          let scrollbar = if !state.soft_wrap {
            Scrollbar::new(&state.scroll_handle)
          } else {
            Scrollbar::vertical(&state.scroll_handle)
          };

          this.relative().child(
            div()
              .absolute()
              .top(-paddings.top)
              .left(left)
              .right(-paddings.right)
              .bottom(-paddings.bottom)
              .child(scrollbar.scroll_size(scroll_size)),
          )
        } else {
          this
        }
      }))
  }
}

impl_styled!(Editor);

impl RenderOnce for Editor {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    const LINE_HEIGHT: Rems = Rems(1.5);

    self.state.update(cx, |state, _| {
      state.disabled = self.disabled;
      state.size = self.size;
    });

    let state = self.state.read(cx);
    let focused = state.focus_handle.is_focused(window) && !state.disabled;
    let gap_x = match self.size {
      Size::Small => px(4.),
      Size::Large => px(8.),
      _ => px(6.),
    };

    let bg = if state.disabled {
      cx.theme().muted
    } else if state.mode.is_code_editor() {
      cx.theme().editor_background()
    } else {
      cx.theme().background
    };

    let prefix = self.prefix;

    let input = div()
      .id(("input", self.state.entity_id()))
      .flex()
      .key_context(CONTEXT)
      .track_focus(&state.focus_handle.clone())
      .tab_index(self.tab_index)
      .when(!state.disabled, |this| {
        this.on_action(window.listener_for(&self.state, InputState::escape))
      })
      .when(!state.disabled && !state.read_only, |this| {
        this
          .on_action(window.listener_for(&self.state, InputState::backspace))
          .on_action(window.listener_for(&self.state, InputState::delete))
          .on_action(window.listener_for(&self.state, InputState::delete_to_beginning_of_line))
          .on_action(window.listener_for(&self.state, InputState::delete_to_end_of_line))
          .on_action(window.listener_for(&self.state, InputState::delete_previous_word))
          .on_action(window.listener_for(&self.state, InputState::delete_next_word))
          .on_action(window.listener_for(&self.state, InputState::enter))
          .on_action(window.listener_for(&self.state, InputState::paste))
          .on_action(window.listener_for(&self.state, InputState::cut))
          .on_action(window.listener_for(&self.state, InputState::undo))
          .on_action(window.listener_for(&self.state, InputState::redo))
          .when(state.mode.is_multi_line(), |this| {
            this
              .on_action(window.listener_for(&self.state, InputState::indent_inline))
              .on_action(window.listener_for(&self.state, InputState::outdent_inline))
              .on_action(window.listener_for(&self.state, InputState::indent_block))
              .on_action(window.listener_for(&self.state, InputState::outdent_block))
          })
          .on_action(window.listener_for(&self.state, InputState::on_action_toggle_code_actions))
      })
      .on_action(window.listener_for(&self.state, InputState::left))
      .on_action(window.listener_for(&self.state, InputState::right))
      .on_action(window.listener_for(&self.state, InputState::select_left))
      .on_action(window.listener_for(&self.state, InputState::select_right))
      .when(state.mode.is_multi_line(), |this| {
        this
          .on_action(window.listener_for(&self.state, InputState::up))
          .on_action(window.listener_for(&self.state, InputState::down))
          .on_action(window.listener_for(&self.state, InputState::select_up))
          .on_action(window.listener_for(&self.state, InputState::select_down))
          .on_action(window.listener_for(&self.state, InputState::page_up))
          .on_action(window.listener_for(&self.state, InputState::page_down))
          .on_action(window.listener_for(&self.state, InputState::on_action_go_to_definition))
      })
      .on_action(window.listener_for(&self.state, InputState::select_all))
      .on_action(window.listener_for(&self.state, InputState::select_to_start_of_line))
      .on_action(window.listener_for(&self.state, InputState::select_to_end_of_line))
      .on_action(window.listener_for(&self.state, InputState::select_to_previous_word))
      .on_action(window.listener_for(&self.state, InputState::select_to_next_word))
      .on_action(window.listener_for(&self.state, InputState::home))
      .on_action(window.listener_for(&self.state, InputState::end))
      .on_action(window.listener_for(&self.state, InputState::move_to_start))
      .on_action(window.listener_for(&self.state, InputState::move_to_end))
      .on_action(window.listener_for(&self.state, InputState::move_to_previous_word))
      .on_action(window.listener_for(&self.state, InputState::move_to_next_word))
      .on_action(window.listener_for(&self.state, InputState::select_to_start))
      .on_action(window.listener_for(&self.state, InputState::select_to_end))
      .on_action(window.listener_for(&self.state, InputState::show_character_palette))
      .on_action(window.listener_for(&self.state, InputState::copy))
      .on_action(window.listener_for(&self.state, InputState::on_action_search))
      .on_key_down(window.listener_for(&self.state, InputState::on_key_down))
      .on_mouse_down(
        MouseButton::Left,
        window.listener_for(&self.state, InputState::on_mouse_down),
      )
      .on_mouse_down(
        MouseButton::Right,
        window.listener_for(&self.state, InputState::on_mouse_down),
      )
      .on_mouse_up(
        MouseButton::Left,
        window.listener_for(&self.state, InputState::on_mouse_up),
      )
      .on_mouse_up(
        MouseButton::Right,
        window.listener_for(&self.state, InputState::on_mouse_up),
      )
      .on_mouse_move(window.listener_for(&self.state, InputState::on_mouse_move))
      .on_scroll_wheel(window.listener_for(&self.state, InputState::on_scroll_wheel))
      .size_full()
      .line_height(LINE_HEIGHT)
      .component_h(self.size)
      .text_size(self.size.text_size())
      .cursor_text()
      .items_center()
      .h_auto()
      .when_some(self.height, |this, height| this.h(height))
      .when(self.appearance, |this| {
        this
          .bg(bg)
          .rounded(cx.theme().radius)
          .when(self.bordered, |this| {
            this
              .border_color(cx.theme().input)
              .border_1()
              .when(focused && self.focus_bordered, |this| {
                this.border_color(cx.theme().ring)
              })
          })
      })
      .items_center()
      .gap(gap_x)
      .refine_style(&self.style)
      .children(prefix)
      .map(|mut this| {
        let paddings = this.style().padding.clone();
        this.child(Self::render_editor(
          paddings,
          &self.state,
          state,
          window,
          cx,
        ))
      });

    if !self.context_menu_enabled {
      return input.into_any_element();
    }

    let input_state = self.state.clone();
    let default_context_menu = self.default_context_menu;
    let context_menu_builder = self.context_menu_builder.clone();

    input
      .context_menu(move |menu, window, cx| {
        let (is_writable, has_selection, has_paste, focus_handle, suppress_editor_menu) = {
          let state = input_state.read(cx);
          let suppress_editor_menu = state
            .search_panel
            .as_ref()
            .is_some_and(|panel| panel.focus_handle(cx).contains_focused(window, cx));
          (
            !state.disabled && !state.read_only,
            !state.selected_range.is_empty(),
            !state.disabled && !state.read_only && cx.read_from_clipboard().is_some(),
            state.focus_handle.clone(),
            suppress_editor_menu,
          )
        };

        let mut menu = menu.small().action_context(focus_handle);

        if suppress_editor_menu {
          return menu;
        }

        input_state.update(cx, |state, _| {
          state.shared_context_menu_open = true;
        });

        if default_context_menu {
          menu = menu
            .menu_with_icon_and_disabled(
              translate_woocraft("editor.context_menu.cut"),
              IconName::Cut,
              Box::new(Cut),
              !(is_writable && has_selection),
            )
            .menu_with_icon_and_disabled(
              translate_woocraft("editor.context_menu.copy"),
              IconName::Copy,
              Box::new(Copy),
              !has_selection,
            )
            .menu_with_icon_and_disabled(
              translate_woocraft("editor.context_menu.paste"),
              IconName::ClipboardPaste,
              Box::new(Paste),
              !has_paste,
            )
            .separator()
            .menu_with_icon(
              translate_woocraft("editor.context_menu.select_all"),
              IconName::SelectAllOn,
              Box::new(SelectAll),
            );
        }

        if let Some(builder) = context_menu_builder.as_ref() {
          menu = builder(menu, &input_state, window, cx);
        }

        menu = {
          let state = input_state.read(cx);
          state.extend_context_menu_from_backend(menu, &input_state, window)
        };

        menu
      })
      .into_any_element()
  }
}
