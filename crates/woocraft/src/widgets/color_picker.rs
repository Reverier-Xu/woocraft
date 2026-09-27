//! Advanced color picker with multiple color space support.
//!
//! ColorPicker provides a comprehensive color selection interface that supports
//! multiple color spaces: RGBA (Red/Green/Blue with Alpha), Oklch (perceptually
//! uniform), and HSLA (traditional hue/saturation/lightness). The picker
//! includes a 2D gradient selector for lightness/chroma and 1D sliders for hue
//! and alpha.
//!
//! # Features
//! - **Multi-space support**: RGBA, Oklch (perceptually uniform), HSLA
//!   (traditional)
//! - **Hex editor**: Type or paste hex color codes directly (8-digit RGB hex)
//! - **2D gradient picker**: Visual lightness/chroma selection with interactive
//!   gradient
//! - **1D sliders**: Dedicated sliders for hue and alpha channels
//! - **Intelligent conversion**: Seamless conversion between color spaces
//!   preserving hue on RGB changes
//! - **Popover trigger**: Integrated popover button for space-efficient UI
//!
//! # Example
//! ```rust,ignore
//! use gpui::{App, Context};
//! use woocraft::{ColorPickerState, ColorPicker, ColorPickerOklch};
//!
//! let color_picker_state = cx.new(|cx| ColorPickerState::new());
//!
//! let picker = ColorPicker::new("my-color-picker", &color_picker_state)
//!   .outline(false);
//! ```
//!
//! # Internals
//! The picker uses Oklch as the internal representation because it's
//! perceptually uniform— equal numeric changes produce visually equal color
//! differences. RGB values and hex input are converted to Oklch on change, with
//! hue preservation when changing from RGB.

use std::sync::Arc;

use gpui::{
  App, AppContext as _, Bounds, Context, ElementId, Entity, EventEmitter, Hsla,
  InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement,
  Pixels, Point, RenderOnce, SharedString, StyleRefinement, Styled, Subscription, Window, canvas,
  div, fill, point, prelude::FluentBuilder as _, px, size,
};
use palette::{FromColor, Hsl, OklabHue, Oklch, Srgb};

use crate::{
  ActiveTheme, Anchor, Button, ButtonVariant, ButtonVariants, Disableable, ElementExt, Input,
  InputEvent, InputState, Popover, Sizable, Size, StyledExt, h_flex, translate_woocraft, v_flex,
};

const CHROMA_MAX: f32 = 0.4;
const HUE_MAX: f32 = 360.0;
const WARNING_LINE_THICKNESS: Pixels = px(2.0);
/// Fixed number of gradient segments per channel slider. 96 segments are
/// visually indistinguishable from per-pixel segmentation (gpui-kit uses the
/// same fixed step count) and keep the cached color table size constant.
const GRADIENT_STEPS: usize = 96;
const POINTER_OUTER_WIDTH: Pixels = px(4.0);
const POINTER_INNER_WIDTH: Pixels = px(2.0);

/// RGBA color representation for the color picker.
///
/// All components (r, g, b, a) are in range 0.0..=1.0.
/// - `r`: Red channel (0.0 = no red, 1.0 = full red)
/// - `g`: Green channel (0.0 = no green, 1.0 = full green)
/// - `b`: Blue channel (0.0 = no blue, 1.0 = full blue)
/// - `a`: Alpha channel (0.0 = fully transparent, 1.0 = fully opaque)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorPickerRgba {
  pub r: f32,
  pub g: f32,
  pub b: f32,
  pub a: f32,
}

/// Oklch color representation for the color picker (default internal space).
///
/// Oklch is a perceptually uniform color space ideal for intuitive color
/// selection. All components are in range 0.0..=1.0 (except hue which is
/// 0.0..=360.0).
/// - `lightness`: Perceived brightness (0.0 = black, 1.0 = white)
/// - `chroma`: Color intensity/saturation (0.0 = gray, higher = more vivid, max
///   ~0.4)
/// - `hue`: Color angle in degrees (0.0..=360.0; red=0, green=120, blue=240)
/// - `alpha`: Transparency (0.0 = transparent, 1.0 = opaque)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorPickerOklch {
  pub lightness: f32,
  pub chroma: f32,
  pub hue: f32,
  pub alpha: f32,
}

/// HSLA color representation for the color picker.
///
/// Traditional hue/saturation/lightness color space. All components in range
/// 0.0..=1.0 (except hue which is 0.0..=360.0).
/// - `hue`: Color angle (0.0..=360.0; red=0, green=120, blue=240)
/// - `saturation`: Color purity (0.0 = gray, 1.0 = fully saturated)
/// - `lightness`: Brightness (0.0 = black, 0.5 = normal, 1.0 = white)
/// - `alpha`: Transparency (0.0 = transparent, 1.0 = opaque)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorPickerHsla {
  pub hue: f32,
  pub saturation: f32,
  pub lightness: f32,
  pub alpha: f32,
}

