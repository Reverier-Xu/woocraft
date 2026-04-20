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
//! use gpuim::{App, Context};
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

use gpuim::{
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
const GRADIENT_MIN_STEPS: usize = 64;
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
  channel_bounds: [Bounds<Pixels>; 4],
  hex_input: Option<Entity<InputState>>,
  pending_programmatic_hex_events: usize,
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
    Self {
      value: ColorPickerOklch::default(),
      channel_bounds: [Bounds::default(); 4],
      hex_input: None,
      pending_programmatic_hex_events: 0,
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
    cx.emit(ColorPickerEvent::Change(self.value()));
    cx.notify();
  }

  /// Returns the current color in Oklch color space.
  pub fn oklch(&self) -> ColorPickerOklch {
    self.value
  }

  /// Returns the complete current color with all color space representations.
  ///
  /// Includes RGBA hex (8-digit), RGBA normalized values, Oklch, and HSLA.
  pub fn value(&self) -> ColorPickerValue {
    resolve_value(self.value)
  }

  /// Returns the opt hex input field entity (if it has been created).
  pub fn hex_input(&self) -> Option<Entity<InputState>> {
    self.hex_input.clone()
  }

  fn ensure_hex_input(&mut self, cx: &mut Context<Self>) -> Entity<InputState> {
    if let Some(hex_input) = &self.hex_input {
      return hex_input.clone();
    }

    let initial_hex = self.value().rgba_hex.to_string();
    let hex_input = cx.new(|cx| {
      InputState::new(cx)
        .placeholder(translate_woocraft("color_picker.hex_placeholder"))
        .default_value(initial_hex.clone())
    });
    let subscription = cx.subscribe(&hex_input, Self::on_hex_input_event);
    self.hex_input = Some(hex_input.clone());
    self._hex_input_subscription = Some(subscription);
    hex_input
  }

  pub fn sync_hex_input_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let hex_input = self.ensure_hex_input(cx);
    let expected = self.value().rgba_hex.to_string();
    let current = hex_input.read(cx).value().to_string();
    if current == expected {
      self.hex_input_dirty = false;
      return;
    }
    // Preserve raw text while user is still editing.
    if self.hex_input_dirty {
      return;
    }

    self.pending_programmatic_hex_events = self.pending_programmatic_hex_events.saturating_add(1);
    hex_input.update(cx, |input, cx| {
      input.set_value(expected.clone(), window, cx);
    });
  }

  fn set_channel_bounds(&mut self, channel: PickerChannel, bounds: Bounds<Pixels>) {
    self.channel_bounds[channel.index()] = bounds;
  }

  fn set_channel_by_position(
    &mut self, channel: PickerChannel, position: Point<Pixels>, cx: &mut Context<Self>,
  ) {
    let bounds = self.channel_bounds[channel.index()];
    if bounds.size.width <= px(0.0) {
      return;
    }

    let ratio = ((position.x - bounds.left()) / bounds.size.width).clamp(0.0, 1.0);
    let mut next = self.value;
    channel.assign(&mut next, channel.value_from_ratio(ratio));
    self.set_oklch(next, cx);
  }

  fn on_hex_input_event(
    &mut self, state: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>,
  ) {
    let input = state.read(cx).value().to_string();
    let parsed = match event {
      InputEvent::Change => {
        if self.pending_programmatic_hex_events > 0 {
          self.pending_programmatic_hex_events -= 1;
          return;
        }
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

    self.hex_input_dirty = false;
    let next = oklch_from_rgba(rgba, self.value.hue);
    self.set_oklch(next, cx);
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
    self.state.update(cx, |state, cx| {
      state.sync_hex_input_display(window, cx);
    });

    let resolved = self.state.read(cx).value();
    let state_id = self.state.entity_id();
    let swatch_color = gpuim::Rgba {
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
      .content(move |_, _, cx| {
        let hex_input = state.read(cx).hex_input();
        let content = v_flex().w(px(320.0)).gap(px(8.0));

        let content = if let Some(hex_input) = hex_input {
          content.child(Input::new(&hex_input))
        } else {
          content
        };

        content
          .child(render_channel_row(
            PickerChannel::Lightness,
            &state,
            size,
            cx,
          ))
          .child(render_channel_row(PickerChannel::Chroma, &state, size, cx))
          .child(render_channel_row(PickerChannel::Hue, &state, size, cx))
          .child(render_channel_row(PickerChannel::Alpha, &state, size, cx))
      })
      .into_any_element()
  }
}

fn render_channel_row(
  channel: PickerChannel, state: &Entity<ColorPickerState>, size: Size, cx: &mut App,
) -> impl IntoElement {
  let (current_oklch, value) = {
    let state = state.read(cx);
    let current_oklch = state.oklch();
    (current_oklch, channel.value(current_oklch))
  };
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
        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
          state_for_down.update(cx, |state, cx| {
            state.set_channel_by_position(channel, event.position, cx);
          });
          cx.stop_propagation();
        })
        .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
          if event.pressed_button == Some(MouseButton::Left) {
            state_for_move.update(cx, |state, cx| {
              state.set_channel_by_position(channel, event.position, cx);
            });
            cx.stop_propagation();
          }
        })
        .child(
          canvas(
            move |_, _, _| {},
            move |bounds, _, window, cx| {
              paint_channel(channel, current_oklch, ratio, bounds, window, cx);
            },
          )
          .absolute()
          .size_full(),
        ),
    )
}

