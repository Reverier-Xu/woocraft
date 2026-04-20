mod code_action_menu;
mod completion_menu;
mod diagnostic_popover;
mod hover_popover;

pub(crate) use code_action_menu::*;
pub(crate) use completion_menu::*;
pub(crate) use diagnostic_popover::*;
use gpuim::{
  AnyElement, App, Div, ElementId, Entity, InteractiveElement as _, IntoElement,
  ParentElement as _, SharedString, Stateful, Styled as _, Window, div,
};
pub(crate) use hover_popover::*;

use crate::{ActiveTheme, CardStyle as _, widgets::label::Label};

pub(crate) enum ContextMenu {
  Completion(Entity<CompletionMenu>),
  CodeAction(Entity<CodeActionMenu>),
}

impl ContextMenu {
  pub(crate) fn is_open(&self, cx: &App) -> bool {
    match self {
      ContextMenu::Completion(menu) => menu.read(cx).is_open(),
      ContextMenu::CodeAction(menu) => menu.read(cx).is_open(),
    }
  }

  pub(crate) fn render(&self) -> impl IntoElement {
    match self {
      ContextMenu::Completion(menu) => menu.clone().into_any_element(),
      ContextMenu::CodeAction(menu) => menu.clone().into_any_element(),
    }
  }
}

pub(super) fn render_markdown(
  id: impl Into<ElementId>, markdown: impl Into<SharedString>, _: &mut Window, _: &mut App,
) -> AnyElement {
  let id: ElementId = id.into();
  let markdown: SharedString = markdown.into();
  div()
    .id(id)
    .w_full()
    .child(Label::new(markdown))
    .into_any_element()
}

pub(super) fn editor_popover(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
  div()
    .id(id)
    .flex_none()
    .occlude()
    .popover_style(cx.theme())
    .shadow_md()
    .text_xs()
    .p_1()
}