/// Complete color value with all color space representations.
///
/// All four representations describe the same color, allowing consumers to pick
/// whatever format is most convenient. Conversions happen automatically when
/// the user changes any color parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorPickerValue {
  pub rgba_hex: SharedString,
  pub rgba: ColorPickerRgba,
  pub oklch: ColorPickerOklch,
  pub hsla: ColorPickerHsla,
}

/// Event emitted when the selected color changes.
///
/// Fired whenever the user adjusts any channel (2D gradient, hue/alpha sliders,
/// or hex input).
#[derive(Clone)]
pub enum ColorPickerEvent {
  Change(ColorPickerValue),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PickerChannel {
  Lightness,
  Chroma,
  Hue,
  Alpha,
}

impl PickerChannel {
  fn index(self) -> usize {
    match self {
      Self::Lightness => 0,
      Self::Chroma => 1,
      Self::Hue => 2,
      Self::Alpha => 3,
    }
  }

  fn id_label(self) -> &'static str {
    match self {
      Self::Lightness => "Lightness",
      Self::Chroma => "Chroma",
      Self::Hue => "Hue",
      Self::Alpha => "Alpha",
    }
  }

  fn i18n_key(self) -> &'static str {
    match self {
      Self::Lightness => "color_picker.lightness",
      Self::Chroma => "color_picker.chroma",
      Self::Hue => "color_picker.hue",
      Self::Alpha => "color_picker.alpha",
    }
  }

  fn range(self) -> (f32, f32) {
    match self {
      Self::Lightness => (0.0, 1.0),
      Self::Chroma => (0.0, CHROMA_MAX),
      Self::Hue => (0.0, HUE_MAX),
      Self::Alpha => (0.0, 1.0),
    }
  }

  fn value(self, oklch: ColorPickerOklch) -> f32 {
    match self {
      Self::Lightness => oklch.lightness,
      Self::Chroma => oklch.chroma,
      Self::Hue => oklch.hue,
      Self::Alpha => oklch.alpha,
    }
  }

  fn assign(self, oklch: &mut ColorPickerOklch, value: f32) {
    let (min, max) = self.range();
    let value = value.clamp(min, max);
    match self {
      Self::Lightness => oklch.lightness = value,
      Self::Chroma => oklch.chroma = value,
      Self::Hue => oklch.hue = value,
      Self::Alpha => oklch.alpha = value,
    }
  }

  fn normalized_value(self, oklch: ColorPickerOklch) -> f32 {
    let (min, max) = self.range();
    let span = (max - min).max(f32::EPSILON);
    ((self.value(oklch) - min) / span).clamp(0.0, 1.0)
  }

  fn value_from_ratio(self, ratio: f32) -> f32 {
    let (min, max) = self.range();
    min + (max - min) * ratio.clamp(0.0, 1.0)
  }

  /// Cache key for this channel's gradient table: the oklch components the
  /// gradient depends on, with the channel's own component zeroed because the
  /// gradient sweeps it across the full range.
  fn gradient_cache_key(self, mut oklch: ColorPickerOklch) -> ColorPickerOklch {
    self.assign(&mut oklch, 0.0);
    oklch
  }
}

/// One precomputed segment of a channel gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ChannelGradientStop {
  rgba: ColorPickerRgba,
  out_of_gamut: bool,
}

/// Cached gradient color table for one channel.
struct ChannelGradient {
  /// Oklch snapshot the table was built from, as produced by
  /// [`PickerChannel::gradient_cache_key`].
  key: ColorPickerOklch,
  colors: Arc<[ChannelGradientStop]>,
}

impl Default for ColorPickerOklch {
  fn default() -> Self {
    Self {
      lightness: 0.64,
      chroma: 0.17,
      hue: 248.0,
      alpha: 1.0,
    }
  }
}

/// State management for the color picker.
///
/// Holds the current color value in Oklch space (perceptually uniform), manages
/// internal channel sliders, and maintains the hex input field state.
/// Emits `ColorPickerEvent::Change` whenever the color changes.
pub struct ColorPickerState {
  value: ColorPickerOklch,
  /// Fully resolved value cache, kept in sync with `value` by `set_oklch` and
  /// `default_value` so renders never re-run color space conversions.
  resolved: ColorPickerValue,
  channel_bounds: [Bounds<Pixels>; 4],
  /// Per-channel gradient color tables (see [`Self::channel_gradient`]).
  gradient_cache: [Option<ChannelGradient>; 4],
  hex_input: Option<Entity<InputState>>,
  hex_input_dirty: bool,
  _hex_input_subscription: Option<Subscription>,
}

impl Default for ColorPickerState {
  fn default() -> Self {
    Self::new()
  }
}