fn paint_channel(
  channel: PickerChannel, oklch: ColorPickerOklch, ratio: f32, bounds: Bounds<Pixels>,
  window: &mut Window, cx: &mut App,
) {
  let width = f32::from(bounds.size.width).max(1.0);
  let steps = (width.ceil() as usize).max(GRADIENT_MIN_STEPS);

  for ix in 0..steps {
    let start_ratio = ix as f32 / steps as f32;
    let end_ratio = (ix + 1) as f32 / steps as f32;
    let sample_ratio = (start_ratio + end_ratio) * 0.5;
    let mut sample = oklch;
    channel.assign(&mut sample, channel.value_from_ratio(sample_ratio));
    let resolved = resolve_value(sample);

    let x = bounds.origin.x + bounds.size.width * start_ratio;
    let next_x = bounds.origin.x + bounds.size.width * end_ratio;
    let segment_width = (next_x - x).max(px(1.0));
    let segment = Bounds::new(
      point(x, bounds.origin.y),
      size(segment_width, bounds.size.height),
    );

    let color = Hsla::from(gpuim::Rgba {
      r: resolved.rgba.r,
      g: resolved.rgba.g,
      b: resolved.rgba.b,
      a: resolved.rgba.a,
    });
    let out_of_gamut = is_out_of_gamut(sample);
    window.paint_quad(fill(segment, color));
    if out_of_gamut {
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

fn resolve_value(value: ColorPickerOklch) -> ColorPickerValue {
  let value = sanitize_oklch(value);
  let (_, out_of_gamut) = convert_oklch(value);
  let fallback = if out_of_gamut {
    fallback_global(value)
  } else {
    value
  };

  let (raw_rgb, _) = convert_oklch(fallback);
  let clamped = ColorPickerRgba {
    r: raw_rgb.red.clamp(0.0, 1.0),
    g: raw_rgb.green.clamp(0.0, 1.0),
    b: raw_rgb.blue.clamp(0.0, 1.0),
    a: fallback.alpha.clamp(0.0, 1.0),
  };

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
  let hex = hex_digits(input);
  if hex.len() != 8 || !is_hex_digits(hex) {
    return None;
  }

  Some(ColorPickerRgba {
    r: parse_hex_byte(&hex[0..2])? as f32 / 255.0,
    g: parse_hex_byte(&hex[2..4])? as f32 / 255.0,
    b: parse_hex_byte(&hex[4..6])? as f32 / 255.0,
    a: parse_hex_byte(&hex[6..8])? as f32 / 255.0,
  })
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