impl ColorPickerState {
  /// Creates a new color picker state with default blue color (Oklch).
  ///
  /// Default color is a medium-saturation blue (h=248°, c=0.17, l=0.64, a=1.0).
  pub fn new() -> Self {
    let value = ColorPickerOklch::default();
    Self {
      value,
      resolved: resolve_value(value),
      channel_bounds: [Bounds::default(); 4],
      gradient_cache: [None, None, None, None],
      hex_input: None,
      hex_input_dirty: false,
      _hex_input_subscription: None,
    }
  }

  /// Sets the initial color value in Oklch color space.
  ///
  /// Call before rendering to start with a specific color. Values are clamped
  /// to valid ranges (lightness 0..1, chroma 0..0.4, hue 0..360, alpha 0..1).
  pub fn default_value(mut self, value: ColorPickerOklch) -> Self {
    self.value = sanitize_oklch(value);
    self.resolved = resolve_value(self.value);
    self
  }

  /// Updates the color to the specified Oklch value, emitting Change event.
  ///
  /// Use this to programmatically set the color (e.g., from external input).
  /// If the value hasn't changed, no event is emitted. All values are clamped
  /// to valid ranges.
  pub fn set_oklch(&mut self, value: ColorPickerOklch, cx: &mut Context<Self>) {
    let next = sanitize_oklch(value);
    if self.value == next {
      return;
    }
    self.value = next;
    self.resolved = resolve_value(next);
    cx.emit(ColorPickerEvent::Change(self.resolved.clone()));
    cx.notify();
  }

  /// Returns the current color in Oklch color space.
  pub fn oklch(&self) -> ColorPickerOklch {
    self.value
  }

  /// Returns the complete current color with all color space representations.
  ///
  /// Includes RGBA hex (8-digit), RGBA normalized values, Oklch, and HSLA.
  /// Clones the cache maintained by [`Self::set_oklch`] instead of re-running
  /// color space conversions.
  pub fn value(&self) -> ColorPickerValue {
    self.resolved.clone()
  }

  /// Returns the opt hex input field entity (if it has been created).
  pub fn hex_input(&self) -> Option<Entity<InputState>> {
    self.hex_input.clone()
  }

  fn ensure_hex_input(
    &mut self, window: &mut Window, cx: &mut Context<Self>,
  ) -> Entity<InputState> {
    if let Some(hex_input) = &self.hex_input {
      return hex_input.clone();
    }

    let initial_hex = self.resolved.rgba_hex.to_string();
    let hex_input = cx.new(|cx| {
      InputState::new(cx)
        .placeholder(translate_woocraft("color_picker.hex_placeholder"))
        .default_value(initial_hex.clone())
    });
    // Subscribing in-window lets the input event handler sync the display
    // right after a committed hex value is applied.
    let subscription = cx.subscribe_in(&hex_input, window, Self::on_hex_input_event);
    self.hex_input = Some(hex_input.clone());
    self._hex_input_subscription = Some(subscription);
    hex_input
  }

  /// Rewrites the hex input display to the canonical hex of the current value
  /// unless the user is mid-edit. Guards by comparing the displayed text with
  /// the expected value, so programmatic writes need no event bookkeeping.
  pub fn sync_hex_input_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let expected = self.resolved.rgba_hex.to_string();
    let hex_input = self.ensure_hex_input(window, cx);
    let current = hex_input.read(cx).value().to_string();
    if current == expected {
      self.hex_input_dirty = false;
      return;
    }
    // Preserve raw text while user is still editing.
    if self.hex_input_dirty {
      return;
    }

    hex_input.update(cx, |input, cx| {
      input.set_value(expected, window, cx);
    });
  }

  /// Returns the gradient color table for `channel`, rebuilding it only when
  /// an oklch component the gradient depends on has changed.
  fn channel_gradient(&mut self, channel: PickerChannel) -> Arc<[ChannelGradientStop]> {
    let cache_key = channel.gradient_cache_key(self.value);
    let current = self.value;
    let cache = &mut self.gradient_cache[channel.index()];
    match cache {
      Some(entry) if entry.key == cache_key => entry.colors.clone(),
      _ => {
        let colors: Arc<[ChannelGradientStop]> = build_channel_gradient(channel, current).into();
        *cache = Some(ChannelGradient {
          key: cache_key,
          colors: colors.clone(),
        });
        colors
      }
    }
  }

  fn set_channel_bounds(&mut self, channel: PickerChannel, bounds: Bounds<Pixels>) {
    self.channel_bounds[channel.index()] = bounds;
  }

  fn set_channel_by_position(
    &mut self, channel: PickerChannel, position: Point<Pixels>, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let bounds = self.channel_bounds[channel.index()];
    if bounds.size.width <= px(0.0) {
      return;
    }

    let ratio = ((position.x - bounds.left()) / bounds.size.width).clamp(0.0, 1.0);
    let mut next = self.value;
    channel.assign(&mut next, channel.value_from_ratio(ratio));
    self.set_oklch(next, cx);
    self.sync_hex_input_display(window, cx);
  }

  fn on_hex_input_event(
    &mut self, state: &Entity<InputState>, event: &InputEvent, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let input = state.read(cx).value().to_string();
    let parsed = match event {
      InputEvent::Change => {
        self.hex_input_dirty = true;
        if !is_complete_eight_hex_input(&input) {
          return;
        }
        parse_complete_eight_hex_color(&input)
      }
      InputEvent::PressEnter { .. } => {
        if !is_enter_committable_hex_input(&input) {
          return;
        }
        parse_hex_color(&input)
      }
      _ => return,
    };

    let Some(rgba) = parsed else {
      return;
    };

    // Skip edits that already denote the current color. Programmatic display
    // syncs write exactly the canonical hex of the current value, so this
    // swallows their echoes without an event counter.
    if rgba_to_hex(rgba) == *self.resolved.rgba_hex {
      self.hex_input_dirty = false;
      return;
    }

    self.hex_input_dirty = false;
    let next = oklch_from_rgba(rgba, self.value.hue);
    self.set_oklch(next, cx);
    self.sync_hex_input_display(window, cx);
  }
}

impl EventEmitter<ColorPickerEvent> for ColorPickerState {}

#[derive(IntoElement)]
/// Interactive color picker component with popover interface.
///
/// ColorPicker is a complex component that manages a 2D gradient selector for
/// lightness/chroma, plus 1D sliders for hue and alpha. It includes an embedded
/// hex color input field and emits Change events whenever the user modifies
/// any channel. The picker opens in a popover positioned relative to the
/// trigger button.
pub struct ColorPicker {
  id: SharedString,
  state: Entity<ColorPickerState>,
  style: StyleRefinement,
  size: Size,
  disabled: bool,
  variant: ButtonVariant,
  outline: bool,
}

impl ColorPicker {
  /// Creates a color picker trigger button bound to the provided state.
  ///
  /// The `id` uniquely identifies this picker instance. The `state` entity
  /// must outlive the picker and is updated whenever the user changes the
  /// color. Default is non-outlined, Size::Medium, ButtonVariant::Default.
  pub fn new(id: impl Into<SharedString>, state: &Entity<ColorPickerState>) -> Self {
    Self {
      id: id.into(),
      state: state.clone(),
      style: StyleRefinement::default(),
      size: Size::default(),
      disabled: false,
      variant: ButtonVariant::Default,
      outline: false,
    }
  }

  /// Sets the outline style for the trigger button.
  ///
  /// When `true`, renders a border-only button. When `false` (default),
  /// renders a filled button. The button background is always the current
  /// selected color.
  pub fn outline(mut self, outline: bool) -> Self {
    self.outline = outline;
    self
  }
}

impl_disableable!(ColorPicker);
impl_sizable!(ColorPicker);

impl ButtonVariants for ColorPicker {
  fn with_variant(mut self, variant: ButtonVariant) -> Self {
    self.variant = variant;
    self
  }
}

impl_styled!(ColorPicker);

impl RenderOnce for ColorPicker {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // Lazily create the hex input; its display is synced when the value
    // changes (right after `set_oklch`) and when popover content is rebuilt.
    self.state.update(cx, |state, cx| {
      state.ensure_hex_input(window, cx);
    });

    // Reads the value cache maintained by `set_oklch`; this render-hot path
    // performs no color space conversions.
    let resolved = self.state.read(cx).value();
    let state_id = self.state.entity_id();
    let swatch_color = gpui::Rgba {
      r: resolved.rgba.r,
      g: resolved.rgba.g,
      b: resolved.rgba.b,
      a: resolved.rgba.a,
    };
    let trigger = Button::new(("color-picker-trigger", state_id))
      .with_size(self.size)
      .with_variant(self.variant)
      .outline(self.outline)
      .disabled(self.disabled)
      .expand(true)
      .child(
        h_flex()
          .w_full()
          .items_center()
          .justify_between()
          .child(
            h_flex().items_center().gap(px(8.0)).child(
              div()
                .size(px(14.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(Hsla::from(swatch_color)),
            ),
          )
          .child(
            div()
              .text_size(self.size.text_size())
              .font_medium()
              .child(resolved.rgba_hex),
          ),
      )
      .refine_style(&self.style);

    if self.disabled {
      return trigger.into_any_element();
    }

    let state = self.state.clone();
    let size = self.size;

    Popover::new(self.id.clone())
      .anchor(Anchor::TopLeft)
      .trigger(trigger)
      .content(move |_, window, cx| {
        state.update(cx, |state, cx| {
          // Sync the hex echo here so programmatic value changes made outside
          // the picker are picked up when the popover renders; channel drags
          // already sync right after `set_oklch`.
          state.sync_hex_input_display(window, cx);
        });
        let hex_input = state.read(cx).hex_input();
        let mut content = v_flex().w(px(320.0)).gap(px(8.0));

        if let Some(hex_input) = hex_input {
          content = content.child(Input::new(&hex_input));
        }

        for channel in [
          PickerChannel::Lightness,
          PickerChannel::Chroma,
          PickerChannel::Hue,
          PickerChannel::Alpha,
        ] {
          content = content.child(render_channel_row(channel, &state, size, cx));
        }

        content
      })
      .into_any_element()
  }
}

fn render_channel_row(
  channel: PickerChannel, state: &Entity<ColorPickerState>, size: Size, cx: &mut App,
) -> impl IntoElement {
  // Refresh (or reuse) the cached gradient color table for this channel so
  // painting below never re-runs color space conversions.
  let (current_oklch, value, stops) = state.update(cx, |state, _| {
    let current_oklch = state.oklch();
    (
      current_oklch,
      channel.value(current_oklch),
      state.channel_gradient(channel),
    )
  });
  let out_of_gamut = is_out_of_gamut(current_oklch);

  let state_for_bounds = state.clone();
  let state_for_down = state.clone();
  let state_for_move = state.clone();
  let channel_id = ElementId::from(("color-picker-channel", state.entity_id()));
  let ratio = channel.normalized_value(current_oklch);

  v_flex()
    .gap(px(4.0))
    .child(
      h_flex()
        .w_full()
        .justify_between()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(SharedString::from(translate_woocraft(channel.i18n_key())))
        .child(
          h_flex()
            .items_center()
            .gap(px(6.0))
            .child(
              div()
                .text_color(if out_of_gamut {
                  cx.theme().warning
                } else {
                  cx.theme().muted_foreground
                })
                .child(format_channel_value(channel, value)),
            )
            .when(out_of_gamut, |this| {
              this.child(
                div()
                  .text_color(cx.theme().warning)
                  .font_semibold()
                  .child("!"),
              )
            }),
        ),
    )
    .child(
      div()
        .id((channel_id, channel.id_label()))
        .relative()
        .h(size.track_height() + px(8.0))
        .w_full()
        .rounded(cx.theme().radius)
        .overflow_hidden()
        .border_1()
        .border_color(cx.theme().border)
        .on_prepaint(move |bounds, _, cx| {
          state_for_bounds.update(cx, |state, _| {
            state.set_channel_bounds(channel, bounds);
          });
        })
        .on_mouse_down(
          MouseButton::Left,
          move |event: &MouseDownEvent, window, cx| {
            state_for_down.update(cx, |state, cx| {
              state.set_channel_by_position(channel, event.position, window, cx);
            });
            cx.stop_propagation();
          },
        )
        .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
          if event.pressed_button == Some(MouseButton::Left) {
            state_for_move.update(cx, |state, cx| {
              state.set_channel_by_position(channel, event.position, window, cx);
            });
            cx.stop_propagation();
          }
        })
        .child(
          canvas(
            move |_, _, _| {},
            move |bounds, _, window, cx| {
              paint_channel(&stops, ratio, bounds, window, cx);
            },
          )
          .absolute()
          .size_full(),
        ),
    )
}

fn paint_channel(
  stops: &[ChannelGradientStop], ratio: f32, bounds: Bounds<Pixels>, window: &mut Window,
  cx: &mut App,
) {
  let steps = stops.len();

  for (ix, stop) in stops.iter().enumerate() {
    let start_ratio = ix as f32 / steps as f32;
    let end_ratio = (ix + 1) as f32 / steps as f32;

    let x = bounds.origin.x + bounds.size.width * start_ratio;
    let next_x = bounds.origin.x + bounds.size.width * end_ratio;
    let segment_width = (next_x - x).max(px(1.0));
    let segment = Bounds::new(
      point(x, bounds.origin.y),
      size(segment_width, bounds.size.height),
    );

    let color = Hsla::from(gpui::Rgba {
      r: stop.rgba.r,
      g: stop.rgba.g,
      b: stop.rgba.b,
      a: stop.rgba.a,
    });
    window.paint_quad(fill(segment, color));
    if stop.out_of_gamut {
      let warning_segment = Bounds::new(
        point(x, bounds.bottom() - WARNING_LINE_THICKNESS),
        size(segment_width, WARNING_LINE_THICKNESS),
      );
      window.paint_quad(fill(warning_segment, cx.theme().warning));
    }
  }

  let center_x = bounds.origin.x + bounds.size.width * ratio;
  let outer = Bounds::new(
    point(center_x - POINTER_OUTER_WIDTH / 2.0, bounds.origin.y),
    size(POINTER_OUTER_WIDTH, bounds.size.height),
  );
  let inner = Bounds::new(
    point(center_x - POINTER_INNER_WIDTH / 2.0, bounds.origin.y),
    size(POINTER_INNER_WIDTH, bounds.size.height),
  );
  window.paint_quad(fill(outer, cx.theme().background));
  window.paint_quad(fill(inner, cx.theme().foreground));
}

/// Resolves one gradient segment: display color plus gamut warning flag.
fn resolve_gradient_stop(value: ColorPickerOklch) -> ChannelGradientStop {
  let (rgba, out_of_gamut) = resolve_rgba(value);
  ChannelGradientStop { rgba, out_of_gamut }
}

/// Builds the gradient color table for `channel`, sweeping it across its full
/// range while keeping every other oklch component fixed.
fn build_channel_gradient(
  channel: PickerChannel, value: ColorPickerOklch,
) -> Vec<ChannelGradientStop> {
  let mut colors = Vec::with_capacity(GRADIENT_STEPS);
  for step in 0..GRADIENT_STEPS {
    let sample_ratio = (step as f32 + 0.5) / GRADIENT_STEPS as f32;
    let mut sample = value;
    channel.assign(&mut sample, channel.value_from_ratio(sample_ratio));
    colors.push(resolve_gradient_stop(sample));
  }
  colors
}

fn sanitize_oklch(value: ColorPickerOklch) -> ColorPickerOklch {
  ColorPickerOklch {
    lightness: value.lightness.clamp(0.0, 1.0),
    chroma: value.chroma.clamp(0.0, CHROMA_MAX),
    hue: value.hue.clamp(0.0, HUE_MAX),
    alpha: value.alpha.clamp(0.0, 1.0),
  }
}

fn is_out_of_gamut(value: ColorPickerOklch) -> bool {
  convert_oklch(value).1
}

fn fallback_global(value: ColorPickerOklch) -> ColorPickerOklch {
  let mut low = 0.0;
  let mut high = value.chroma;
  let mut best = value;
  best.chroma = 0.0;

  if convert_oklch(best).1 {
    return best;
  }

  for _ in 0..20 {
    let mid = (low + high) * 0.5;
    let mut candidate = value;
    candidate.chroma = mid;
    let out = convert_oklch(candidate).1;
    if out {
      high = mid;
    } else {
      low = mid;
      best = candidate;
    }
  }

  best
}

fn convert_oklch(value: ColorPickerOklch) -> (Srgb, bool) {
  let oklch = Oklch::new(
    value.lightness,
    value.chroma,
    OklabHue::from_degrees(value.hue),
  );
  let raw_rgb: Srgb = Srgb::from_color(oklch);

  let rgb_out_of_range = raw_rgb.red < 0.0
    || raw_rgb.red > 1.0
    || raw_rgb.green < 0.0
    || raw_rgb.green > 1.0
    || raw_rgb.blue < 0.0
    || raw_rgb.blue > 1.0;

  let raw_hsl: Hsl = Hsl::from_color(raw_rgb);
  let saturation = raw_hsl.saturation;
  let lightness = raw_hsl.lightness;
  let hue = raw_hsl.hue.into_degrees();
  let hue_out_of_range = !hue.is_finite() && saturation > 0.000_01;
  let hsl_out_of_range = hue_out_of_range
    || !saturation.is_finite()
    || !lightness.is_finite()
    || !(0.0..=1.0).contains(&saturation)
    || !(0.0..=1.0).contains(&lightness);

  (raw_rgb, rgb_out_of_range || hsl_out_of_range)
}

/// Converts `value` to a displayable sRGB color, bisecting chroma toward the
/// nearest in-gamut color when the requested color is out of gamut. Returns
/// the clamped RGBA plus whether the requested color was out of gamut.
fn resolve_rgba(value: ColorPickerOklch) -> (ColorPickerRgba, bool) {
  let value = sanitize_oklch(value);
  let (_, out_of_gamut) = convert_oklch(value);
  let fallback = if out_of_gamut {
    fallback_global(value)
  } else {
    value
  };

  let (raw_rgb, _) = convert_oklch(fallback);
  let rgba = ColorPickerRgba {
    r: raw_rgb.red.clamp(0.0, 1.0),
    g: raw_rgb.green.clamp(0.0, 1.0),
    b: raw_rgb.blue.clamp(0.0, 1.0),
    a: fallback.alpha.clamp(0.0, 1.0),
  };
  (rgba, out_of_gamut)
}

fn resolve_value(value: ColorPickerOklch) -> ColorPickerValue {
  let value = sanitize_oklch(value);
  let (clamped, _) = resolve_rgba(value);

  let hsl: Hsl = Hsl::from_color(Srgb::new(clamped.r, clamped.g, clamped.b));
  let hsla = ColorPickerHsla {
    hue: normalize_degrees(hsl.hue.into_degrees()),
    saturation: hsl.saturation.clamp(0.0, 1.0),
    lightness: hsl.lightness.clamp(0.0, 1.0),
    alpha: clamped.a,
  };

  ColorPickerValue {
    rgba_hex: rgba_to_hex(clamped).into(),
    rgba: clamped,
    oklch: value,
    hsla,
  }
}

fn normalize_degrees(value: f32) -> f32 {
  let mut value = value % 360.0;
  if value < 0.0 {
    value += 360.0;
  }
  value
}

fn format_channel_value(channel: PickerChannel, value: f32) -> String {
  match channel {
    PickerChannel::Hue => format!("{value:.0} {}", translate_woocraft("color_picker.hue_unit")),
    _ => format!("{value:.3}"),
  }
}

fn hex_digits(input: &str) -> &str {
  let input = input.trim();
  input.strip_prefix('#').unwrap_or(input)
}

fn is_hex_digits(text: &str) -> bool {
  !text.is_empty() && text.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_complete_eight_hex_input(input: &str) -> bool {
  let hex = hex_digits(input);
  hex.len() == 8 && is_hex_digits(hex)
}

fn is_enter_committable_hex_input(input: &str) -> bool {
  let hex = hex_digits(input);
  matches!(hex.len(), 3 | 4 | 6 | 8) && is_hex_digits(hex)
}

fn parse_hex_color(input: &str) -> Option<ColorPickerRgba> {
  let hex = hex_digits(input);
  if hex.is_empty() {
    return None;
  }

  let (r, g, b, a) = match hex.len() {
    3 => {
      let mut chars = hex.chars();
      let r = chars.next()?;
      let g = chars.next()?;
      let b = chars.next()?;
      (
        parse_hex_byte_pair(r, r)?,
        parse_hex_byte_pair(g, g)?,
        parse_hex_byte_pair(b, b)?,
        255,
      )
    }
    4 => {
      let mut chars = hex.chars();
      let r = chars.next()?;
      let g = chars.next()?;
      let b = chars.next()?;
      let a = chars.next()?;
      (
        parse_hex_byte_pair(r, r)?,
        parse_hex_byte_pair(g, g)?,
        parse_hex_byte_pair(b, b)?,
        parse_hex_byte_pair(a, a)?,
      )
    }
    6 => (
      parse_hex_byte(&hex[0..2])?,
      parse_hex_byte(&hex[2..4])?,
      parse_hex_byte(&hex[4..6])?,
      255,
    ),
    8 => (
      parse_hex_byte(&hex[0..2])?,
      parse_hex_byte(&hex[2..4])?,
      parse_hex_byte(&hex[4..6])?,
      parse_hex_byte(&hex[6..8])?,
    ),
    _ => return None,
  };

  Some(ColorPickerRgba {
    r: r as f32 / 255.0,
    g: g as f32 / 255.0,
    b: b as f32 / 255.0,
    a: a as f32 / 255.0,
  })
}

fn parse_complete_eight_hex_color(input: &str) -> Option<ColorPickerRgba> {
  // Only the full 8-digit RGBA form is accepted here; the length check keeps
  // shorter (3/6-digit) inputs from being resolved by `parse_hex_color`.
  if hex_digits(input).len() != 8 {
    return None;
  }
  parse_hex_color(input)
}

fn parse_hex_byte(input: &str) -> Option<u8> {
  u8::from_str_radix(input, 16).ok()
}

fn parse_hex_byte_pair(left: char, right: char) -> Option<u8> {
  let mut text = [0u8; 2];
  text[0] = left as u8;
  text[1] = right as u8;
  let text = std::str::from_utf8(&text).ok()?;
  parse_hex_byte(text)
}

fn oklch_from_rgba(rgba: ColorPickerRgba, fallback_hue: f32) -> ColorPickerOklch {
  let rgb = Srgb::new(rgba.r, rgba.g, rgba.b);
  let oklch: Oklch = Oklch::from_color(rgb);
  let hue = oklch.hue.into_degrees();
  ColorPickerOklch {
    lightness: oklch.l,
    chroma: oklch.chroma,
    hue: if hue.is_finite() {
      normalize_degrees(hue)
    } else {
      normalize_degrees(fallback_hue)
    },
    alpha: rgba.a,
  }
}

fn rgba_to_hex(rgba: ColorPickerRgba) -> String {
  let r = (rgba.r.clamp(0.0, 1.0) * 255.0).round() as u8;
  let g = (rgba.g.clamp(0.0, 1.0) * 255.0).round() as u8;
  let b = (rgba.b.clamp(0.0, 1.0) * 255.0).round() as u8;
  let a = (rgba.a.clamp(0.0, 1.0) * 255.0).round() as u8;
  format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
}

#[cfg(test)]
mod tests {
  use gpui::TestAppContext;

  use super::*;

  fn sample_oklch() -> ColorPickerOklch {
    ColorPickerOklch {
      lightness: 0.64,
      chroma: 0.17,
      hue: 248.0,
      alpha: 1.0,
    }
  }

  fn gradient_channels() -> [PickerChannel; 4] {
    [
      PickerChannel::Lightness,
      PickerChannel::Chroma,
      PickerChannel::Hue,
      PickerChannel::Alpha,
    ]
  }

  #[test]
  fn parse_hex_color_supports_short_and_long_forms() {
    let white = parse_hex_color("#FFFFFFFF").expect("8-digit hex parses");
    assert_eq!(
      white,
      ColorPickerRgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0
      }
    );

    let black = parse_hex_color("000").expect("3-digit hex parses");
    assert_eq!(
      black,
      ColorPickerRgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0
      }
    );

    assert!(parse_hex_color("11223344").is_some());
    assert!(parse_hex_color("11223").is_none(), "wrong digit count");
    assert!(parse_hex_color("gggggggg").is_none(), "invalid digits");
  }

  #[test]
  fn parse_complete_eight_hex_color_requires_full_rgba() {
    assert!(parse_complete_eight_hex_color("#AABBCCDD").is_some());
    assert!(parse_complete_eight_hex_color("aabbccdd").is_some());
    assert!(parse_complete_eight_hex_color("#AABBCC").is_none());
    assert!(parse_complete_eight_hex_color("AABBCCDDA").is_none());
    assert!(parse_complete_eight_hex_color("AABBCCGG").is_none());
  }

  #[test]
  fn canonical_hex_round_trips_through_parse() {
    // The hex echo guard in `on_hex_input_event` relies on programmatic sync
    // values parsing back to the exact same RGBA components.
    for byte in [0u8, 1, 17, 127, 128, 200, 254, 255] {
      let value = byte as f32 / 255.0;
      let rgba = ColorPickerRgba {
        r: value,
        g: value,
        b: value,
        a: value,
      };
      let parsed = parse_hex_color(&rgba_to_hex(rgba)).expect("canonical hex parses");
      assert_eq!(parsed, rgba);
    }
  }

  #[test]
  fn gradient_cache_key_tracks_dependency_components_only() {
    let base = sample_oklch();

    // Sweeping the channel's own component keeps the key stable.
    for channel in gradient_channels() {
      let mut swept = base;
      channel.assign(&mut swept, channel.value_from_ratio(0.75));
      assert_eq!(
        channel.gradient_cache_key(base),
        channel.gradient_cache_key(swept),
        "{channel:?} key must ignore its own component"
      );
    }

    // Any other component change invalidates the key.
    for channel in gradient_channels() {
      for other in gradient_channels() {
        if other == channel {
          continue;
        }
        let mut changed = base;
        other.assign(&mut changed, other.value_from_ratio(0.75));
        assert_ne!(
          channel.gradient_cache_key(base),
          channel.gradient_cache_key(changed),
          "{channel:?} key must depend on {other:?}"
        );
      }
    }
  }

  #[test]
  fn channel_gradient_cache_reuses_matching_tables() {
    let mut state = ColorPickerState::new();

    let first = state.channel_gradient(PickerChannel::Lightness);
    let second = state.channel_gradient(PickerChannel::Lightness);
    assert!(
      Arc::ptr_eq(&first, &second),
      "unchanged value reuses the table"
    );

    // The lightness gradient depends on chroma, so it must rebuild.
    state.value.chroma += 0.01;
    let third = state.channel_gradient(PickerChannel::Lightness);
    assert!(!Arc::ptr_eq(&first, &third));

    // Moving the swept component itself must not invalidate the table.
    let chroma_table = state.channel_gradient(PickerChannel::Chroma);
    state.value.lightness += 0.01;
    assert!(Arc::ptr_eq(
      &third,
      &state.channel_gradient(PickerChannel::Lightness)
    ));
    assert!(!Arc::ptr_eq(
      &chroma_table,
      &state.channel_gradient(PickerChannel::Chroma)
    ));
  }

  #[test]
  fn alpha_gradient_spans_full_range() {
    let colors = build_channel_gradient(PickerChannel::Alpha, sample_oklch());
    assert_eq!(colors.len(), GRADIENT_STEPS);
    assert!(colors.first().unwrap().rgba.a < 0.1);
    assert!((colors.last().unwrap().rgba.a - 1.0).abs() < 0.05);
  }

  #[test]
  fn hue_gradient_of_gray_is_uniform() {
    let gray = ColorPickerOklch {
      lightness: 0.5,
      chroma: 0.0,
      hue: 0.0,
      alpha: 1.0,
    };
    let colors = build_channel_gradient(PickerChannel::Hue, gray);
    assert_eq!(colors.len(), GRADIENT_STEPS);
    assert!(colors.iter().all(|stop| stop.rgba == colors[0].rgba));
    assert!(colors.iter().all(|stop| !stop.out_of_gamut));
  }

  #[gpui::test]
  fn set_oklch_keeps_resolved_cache_consistent(cx: &mut TestAppContext) {
    let state = cx.new(|_| ColorPickerState::new());
    cx.update(|app| {
      assert_eq!(
        state.read(app).value(),
        resolve_value(ColorPickerOklch::default())
      );

      let next = ColorPickerOklch {
        lightness: 0.8,
        chroma: 0.1,
        hue: 90.0,
        alpha: 0.5,
      };
      state.update(app, |state, cx| state.set_oklch(next, cx));

      let resolved = state.read(app).value();
      assert_eq!(resolved.oklch, next);
      // The cached value must stay consistent with a fresh resolve.
      assert_eq!(resolved, resolve_value(next));
    });
  }
}
