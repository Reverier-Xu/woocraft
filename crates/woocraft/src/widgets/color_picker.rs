//! Advanced color picker with multiple color space support.
//!
//! ColorPicker provides a comprehensive color selection interface that supports
//! multiple color spaces: HCT (hue, chroma, tone — the design system's own),
//! RGBA, and HSLA. The picker exposes 1D sliders for tone, chroma, hue and
//! alpha, each painted with the gradient it selects from.
//!
//! # Features
//! - **Multi-space support**: HCT (the internal space), RGBA, HSLA
//! - **Hex editor**: Type or paste hex color codes directly (8-digit RGB hex)
//! - **2D gradient picker**: Visual lightness/chroma selection with interactive
//!   gradient
//! - **1D sliders**: Dedicated sliders for hue and alpha channels
//! - **Unbounded dragging**: A channel keeps following the pointer once a drag
//!   starts, including outside the track and outside the window
//! - **Intelligent conversion**: Seamless conversion between color spaces
//!   preserving hue on RGB changes
//! - **Popover trigger**: Integrated popover button for space-efficient UI
//!
//! # Example
//! ```rust,ignore
//! use gpui::{App, Context};
//! use woocraft::{ColorPickerState, ColorPicker, ColorPickerHct};
//!
//! let color_picker_state = cx.new(|cx| ColorPickerState::new());
//!
//! let picker = ColorPicker::new("my-color-picker", &color_picker_state)
//!   .outline(false);
//! ```
//!
//! # Internals
//! The picker uses HCT as the internal representation, matching the rest of the
//! design system: hue and chroma come from CAM16, so equal steps look equal and
//! hue holds when chroma changes, while tone is CIE L\*, so a tone difference
//! is a contrast difference. RGB values and hex input are converted to HCT on
//! change, with hue preservation when the color is neutral.

use std::sync::Arc;

use gpui::{
  App, AppContext as _, Bounds, Context, Corners, DispatchPhase, ElementId, Entity, EventEmitter,
  Hsla, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
  MouseUpEvent, PaintQuad, ParentElement, Pixels, Point, Rems, RenderOnce, SharedString,
  StyleRefinement, Styled, Subscription, TextAlign, Window, canvas, div, fill, point, px, rems,
  size,
};
use woocraft_colors::{Cam16, Hct, Tone, ViewingConditions};

use crate::{
  ActiveTheme, Anchor, Button, ButtonVariant, ButtonVariants, Disableable, ElementExt, Input,
  InputEvent, InputState, Popover, Sizable, Size, StyledExt, contrasting_neutral, h_flex,
  translate_woocraft, v_flex,
};

/// Upper bound of the chroma slider.
///
/// HCT chroma is unbounded in principle but the sRGB gamut caps it at roughly
/// 130, so a slider that reaches past that can always express everything a
/// display can show. Requests above the gamut are reduced by the solver.
const CHROMA_MAX: f32 = 130.0;
const HUE_MAX: f32 = 360.0;
/// Upper bound of the tone slider: CIE L\* runs 0 (black) to 100 (white).
const TONE_MAX: f32 = 100.0;
const WARNING_LINE_THICKNESS: Rems = rems(0.125);
/// Gradient segments per channel slider.
///
/// A slider is a 1-D gradient, so its fidelity is bounded by how many segments
/// it is cut into. The table is cached per (channel, value) and rebuilt only
/// when the value changes, so a high count costs one-time work rather than
/// per-frame work: 512 segments read as a continuous gradient at the popover's
/// width, where the previous 96 showed visible banding once the picker grew.
const GRADIENT_STEPS: usize = 512;
/// Width of the indicator's coloured bar.
const POINTER_WIDTH: Rems = rems(0.125);
/// Inset of the indicator from the track's top and bottom.
///
/// The indicator as a whole — bar plus outline — is `track height - 0.25em`
/// tall and vertically centred, so it clears the track's edges by this much on
/// both sides.
const POINTER_INSET: Rems = rems(0.125);
/// Width of the indicator's outline.
///
/// The outline is drawn just outside the [`POINTER_WIDTH`] bar rather than
/// eating into it: at the medium size 0.125em is 2px, so a border-box outline
/// would leave nothing of the track colour the bar is meant to show.
const POINTER_OUTLINE: Pixels = px(1.);
/// Corner radius of the indicator.
///
/// Both the bar and its outline are rounded by this much. It is clamped per
/// quad to half the shortest side, so the pill shape never degenerates however
/// small the track is.
const POINTER_RADIUS: Rems = rems(0.0625);
/// How far past the two ends of the track the end colors are painted.
///
/// The track clips its content to its rounded rect, so this padding is simply
/// cut away; it exists so the first and last gradient segments still paint a
/// full-width sliver instead of being eaten by the rounding, which would leave
/// a sliver of background between the indicator and the end color.
const TRACK_EDGE_PADDING: Rems = rems(0.25);

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

/// HCT color representation for the color picker (the internal space).
///
/// HCT is what the design system reasons in, so the picker speaks it too: a
/// color is a hue, a chroma and a tone, and tone is the axis contrast is
/// measured on.
/// - `hue`: CAM16 hue in degrees (0.0..=360.0)
/// - `chroma`: CAM16 chroma (0.0 = grey; the sRGB gamut bounds it at roughly
///   130 depending on hue and tone)
/// - `tone`: CIE L* (0.0 = black, 100.0 = white). Unlike a perceptual
///   lightness, equal tone steps are equal contrast steps.
/// - `alpha`: Transparency (0.0 = transparent, 1.0 = opaque)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorPickerHct {
  pub hue: f32,
  pub chroma: f32,
  pub tone: f32,
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
  pub hct: ColorPickerHct,
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
  Tone,
  Chroma,
  Hue,
  Alpha,
}

impl PickerChannel {
  fn index(self) -> usize {
    match self {
      Self::Tone => 0,
      Self::Chroma => 1,
      Self::Hue => 2,
      Self::Alpha => 3,
    }
  }

  fn id_label(self) -> &'static str {
    match self {
      Self::Tone => "Tone",
      Self::Chroma => "Chroma",
      Self::Hue => "Hue",
      Self::Alpha => "Alpha",
    }
  }

  fn i18n_key(self) -> &'static str {
    match self {
      Self::Tone => "color_picker.tone",
      Self::Chroma => "color_picker.chroma",
      Self::Hue => "color_picker.hue",
      Self::Alpha => "color_picker.alpha",
    }
  }

  fn range(self) -> (f32, f32) {
    match self {
      Self::Tone => (0.0, TONE_MAX),
      Self::Chroma => (0.0, CHROMA_MAX),
      Self::Hue => (0.0, HUE_MAX),
      Self::Alpha => (0.0, 1.0),
    }
  }

  fn value(self, color: ColorPickerHct) -> f32 {
    match self {
      Self::Tone => color.tone,
      Self::Chroma => color.chroma,
      Self::Hue => color.hue,
      Self::Alpha => color.alpha,
    }
  }

  fn assign(self, color: &mut ColorPickerHct, value: f32) {
    let (min, max) = self.range();
    let value = value.clamp(min, max);
    match self {
      Self::Tone => color.tone = value,
      Self::Chroma => color.chroma = value,
      Self::Hue => color.hue = value,
      Self::Alpha => color.alpha = value,
    }
  }

  fn normalized_value(self, color: ColorPickerHct) -> f32 {
    let (min, max) = self.range();
    let span = (max - min).max(f32::EPSILON);
    ((self.value(color) - min) / span).clamp(0.0, 1.0)
  }

  fn value_from_ratio(self, ratio: f32) -> f32 {
    let (min, max) = self.range();
    min + (max - min) * ratio.clamp(0.0, 1.0)
  }

  /// Cache key for this channel's gradient table: the HCT components the
  /// gradient depends on, with the channel's own component zeroed because the
  /// gradient sweeps it across the full range.
  fn gradient_cache_key(self, mut color: ColorPickerHct) -> ColorPickerHct {
    self.assign(&mut color, 0.0);
    color
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
  /// HCT snapshot the table was built from, as produced by
  /// [`PickerChannel::gradient_cache_key`].
  key: ColorPickerHct,
  colors: Arc<[ChannelGradientStop]>,
}

impl Default for ColorPickerHct {
  /// A medium-saturation blue at body-text tone.
  fn default() -> Self {
    Self {
      hue: 248.0,
      chroma: 48.0,
      tone: 45.0,
      alpha: 1.0,
    }
  }
}

/// State management for the color picker.
///
/// Holds the current color value in HCT space, manages
/// internal channel sliders, and maintains the hex input field state.
/// Emits `ColorPickerEvent::Change` whenever the color changes.
pub struct ColorPickerState {
  value: ColorPickerHct,
  /// Fully resolved value cache, kept in sync with `value` by `set_hct` and
  /// `default_value` so renders never re-run color space conversions.
  resolved: ColorPickerValue,
  channel_bounds: [Bounds<Pixels>; 4],
  /// Channel currently following a pressed pointer, if any.
  dragging: Option<PickerChannel>,
  /// The plane on display.
  ///
  /// Follows whichever slider was dragged last and stays there once the drag
  /// ends: resetting to hue/chroma would throw away the view the user just
  /// navigated to, and would resample a bitmap they are still reading.
  plane_kind: PlaneKind,
  /// Per-channel gradient color tables (see [`Self::channel_gradient`]).
  gradient_cache: [Option<ChannelGradient>; 4],
  /// The 2-D planes (see [`Self::plane`]), one slot per kind.
  ///
  /// Keyed by the plane's fixed axis. A single shared slot would throw the
  /// bitmap away every time a drag moved to a different slider, which is
  /// exactly when the user is looking at it.
  plane_cache: [Option<(u32, Arc<Plane>)>; 3],
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
  /// Creates a new color picker state with a default blue color.
  ///
  /// Default color is a medium-saturation blue (h=248°, c=0.17, l=0.64, a=1.0).
  pub fn new() -> Self {
    let value = ColorPickerHct::default();
    Self {
      value,
      resolved: resolve_value(value),
      channel_bounds: [Bounds::default(); 4],
      dragging: None,
      plane_kind: PlaneKind::HueChroma,
      gradient_cache: [None, None, None, None],
      plane_cache: [None, None, None],
      hex_input: None,
      hex_input_dirty: false,
      _hex_input_subscription: None,
    }
  }

  /// Sets the initial color value in HCT color space.
  ///
  /// Call before rendering to start with a specific color. Values are clamped
  /// to valid ranges (lightness 0..1, chroma 0..0.4, hue 0..360, alpha 0..1).
  pub fn default_value(mut self, value: ColorPickerHct) -> Self {
    self.value = sanitize_hct(value);
    self.resolved = resolve_value(self.value);
    self
  }

  /// Updates the color to the specified HCT value, emitting Change event.
  ///
  /// Use this to programmatically set the color (e.g., from external input).
  /// If the value hasn't changed, no event is emitted. All values are clamped
  /// to valid ranges.
  pub fn set_hct(&mut self, value: ColorPickerHct, cx: &mut Context<Self>) {
    let next = sanitize_hct(value);
    if self.value == next {
      return;
    }
    self.value = next;
    self.resolved = resolve_value(next);
    cx.emit(ColorPickerEvent::Change(self.resolved.clone()));
    cx.notify();
  }

  /// Returns the current color in HCT color space.
  pub fn hct(&self) -> ColorPickerHct {
    self.value
  }

  /// Returns the complete current color with all color space representations.
  ///
  /// Includes RGBA hex (8-digit), RGBA normalized values, HCT, and HSLA.
  /// Clones the cache maintained by [`Self::set_hct`] instead of re-running
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
  /// an HCT component the gradient depends on has changed.
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

  /// The 2-D plane to show, and the data for it.
  ///
  /// Which plane is shown follows the slider being dragged: the plane explains
  /// the two axes that slider does *not* control, so it answers "where am I"
  /// for the axis the user is currently moving. Between drags it falls back to
  /// hue/chroma, which is the pair a colour picker is usually asked about.
  ///
  /// Sampling costs tens of thousands of CAM16 conversions, so the result is
  /// cached against everything it depends on. A drag only changes one axis, and
  /// that axis is one the plane holds fixed — so the cache survives the whole
  /// gesture and the bitmap is built once.
  fn plane(&mut self) -> (PlaneKind, Arc<Plane>) {
    // A drag switches the view to the plane of the axes it is not changing; the
    // view then stays put until another slider is dragged.
    if let Some(channel) = self.dragging {
      self.plane_kind = PlaneKind::for_channel(channel);
    }
    let kind = self.plane_kind;

    // The bitmap is a slice through the colour space, so it depends only on the
    // axis the plane holds *fixed*: that is the axis a drag would move which
    // must invalidate it. The two drawn axes are swept by the plane itself, so
    // dragging either of them reuses the bitmap.
    let fixed = kind.fixed_axis();
    let fixed_value = match fixed {
      PlaneAxis::Hue => self.value.hue,
      PlaneAxis::Chroma => self.value.chroma,
      PlaneAxis::Tone => self.value.tone,
    };
    let key = fixed_value.to_bits();
    let slot = match kind {
      PlaneKind::HueChroma => 0,
      PlaneKind::HueTone => 1,
      PlaneKind::ChromaTone => 2,
    };

    match &self.plane_cache[slot] {
      Some((cached, plane)) if *cached == key => (kind, plane.clone()),
      _ => {
        let plane: Arc<Plane> = Arc::new(Plane::sample(kind, self.value));
        self.plane_cache[slot] = Some((key, plane.clone()));
        (kind, plane)
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
    let rem_size = window.rem_size();
    let travel = channel_pointer_travel(bounds, rem_size);
    if travel.size.width <= px(0.) {
      return;
    }

    let ratio = ((position.x - travel.left()) / travel.size.width).clamp(0.0, 1.0);
    let mut next = self.value;
    channel.assign(&mut next, channel.value_from_ratio(ratio));
    self.set_hct(next, cx);
    self.sync_hex_input_display(window, cx);
  }

  /// Starts a channel gesture: the channel takes the pressed pointer at
  /// `position` and keeps following it until the button is released, wherever
  /// the pointer goes.
  fn begin_channel_drag(
    &mut self, channel: PickerChannel, position: Point<Pixels>, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.dragging = Some(channel);
    self.set_channel_by_position(channel, position, window, cx);
    // Notify unconditionally: the rebuilt track must know it owns the gesture
    // even when the value itself did not change.
    cx.notify();
  }

  /// Steers the dragged channel to `position`.
  ///
  /// The position may lie outside the track — including outside the window —
  /// in which case the value saturates at the corresponding end of the range.
  /// Moves for a channel that does not own the gesture are ignored, so a stale
  /// drag can never steer another channel.
  fn drag_channel_to(
    &mut self, channel: PickerChannel, position: Point<Pixels>, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if self.dragging != Some(channel) {
      return;
    }

    self.set_channel_by_position(channel, position, window, cx);
  }

  /// Ends the channel gesture, if one is in flight.
  fn end_channel_drag(&mut self, cx: &mut Context<Self>) {
    if self.dragging.take().is_some() {
      cx.notify();
    }
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
    let next = hct_from_rgba(rgba, self.value.hue);
    self.set_hct(next, cx);
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
    // changes (right after `set_hct`) and when popover content is rebuilt.
    self.state.update(cx, |state, cx| {
      state.ensure_hex_input(window, cx);
    });

    // Reads the value cache maintained by `set_hct`; this render-hot path
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
            h_flex().items_center().gap(rems(0.5)).child(
              div()
                .size(rems(0.875))
                .rounded(rems(0.25))
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
          // already sync right after `set_hct`.
          state.sync_hex_input_display(window, cx);
        });
        let hex_input = state.read(cx).hex_input();
        let mut content = v_flex().gap(rems(0.5));

        if let Some(hex_input) = hex_input {
          content = content.child(Input::new(&hex_input));
        }

        let (current, kind, plane) = state.update(cx, |state, _| {
          let current = state.hct();
          let (kind, plane) = state.plane();
          (current, kind, plane)
        });
        let mut sliders = v_flex().flex_1().gap(rems(0.5));
        for channel in [
          PickerChannel::Tone,
          PickerChannel::Chroma,
          PickerChannel::Hue,
          PickerChannel::Alpha,
        ] {
          sliders = sliders.child(render_channel_row(channel, &state, size, cx));
        }

        // The plane is a fixed square; the sliders take the rest of the width.
        // `items_start` keeps the plane at its own height instead of letting
        // the row stretch it into a sliver. The width is in em for the same
        // reason the plane is: it has to scale with the component size.
        content.w(size.em(29.)).child(
          h_flex()
            .w_full()
            .items_start()
            .gap(rems(0.75))
            .child(sliders)
            .child(render_plane(current, kind, plane, size, cx)),
        )
      })
      .into_any_element()
  }
}

/// Which two axes a plane shows.
///
/// The picker has four sliders; each pair of them spans a plane. Showing one
/// plane at a time keeps the popover readable — three 2-D charts next to four
/// sliders is a wall of colour — while still answering "where am I" for
/// whichever axis the user is currently moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaneKind {
  /// Hue across, chroma up.
  HueChroma,
  /// Hue across, tone up.
  HueTone,
  /// Chroma across, tone up.
  ChromaTone,
}

impl PlaneKind {
  /// The plane to show while `channel` is being dragged.
  ///
  /// It is the plane that holds the dragged axis *fixed* and draws the other
  /// two. Two reasons: it shows where the value sits within the dimensions the
  /// drag is not changing, and — because the bitmap only depends on the fixed
  /// axis — the whole gesture reuses one bitmap instead of rebuilding it on
  /// every move.
  fn for_channel(channel: PickerChannel) -> Self {
    match channel {
      PickerChannel::Tone => Self::HueChroma,
      PickerChannel::Chroma => Self::HueTone,
      PickerChannel::Hue => Self::ChromaTone,
      // Alpha has no plane of its own; fall back to the default view.
      PickerChannel::Alpha => Self::HueChroma,
    }
  }

  /// The axis this plane holds constant.
  fn fixed_axis(self) -> PlaneAxis {
    match self {
      Self::HueChroma => PlaneAxis::Tone,
      Self::HueTone => PlaneAxis::Chroma,
      Self::ChromaTone => PlaneAxis::Hue,
    }
  }

  /// Axis ranges as `(horizontal, vertical)`.
  fn axes(self) -> ((PlaneAxis, f32), (PlaneAxis, f32)) {
    match self {
      Self::HueChroma => ((PlaneAxis::Hue, HUE_MAX), (PlaneAxis::Chroma, CHROMA_MAX)),
      Self::HueTone => ((PlaneAxis::Hue, HUE_MAX), (PlaneAxis::Tone, TONE_MAX)),
      Self::ChromaTone => ((PlaneAxis::Chroma, CHROMA_MAX), (PlaneAxis::Tone, TONE_MAX)),
    }
  }
}

/// One axis of a plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaneAxis {
  Hue,
  Chroma,
  Tone,
}

/// A sampled plane: an RGBA bitmap plus the mask of which pixels are outside
/// each gamut.
///
/// The bitmap exists so the plane costs one draw instead of one per sample.
/// `PlaneResolution` pixels squared is a few hundred thousand conversions when
/// the tone changes and nothing at all on every other frame, whereas painting
/// per sample made a drag stutter.
struct Plane {
  width: usize,
  height: usize,
  /// Premultiplied RGBA, row-major, top row first.
  rgba: Vec<u8>,
  /// `1` where the sample is outside sRGB.
  outside_srgb: Vec<u8>,
  /// `1` where the sample is outside Display-P3.
  outside_p3: Vec<u8>,
}

/// Pixels per side of a sampled plane.
///
/// This is the *data* resolution, not the on-screen size: the bitmap is scaled
/// to the plot area, so a larger value buys smooth boundaries rather than a
/// bigger chart. 192 is past the point where the sRGB edge reads as a curve at
/// the sizes the popover uses.
const PLANE_RESOLUTION: usize = 192;

/// Side of the 2-D plane, in em.
///
/// Square, and equal to the height of the four slider rows beside it, so the
/// two halves of the popover end level. Derived in em rather than hard-coded in
/// pixels: the sliders are laid out in em, so the chart has to be too, or it
/// stops matching them as soon as the component size changes.
///
/// Each row is a label at the system's type size (1em text, ~1.4em line box),
/// a `0.25em` gap and a track of `track_height + 0.5em` (1.75em), so 3.4em;
/// four rows plus three `0.5em` gaps between them is 15.1em.
const PLANE_SIDE: Rems = rems(15.1);

impl Plane {
  /// Samples the plane at the picker's current position.
  ///
  /// `current` fills the axes this plane does not show, so the bitmap is a
  /// slice through the actual colour space rather than through some fixed
  /// origin.
  fn sample(kind: PlaneKind, current: ColorPickerHct) -> Self {
    let ((x_axis, x_max), (y_axis, y_max)) = kind.axes();
    let n = PLANE_RESOLUTION;
    let conditions = ViewingConditions::default();

    let mut rgba = vec![0u8; n * n * 4];
    let mut outside_srgb = vec![0u8; n * n];
    let mut outside_p3 = vec![0u8; n * n];

    for row in 0..n {
      // Row 0 is the top of the bitmap, which is the *maximum* of the vertical
      // axis.
      let y_value = (1.0 - (row as f64 + 0.5) / n as f64) * f64::from(y_max);
      for col in 0..n {
        let x_value = (col as f64 + 0.5) / n as f64 * f64::from(x_max);

        // Start from the fixed axis only: the two drawn axes are overwritten
        // below, so seeding them from `current` would make the bitmap depend on
        // values it does not show.
        let (mut hue, mut chroma, mut tone) = match kind.fixed_axis() {
          PlaneAxis::Hue => (f64::from(current.hue), 0.0, 0.0),
          PlaneAxis::Chroma => (0.0, f64::from(current.chroma), 0.0),
          PlaneAxis::Tone => (0.0, 0.0, f64::from(current.tone)),
        };
        match x_axis {
          PlaneAxis::Hue => hue = x_value,
          PlaneAxis::Chroma => chroma = x_value,
          PlaneAxis::Tone => tone = x_value,
        }
        match y_axis {
          PlaneAxis::Hue => hue = y_value,
          PlaneAxis::Chroma => chroma = y_value,
          PlaneAxis::Tone => tone = y_value,
        }

        // One CAM16 inverse, no HCT solve: the solve finds the nearest
        // *displayable* colour, which is the one thing this view must not do —
        // it has to show what was asked for, and separately whether that was
        // displayable.
        let xyz = Cam16::xyz_from_hct(tone, chroma, hue, &conditions);
        let linear = woocraft_colors::linear_srgb(xyz);
        let inside = linear.iter().all(|c| (-1e-3..=1.0 + 1e-3).contains(c));
        let p3_inside = woocraft_colors::in_p3(xyz);

        let index = row * n + col;
        outside_srgb[index] = u8::from(!inside);
        outside_p3[index] = u8::from(!p3_inside);

        // Outside sRGB the sample is drawn clamped and dimmed, so the shape of
        // what the display cannot show is visible rather than blank.
        let alpha = if inside { 255u8 } else { 90 };
        let offset = index * 4;
        rgba[offset] = to_byte(delinearize(linear[0]));
        rgba[offset + 1] = to_byte(delinearize(linear[1]));
        rgba[offset + 2] = to_byte(delinearize(linear[2]));
        rgba[offset + 3] = alpha;
      }
    }

    Self {
      width: n,
      height: n,
      rgba,
      outside_srgb,
      outside_p3,
    }
  }
}

/// A clamped `0..=1` channel as an 8-bit value.
fn to_byte(value: f64) -> u8 {
  (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The sRGB transfer function, for turning linear channels back into display
/// values. Clamped, because the plane paints clamped colours.
fn delinearize(linear: f64) -> f64 {
  let value = linear.clamp(0.0, 1.0);
  if value <= 0.003_130_8 {
    value * 12.92
  } else {
    1.055 * value.powf(1.0 / 2.4) - 0.055
  }
}

/// Paints the hue/chroma plane, its axes and both gamut boundaries.
fn paint_plane(
  plane: &Plane, kind: PlaneKind, current: ColorPickerHct, tier: Size, bounds: Bounds<Pixels>,
  window: &mut Window, cx: &mut App,
) {
  let rem_size = window.rem_size();
  // Room for the axis labels, which are at the system's single type size.
  let axis_inset = rems(1.5).to_pixels(rem_size);

  let plot = Bounds::new(
    point(bounds.origin.x + axis_inset, bounds.origin.y),
    size(
      (bounds.size.width - axis_inset).max(px(1.)),
      (bounds.size.height - axis_inset).max(px(1.)),
    ),
  );

  // One textured quad for the whole plane. gpui's `RenderImage` wants BGRA, and
  // the bitmap is built RGBA, so the swap happens here rather than in the
  // sampler.
  let mut bgra = plane.rgba.clone();
  for pixel in bgra.as_chunks_mut::<4>().0 {
    pixel.swap(0, 2);
  }
  let buffer = image::RgbaImage::from_raw(plane.width as u32, plane.height as u32, bgra)
    .expect("plane bitmap is width * height * 4 bytes");
  let image = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(buffer)]));
  window
    .paint_image(plot, plot, gpui::Corners::default(), image, 0, false)
    .ok();

  // Gamut boundaries: only the edge pixels of each mask, so the interior is not
  // overpainted.
  let srgb_color = cx.theme().foreground.opacity(0.75);
  let p3_color = cx.theme().info.opacity(0.85);
  let cell_w = plot.size.width / plane.width as f32;
  let cell_h = plot.size.height / plane.height as f32;

  let draw_edge = |mask: &[u8], color: Hsla, window: &mut Window| {
    for row in 0..plane.height {
      for col in 0..plane.width {
        let index = row * plane.width + col;
        if mask[index] == 0 {
          continue;
        }
        let edge = (col > 0 && mask[index - 1] == 0)
          || (col + 1 < plane.width && mask[index + 1] == 0)
          || (row > 0 && mask[index - plane.width] == 0)
          || (row + 1 < plane.height && mask[index + plane.width] == 0);
        if !edge {
          continue;
        }
        let x = plot.origin.x + cell_w * col as f32;
        let y = plot.origin.y + cell_h * row as f32;
        window.paint_quad(fill(
          Bounds::new(point(x, y), size(cell_w.max(px(1.)), cell_h.max(px(1.)))),
          color,
        ));
      }
    }
  };
  draw_edge(&plane.outside_srgb, srgb_color, window);
  draw_edge(&plane.outside_p3, p3_color, window);

  // Axes.
  let axis = cx.theme().border;
  window.paint_quad(fill(
    Bounds::new(
      point(plot.origin.x, plot.bottom()),
      size(plot.size.width, px(1.)),
    ),
    axis,
  ));
  window.paint_quad(fill(
    Bounds::new(plot.origin, size(px(1.), plot.size.height)),
    axis,
  ));

  // Current position.
  let ((x_axis, x_max), (y_axis, y_max)) = kind.axes();
  let marker_x = plot.origin.x + plot.size.width * (axis_value(x_axis, current) / x_max);
  let marker_y = plot.bottom() - plot.size.height * (axis_value(y_axis, current) / y_max);
  let ring = Bounds::new(
    point(marker_x - px(4.), marker_y - px(4.)),
    size(px(8.), px(8.)),
  );
  window.paint_quad(
    fill(ring, Hsla::transparent_black())
      .border_widths(px(2.))
      .border_color(cx.theme().foreground),
  );

  // Axis labels use the same size as every other label in the system; the room
  // they need is taken out of the plot instead of out of the type scale.
  let mut text_style = window.text_style();
  text_style.color = cx.theme().muted_foreground;
  text_style.font_size = tier.text_size().into();
  let font_size = text_style.font_size.to_pixels(rem_size);
  let draw_label = |window: &mut Window, text: &str, at: Point<Pixels>, cx: &mut App| {
    let runs = [text_style.to_run(text.len())];
    let shaped = window
      .text_system()
      .shape_line(text.to_string().into(), font_size, &runs, None);
    shaped
      .paint(at, font_size, TextAlign::Left, None, window, cx)
      .ok();
  };

  draw_label(
    window,
    &axis_label(x_axis, 0.0),
    point(plot.origin.x, plot.bottom() + px(1.)),
    cx,
  );
  draw_label(
    window,
    &axis_label(x_axis, x_max),
    point(plot.right() - px(28.), plot.bottom() + px(1.)),
    cx,
  );
  draw_label(
    window,
    axis_name(y_axis),
    point(plot.origin.x - axis_inset + px(1.), plot.origin.y),
    cx,
  );
  draw_label(
    window,
    &axis_label(y_axis, y_max),
    point(plot.origin.x - axis_inset + px(1.), plot.origin.y - px(6.)),
    cx,
  );
}

/// The current value of one plane axis.
fn axis_value(axis: PlaneAxis, current: ColorPickerHct) -> f32 {
  match axis {
    PlaneAxis::Hue => current.hue,
    PlaneAxis::Chroma => current.chroma,
    PlaneAxis::Tone => current.tone,
  }
}

/// The short name of an axis, for the vertical label.
fn axis_name(axis: PlaneAxis) -> &'static str {
  match axis {
    PlaneAxis::Hue => "H",
    PlaneAxis::Chroma => "C",
    PlaneAxis::Tone => "T",
  }
}

/// A tick label for one end of an axis.
fn axis_label(axis: PlaneAxis, value: f32) -> String {
  match axis {
    PlaneAxis::Hue => format!("{value:.0}°"),
    _ => format!("{value:.0}"),
  }
}

fn render_plane(
  current: ColorPickerHct, kind: PlaneKind, plane: Arc<Plane>, size: Size, cx: &mut App,
) -> impl IntoElement {
  let label = cx.theme().muted_foreground;
  let title = match kind {
    PlaneKind::HueChroma => "color_picker.plane_hc",
    PlaneKind::HueTone => "color_picker.plane_ht",
    PlaneKind::ChromaTone => "color_picker.plane_ct",
  };

  v_flex()
    .flex_none()
    .gap(rems(0.25))
    .child(
      div()
        .text_size(size.text_size())
        .text_color(label)
        .child(SharedString::from(translate_woocraft(title))),
    )
    // Square: the plane is a chart of two equal-weight axes, so an oblong box
    // would distort the shape of the gamut boundary. The side is fixed rather
    // than derived from the column, because the popover's height follows the
    // four slider rows and stretching to it is what made the chart a sliver.
    .child(
      div()
        .size(PLANE_SIDE)
        .relative()
        .rounded(cx.theme().radius)
        .overflow_hidden()
        .border_1()
        .border_color(cx.theme().border)
        .child(
          canvas(
            move |_, _, _| {},
            move |bounds, _, window, cx| {
              paint_plane(&plane, kind, current, size, bounds, window, cx);
            },
          )
          .absolute()
          .size_full(),
        ),
    )
}

fn render_channel_row(
  channel: PickerChannel, state: &Entity<ColorPickerState>, size: Size, cx: &mut App,
) -> impl IntoElement {
  // Refresh (or reuse) the cached gradient color table for this channel so
  // painting below never re-runs color space conversions.
  let (current_hct, value, stops) = state.update(cx, |state, _| {
    let current_hct = state.hct();
    (
      current_hct,
      channel.value(current_hct),
      state.channel_gradient(channel),
    )
  });
  let out_of_gamut = is_out_of_gamut(current_hct);

  // Only the channel that was actually clamped says so. A channel whose value
  // survived the gamut mapping keeps its single figure: showing an arrow with
  // the same number on both sides would suggest a change that did not happen.
  let achieved = out_of_gamut.then(|| {
    // `to_hct` re-reads the produced color, so its chroma is what the display
    // can actually show; hue and tone come through unchanged.
    let produced = to_hct(current_hct);
    channel.value(ColorPickerHct {
      hue: produced.hue() as f32,
      chroma: produced.chroma() as f32,
      tone: produced.tone().value() as f32,
      alpha: current_hct.alpha,
    })
  });
  let clamped = achieved.filter(|achieved| (achieved - value).abs() > 0.5);

  // One label per row: either the value, or `requested → achieved` when the
  // gamut forced it down.
  let label = match clamped {
    Some(achieved) => format!(
      "{} → {}",
      format_channel_value(channel, value),
      format_channel_value(channel, achieved),
    ),
    None => format_channel_value(channel, value),
  };

  let state_for_bounds = state.clone();
  let state_for_down = state.clone();
  let state_for_idle_move = state.clone();
  let state_for_paint = state.clone();
  let entity_id = state.entity_id();
  let channel_id = ElementId::from(("color-picker-channel", entity_id));
  let ratio = channel.normalized_value(current_hct);

  v_flex()
    .gap(rems(0.25))
    .child(
      h_flex()
        .w_full()
        .justify_between()
        .text_size(size.text_size())
        .text_color(cx.theme().muted_foreground)
        .child(SharedString::from(translate_woocraft(channel.i18n_key())))
        .child(
          h_flex().items_center().gap(rems(0.375)).child(
            div()
              .text_color(if clamped.is_some() {
                cx.theme().warning
              } else {
                cx.theme().muted_foreground
              })
              .child(SharedString::from(label)),
          ),
        ),
    )
    .child(
      div()
        .id((channel_id, channel.id_label()))
        .relative()
        .h(size.track_height() + rems(0.5))
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
              state.begin_channel_drag(channel, event.position, window, cx);
            });
            cx.stop_propagation();
          },
        )
        // Safety net: if the release itself was missed (e.g. it happened
        // outside the window), clear the gesture on the next unpressed move
        // over the track.
        .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
          if event.pressed_button.is_none() {
            state_for_idle_move.update(cx, |state, cx| state.end_channel_drag(cx));
          }
        })
        .child(
          canvas(
            move |_, _, _| {},
            move |bounds, _, window, cx| {
              paint_channel(&stops, ratio, bounds, window, cx);
              paint_channel_drag_listeners(channel, &state_for_paint, window, cx);
            },
          )
          .absolute()
          .size_full(),
        ),
    )
}

/// Installs the window-level listeners that keep a channel gesture alive while
/// the pointer is anywhere — outside the track, outside the popover, even
/// outside the window.
///
/// An element's own `on_mouse_move` only fires while it is hovered, so a drag
/// used to stop the moment the pointer left the track. These listeners are
/// registered during paint and only while a gesture is in flight, so no cost is
/// paid while the picker is idle, and they see every move the window receives
/// for the rest of the gesture.
fn paint_channel_drag_listeners(
  channel: PickerChannel, state: &Entity<ColorPickerState>, window: &mut Window, cx: &mut App,
) {
  if state.read(cx).dragging != Some(channel) {
    return;
  }

  window.on_mouse_event({
    let state = state.clone();
    move |event: &MouseMoveEvent, phase, window, cx| {
      if phase != DispatchPhase::Bubble {
        return;
      }

      if event.pressed_button == Some(MouseButton::Left) {
        state.update(cx, |state, cx| {
          state.drag_channel_to(channel, event.position, window, cx);
        });
        cx.stop_propagation();
      } else {
        // A release outside the window never reaches the track: the first
        // move without a pressed button ends the gesture instead.
        state.update(cx, |state, cx| state.end_channel_drag(cx));
      }
    }
  });

  window.on_mouse_event({
    let state = state.clone();
    move |_: &MouseUpEvent, phase, _, cx| {
      if phase != DispatchPhase::Bubble {
        return;
      }

      state.update(cx, |state, cx| state.end_channel_drag(cx));
    }
  });

  // A gesture cannot survive a new press somewhere else. This also clears a
  // gesture left behind when the popover closed mid-drag, so it cannot steer a
  // channel from a later, unrelated drag. A press on the owning track is left
  // alone: the track itself restarts the gesture there.
  window.on_mouse_event({
    let state = state.clone();
    move |event: &MouseDownEvent, phase, _, cx| {
      if phase != DispatchPhase::Bubble {
        return;
      }

      let bounds = state.read(cx).channel_bounds[channel.index()];
      if bounds.contains(&event.position) {
        return;
      }

      state.update(cx, |state, cx| state.end_channel_drag(cx));
    }
  });
}

/// The x-range the indicator's centre travels over.
///
/// Inset by half the painted indicator width (bar plus outline) on each side,
/// so the indicator never leaves the track at either end while the pointer can
/// still reach both ends of the range.
fn channel_pointer_travel(bounds: Bounds<Pixels>, rem_size: Pixels) -> Bounds<Pixels> {
  let inset = pointer_half_width(rem_size);
  let travel_width = bounds.size.width - inset * 2.0;
  Bounds::new(
    point(bounds.origin.x + inset, bounds.origin.y),
    size(travel_width.max(px(0.)), bounds.size.height),
  )
}

/// Half the painted width of the indicator: the bar plus its outline on either
/// side.
fn pointer_half_width(rem_size: Pixels) -> Pixels {
  POINTER_WIDTH.to_pixels(rem_size) / 2.0 + POINTER_OUTLINE
}

/// The indicator for `ratio`: its colour-filled bar and its outline.
///
/// The outline colour is picked from `outline`, which callers pass as the
/// neutral grey most distinct from the bar's own colour — the indicator sits on
/// an arbitrary colour from the gradient, so a fixed theme foreground would
/// vanish over parts of the track.
fn channel_indicator(
  stops: &[ChannelGradientStop], ratio: f32, bounds: Bounds<Pixels>, rem_size: Pixels,
  outline: Hsla,
) -> (PaintQuad, PaintQuad) {
  let bar_width = POINTER_WIDTH.to_pixels(rem_size);
  let outline_width = POINTER_OUTLINE;
  let radius = POINTER_RADIUS.to_pixels(rem_size);
  let travel = channel_pointer_travel(bounds, rem_size);
  let center_x = travel.origin.x + travel.size.width * ratio;

  // The whole indicator — bar plus outline — is `track height - 0.25em` tall
  // and vertically centred, so the bar itself is the outline width shorter at
  // each end.
  let total_height = (bounds.size.height - POINTER_INSET.to_pixels(rem_size) * 2.0).max(px(0.));
  let bar_height = (total_height - outline_width * 2.0).max(px(0.));
  let bar = Bounds::new(
    point(
      center_x - bar_width / 2.0,
      bounds.center().y - bar_height / 2.0,
    ),
    size(bar_width, bar_height),
  );
  let ring = Bounds::new(
    point(bar.origin.x - outline_width, bar.origin.y - outline_width),
    size(
      bar.size.width + outline_width * 2.0,
      bar.size.height + outline_width * 2.0,
    ),
  );

  // Each quad is clamped on its own: the outline is larger than the bar, so it
  // takes a larger radius, and both stay pill-shaped at any track height.
  let bar_radius = Corners::all(radius).clamp_radii_for_quad_size(bar.size);
  let ring_radius = Corners::all(radius + outline_width).clamp_radii_for_quad_size(ring.size);

  (
    fill(bar, channel_color_at(stops, ratio)).corner_radii(bar_radius),
    fill(ring, Hsla::transparent_black())
      .corner_radii(ring_radius)
      .border_widths(outline_width)
      .border_color(outline),
  )
}

fn paint_channel(
  stops: &[ChannelGradientStop], ratio: f32, bounds: Bounds<Pixels>, window: &mut Window,
  cx: &mut App,
) {
  let rem_size = window.rem_size();
  let warning_line_thickness = WARNING_LINE_THICKNESS.to_pixels(rem_size);

  for (stop, segment) in stops
    .iter()
    .zip(channel_segments(stops.len(), bounds, rem_size))
  {
    let color = stop_color(stop);
    window.paint_quad(fill(segment, color));
    if stop.out_of_gamut {
      let warning_segment = Bounds::new(
        point(segment.origin.x, bounds.bottom() - warning_line_thickness),
        size(segment.size.width, warning_line_thickness),
      );
      window.paint_quad(fill(warning_segment, cx.theme().warning));
    }
  }

  // The indicator is a bar inset from the track's top and bottom, filled with
  // the color the track has at that spot so it reads as part of the gradient.
  // Its outline is a neutral grey chosen for contrast against that same color:
  // the bar sits on an arbitrary point of the gradient, so one fixed theme
  // colour would disappear over part of the track.
  let indicator_color = channel_color_at(stops, ratio);
  let outline = contrasting_neutral(indicator_color);
  let (indicator_fill, indicator_outline) =
    channel_indicator(stops, ratio, bounds, rem_size, outline);
  window.paint_quad(indicator_fill);
  window.paint_quad(indicator_outline);
}

/// The x-bounds of each gradient segment, laid out over the indicator's travel
/// range so a ratio maps to the same x for the gradient and the indicator.
///
/// The two outer segments are stretched past the track's ends: the track clips
/// that overhang, and the extra end color keeps its rounded corners from
/// leaving a sliver of background next to the indicator when it sits at an end.
fn channel_segments(steps: usize, bounds: Bounds<Pixels>, rem_size: Pixels) -> Vec<Bounds<Pixels>> {
  let steps = steps.max(1);
  let edge_padding = TRACK_EDGE_PADDING.to_pixels(rem_size);
  let travel = channel_pointer_travel(bounds, rem_size);

  (0..steps)
    .map(|ix| {
      let start_ratio = ix as f32 / steps as f32;
      let end_ratio = (ix + 1) as f32 / steps as f32;

      let mut x = travel.origin.x + travel.size.width * start_ratio;
      let mut next_x = travel.origin.x + travel.size.width * end_ratio;
      if ix == 0 {
        x = bounds.left() - edge_padding;
      }
      if ix + 1 == steps {
        next_x = bounds.right() + edge_padding;
      }

      Bounds::new(
        point(x, bounds.origin.y),
        size((next_x - x).max(px(1.)), bounds.size.height),
      )
    })
    .collect()
}

/// The gradient color at `ratio`, sampled from the same table the track paints
/// with, so the indicator matches the pixel under it.
fn channel_color_at(stops: &[ChannelGradientStop], ratio: f32) -> Hsla {
  let steps = stops.len().max(1);
  let index = (ratio.clamp(0.0, 1.0) * steps as f32) as usize;
  stop_color(&stops[index.min(steps - 1)])
}

fn stop_color(stop: &ChannelGradientStop) -> Hsla {
  Hsla::from(gpui::Rgba {
    r: stop.rgba.r,
    g: stop.rgba.g,
    b: stop.rgba.b,
    a: stop.rgba.a,
  })
}

/// Resolves one gradient segment: display color plus gamut warning flag.
fn resolve_gradient_stop(value: ColorPickerHct) -> ChannelGradientStop {
  let (rgba, out_of_gamut) = resolve_rgba(value);
  ChannelGradientStop { rgba, out_of_gamut }
}

/// Builds the gradient color table for `channel`, sweeping it across its full
/// range while keeping every other HCT component fixed.
fn build_channel_gradient(
  channel: PickerChannel, value: ColorPickerHct,
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

fn sanitize_hct(value: ColorPickerHct) -> ColorPickerHct {
  ColorPickerHct {
    hue: value.hue.clamp(0.0, HUE_MAX),
    chroma: value.chroma.clamp(0.0, CHROMA_MAX),
    tone: value.tone.clamp(0.0, TONE_MAX),
    alpha: value.alpha.clamp(0.0, 1.0),
  }
}

/// The HCT color as the crate's type.
fn to_hct(value: ColorPickerHct) -> Hct {
  let value = sanitize_hct(value);
  Hct::new(
    f64::from(value.hue),
    f64::from(value.chroma),
    Tone::new(f64::from(value.tone)),
  )
}

/// Whether the request fell outside the sRGB gamut.
///
/// The solver always returns a displayable color, so the honest signal is how
/// much chroma it had to give up: if the produced color carries meaningfully
/// less chroma than asked for, the request was out of gamut.
fn is_out_of_gamut(value: ColorPickerHct) -> bool {
  let requested = f64::from(sanitize_hct(value).chroma);
  if requested < 0.5 {
    return false;
  }
  let produced = to_hct(value).chroma();
  requested - produced > 1.0
}

/// Converts `value` to a displayable sRGB color.
///
/// HCT has no out-of-gamut representation to clamp: the solver keeps hue and
/// tone and reduces chroma until the color fits, so this is a plain conversion
/// and the gamut flag is a report rather than a correction.
fn resolve_rgba(value: ColorPickerHct) -> (ColorPickerRgba, bool) {
  let value = sanitize_hct(value);
  let color = to_hct(value);
  let srgb = color.srgb();
  let rgba = ColorPickerRgba {
    r: srgb.r as f32,
    g: srgb.g as f32,
    b: srgb.b as f32,
    a: value.alpha,
  };
  (rgba, is_out_of_gamut(value))
}

fn resolve_value(value: ColorPickerHct) -> ColorPickerValue {
  let value = sanitize_hct(value);
  let (clamped, _) = resolve_rgba(value);
  let hct = to_hct(value);
  let hsl = hct.hsl();

  let hsla = ColorPickerHsla {
    hue: normalize_degrees(hsl.h as f32),
    saturation: (hsl.s as f32).clamp(0.0, 1.0),
    lightness: (hsl.l as f32).clamp(0.0, 1.0),
    alpha: clamped.a,
  };

  ColorPickerValue {
    rgba_hex: rgba_to_hex(clamped).into(),
    rgba: clamped,
    hct: ColorPickerHct {
      hue: hct.hue() as f32,
      chroma: hct.chroma() as f32,
      tone: hct.tone().value() as f32,
      alpha: value.alpha,
    },
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

fn hct_from_rgba(rgba: ColorPickerRgba, fallback_hue: f32) -> ColorPickerHct {
  let srgb = woocraft_colors::Srgb::with_alpha(
    f64::from(rgba.r),
    f64::from(rgba.g),
    f64::from(rgba.b),
    f64::from(rgba.a),
  );
  let hct = Hct::from_srgb(srgb);

  // A neutral has no hue of its own, so keep the hue the user was editing
  // instead of letting the conversion snap it to an arbitrary angle.
  let hue = if hct.chroma() < 0.5 {
    normalize_degrees(fallback_hue)
  } else {
    hct.hue() as f32
  };

  ColorPickerHct {
    hue,
    chroma: hct.chroma() as f32,
    tone: hct.tone().value() as f32,
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
  use std::ops::Deref as _;

  use gpui::{
    Context, IntoElement, Modifiers, MouseButton, Pixels, Render, TestAppContext,
    VisualTestContext, Window, point, px,
  };

  use super::*;
  use crate::Theme;

  fn sample_hct() -> ColorPickerHct {
    ColorPickerHct {
      hue: 248.0,
      chroma: 48.0,
      tone: 45.0,
      alpha: 1.0,
    }
  }

  fn gradient_channels() -> [PickerChannel; 4] {
    [
      PickerChannel::Tone,
      PickerChannel::Chroma,
      PickerChannel::Hue,
      PickerChannel::Alpha,
    ]
  }

  #[test]
  fn plane_follows_the_dragged_slider() {
    // The plane explains the two axes the dragged slider does not control.
    assert_eq!(
      PlaneKind::for_channel(PickerChannel::Tone),
      PlaneKind::HueChroma
    );
    assert_eq!(
      PlaneKind::for_channel(PickerChannel::Chroma),
      PlaneKind::HueTone
    );
    assert_eq!(
      PlaneKind::for_channel(PickerChannel::Hue),
      PlaneKind::ChromaTone
    );
    // Alpha has no plane; it keeps the default view.
    assert_eq!(
      PlaneKind::for_channel(PickerChannel::Alpha),
      PlaneKind::HueChroma
    );
  }

  #[test]
  fn the_plane_shown_during_a_drag_explains_the_other_two_axes() {
    // Dragging an axis shows the plane of the two axes it is *not* setting, so
    // the chart answers "where does this value sit" for the dimensions the drag
    // leaves alone.
    for (channel, kind) in [
      (PickerChannel::Tone, PlaneKind::HueChroma),
      (PickerChannel::Chroma, PlaneKind::HueTone),
      (PickerChannel::Hue, PlaneKind::ChromaTone),
      // Alpha has no plane of its own.
      (PickerChannel::Alpha, PlaneKind::HueChroma),
    ] {
      assert_eq!(PlaneKind::for_channel(channel), kind);
    }
  }

  #[test]
  fn a_plane_is_reused_until_its_fixed_axis_moves() {
    let mut state = ColorPickerState::new();

    // Idle, the plane is hue/chroma and holds tone.
    let (kind, first) = state.plane();
    assert_eq!(kind, PlaneKind::HueChroma);
    assert_eq!(kind.fixed_axis(), PlaneAxis::Tone);

    // Moving a drawn axis leaves the bitmap valid: the plane sweeps it.
    state.value.hue += 30.0;
    state.value.chroma += 10.0;
    let (_, after_drawn) = state.plane();
    assert!(
      Arc::ptr_eq(&first, &after_drawn),
      "moving a drawn axis must not resample"
    );

    // Moving the fixed axis invalidates it: that is a different slice.
    state.value.tone += 5.0;
    let (_, after_fixed) = state.plane();
    assert!(
      !Arc::ptr_eq(&first, &after_fixed),
      "moving the fixed axis must resample"
    );
  }

  #[test]
  fn the_plane_stays_after_a_drag_ends() {
    let mut state = ColorPickerState::new();
    assert_eq!(state.plane().0, PlaneKind::HueChroma);

    // Dragging hue switches to chroma/tone...
    state.dragging = Some(PickerChannel::Hue);
    let (kind, dragged) = state.plane();
    assert_eq!(kind, PlaneKind::ChromaTone);

    // ...and releasing keeps it, so the user is not thrown back to the default
    // view the moment they let go.
    state.dragging = None;
    let (kind, released) = state.plane();
    assert_eq!(kind, PlaneKind::ChromaTone);
    assert!(
      Arc::ptr_eq(&dragged, &released),
      "releasing must not rebuild the plane"
    );

    // A later drag of another slider moves the view again.
    state.dragging = Some(PickerChannel::Chroma);
    assert_eq!(state.plane().0, PlaneKind::HueTone);
  }

  #[test]
  fn marker_matches_the_gamut_it_is_drawn_in() {
    // The reported bug: #22F583 is a legal sRGB colour, but its marker sat
    // outside the sRGB boundary because the plane sampled with tone passed as
    // CAM16's J.
    let conditions = ViewingConditions::default();
    let srgb = woocraft_colors::Srgb::from_hex("#22F583FF").unwrap();
    let hct = Hct::from_srgb(srgb);

    // The colour itself is inside both gamuts.
    let cam = Cam16::from_srgb(srgb, &conditions);
    assert!(woocraft_colors::in_srgb(cam.to_xyz(&conditions)));

    // And the same coordinates, sampled the way the plane samples them, land
    // in the same place.
    let xyz = Cam16::xyz_from_hct(hct.tone().value(), hct.chroma(), hct.hue(), &conditions);
    assert!(
      woocraft_colors::in_srgb(xyz),
      "the plane's own sampling puts {} outside sRGB, so its marker would drift out",
      srgb.to_hex()
    );
    assert!(woocraft_colors::in_p3(xyz));
  }

  #[test]
  fn plane_marks_both_gamuts() {
    let plane = Plane::sample(
      PlaneKind::HueChroma,
      ColorPickerHct {
        hue: 0.0,
        chroma: 0.0,
        tone: 50.0,
        alpha: 1.0,
      },
    );
    let srgb_out = plane.outside_srgb.iter().filter(|v| **v == 1).count();
    let p3_out = plane.outside_p3.iter().filter(|v| **v == 1).count();

    assert!(
      srgb_out > 0,
      "the plane should show colours sRGB cannot hold"
    );
    assert!(
      p3_out < srgb_out,
      "P3 is wider than sRGB, so fewer samples may fall outside it"
    );
    assert!(p3_out > 0, "very saturated requests should still leave P3");
    assert_eq!(plane.rgba.len(), plane.width * plane.height * 4);
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
    let base = sample_hct();

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

    let first = state.channel_gradient(PickerChannel::Tone);
    let second = state.channel_gradient(PickerChannel::Tone);
    assert!(
      Arc::ptr_eq(&first, &second),
      "unchanged value reuses the table"
    );

    // The tone gradient depends on chroma, so it must rebuild.
    state.value.chroma += 1.0;
    let third = state.channel_gradient(PickerChannel::Tone);
    assert!(!Arc::ptr_eq(&first, &third));

    // Moving the swept component itself must not invalidate the table.
    let chroma_table = state.channel_gradient(PickerChannel::Chroma);
    state.value.tone += 1.0;
    assert!(Arc::ptr_eq(
      &third,
      &state.channel_gradient(PickerChannel::Tone)
    ));
    assert!(!Arc::ptr_eq(
      &chroma_table,
      &state.channel_gradient(PickerChannel::Chroma)
    ));
  }

  #[test]
  fn alpha_gradient_spans_full_range() {
    let colors = build_channel_gradient(PickerChannel::Alpha, sample_hct());
    assert_eq!(colors.len(), GRADIENT_STEPS);
    assert!(colors.first().unwrap().rgba.a < 0.1);
    assert!((colors.last().unwrap().rgba.a - 1.0).abs() < 0.05);
  }

  #[test]
  fn hue_gradient_of_gray_is_uniform() {
    let gray = ColorPickerHct {
      hue: 0.0,
      chroma: 0.0,
      tone: 50.0,
      alpha: 1.0,
    };
    let colors = build_channel_gradient(PickerChannel::Hue, gray);
    assert_eq!(colors.len(), GRADIENT_STEPS);
    assert!(colors.iter().all(|stop| stop.rgba == colors[0].rgba));
    assert!(colors.iter().all(|stop| !stop.out_of_gamut));
  }

  #[gpui::test]
  fn set_hct_keeps_resolved_cache_consistent(cx: &mut TestAppContext) {
    let state = cx.new(|_| ColorPickerState::new());
    cx.update(|app| {
      assert_eq!(
        state.read(app).value(),
        resolve_value(ColorPickerHct::default())
      );

      let next = ColorPickerHct {
        hue: 90.0,
        chroma: 30.0,
        tone: 80.0,
        alpha: 0.5,
      };
      state.update(app, |state, cx| state.set_hct(next, cx));

      let resolved = state.read(app).value();
      // Hue and tone survive the conversion; chroma is whatever the gamut
      // allowed at that hue and tone.
      assert!((resolved.hct.hue - next.hue).abs() < 2.0);
      assert!((resolved.hct.tone - next.tone).abs() < 1.0);
      assert!(resolved.hct.chroma <= next.chroma + 1.0);
      // The cached value must stay consistent with a fresh resolve.
      assert_eq!(resolved, resolve_value(next));
    });
  }

  /// Host that renders the given channel tracks of a color picker and pins
  /// them to known rectangles, so a test can drive the pointer in absolute
  /// window coordinates.
  struct TrackHost {
    state: Entity<ColorPickerState>,
    channels: &'static [PickerChannel],
  }

  impl Render for TrackHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
      let mut tracks = Vec::with_capacity(self.channels.len());
      for channel in self.channels {
        tracks.push(render_channel_row(*channel, &self.state, Size::Medium, cx).into_any_element());
      }
      div().size_full().p_4().children(tracks)
    }
  }

  /// Opens a window with the given channel tracks and returns the window's
  /// visual context, the picker state and the tracks' bounds in the same
  /// order.
  fn open_track_window(
    cx: &mut TestAppContext, channels: &'static [PickerChannel],
  ) -> (
    VisualTestContext,
    Entity<ColorPickerState>,
    Vec<Bounds<Pixels>>,
  ) {
    cx.set_global(Theme::default());

    let state = cx.new(|_| ColorPickerState::new());
    let state_for_host = state.clone();
    let window = cx.open_window(gpui::size(px(480.), px(320.)), move |_, _cx| TrackHost {
      state: state_for_host,
      channels,
    });

    let mut visual = VisualTestContext::from_window(*window.deref(), cx);
    visual.run_until_parked();

    let bounds: Vec<Bounds<Pixels>> = visual.update(|_, cx| {
      let state = state.read(cx);
      channels
        .iter()
        .map(|channel| state.channel_bounds[channel.index()])
        .collect()
    });
    for (channel, bounds) in channels.iter().zip(&bounds) {
      assert!(
        bounds.size.width > px(0.),
        "the {channel:?} track must be laid out before it is dragged"
      );
    }
    (visual, state, bounds)
  }

  #[gpui::test]
  fn drag_keeps_steering_the_channel_outside_the_track(cx: &mut TestAppContext) {
    let (mut visual, state, bounds) = open_track_window(cx, &[PickerChannel::Tone]);
    let bounds = bounds[0];
    let center = bounds.center();

    visual.simulate_mouse_down(center, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();

    // Dragging within the track moves the value as before.
    visual.simulate_mouse_move(
      point(bounds.origin.x + bounds.size.width * 0.75, center.y),
      MouseButton::Left,
      Modifiers::default(),
    );
    visual.run_until_parked();
    let inside = visual.update(|_, cx| state.read(cx).hct().tone);
    assert!(
      (inside - 75.0).abs() < 2.0,
      "pointer at 75% of the track selects tone {inside}"
    );

    // The pointer leaves the track but stays pressed: the value must keep
    // following it and saturate at the track's end.
    visual.simulate_mouse_move(
      point(bounds.origin.x + bounds.size.width * 3.0, center.y),
      MouseButton::Left,
      Modifiers::default(),
    );
    visual.run_until_parked();
    assert_eq!(
      visual.update(|_, cx| state.read(cx).hct().tone),
      TONE_MAX,
      "dragging past the right edge must still reach the maximum"
    );

    visual.simulate_mouse_move(
      point(bounds.origin.x - bounds.size.width * 3.0, center.y),
      MouseButton::Left,
      Modifiers::default(),
    );
    visual.run_until_parked();
    assert_eq!(
      visual.update(|_, cx| state.read(cx).hct().tone),
      0.0,
      "dragging past the left edge must still reach the minimum"
    );

    // Releasing ends the gesture: later moves must not steer the channel.
    visual.simulate_mouse_up(
      point(bounds.origin.x - bounds.size.width * 3.0, center.y),
      MouseButton::Left,
      Modifiers::default(),
    );
    visual.run_until_parked();

    visual.simulate_mouse_move(
      point(bounds.origin.x + bounds.size.width * 0.5, center.y),
      None,
      Modifiers::default(),
    );
    visual.run_until_parked();
    assert_eq!(
      visual.update(|_, cx| state.read(cx).hct().tone),
      0.0,
      "a released gesture must not keep following the pointer"
    );
  }

  #[gpui::test]
  fn drag_steers_only_the_channel_it_started_on(cx: &mut TestAppContext) {
    let (mut visual, state, bounds) =
      open_track_window(cx, &[PickerChannel::Hue, PickerChannel::Alpha]);
    let (hue_track, alpha_track) = (bounds[0], bounds[1]);
    let alpha_before = visual.update(|_, cx| state.read(cx).hct().alpha);

    visual.simulate_mouse_down(hue_track.center(), MouseButton::Left, Modifiers::default());
    visual.run_until_parked();

    // The pointer travels over the alpha track: the hue channel owns the
    // gesture, so alpha must stay put while hue keeps following.
    visual.simulate_mouse_move(
      point(
        hue_track.origin.x + hue_track.size.width * 0.25,
        alpha_track.center().y,
      ),
      MouseButton::Left,
      Modifiers::default(),
    );
    visual.run_until_parked();

    let hct = visual.update(|_, cx| state.read(cx).hct());
    assert!(
      (hct.hue - 90.0).abs() < 5.0,
      "pointer at 25% of the hue track selects {}°",
      hct.hue
    );
    assert_eq!(
      hct.alpha, alpha_before,
      "the alpha track must not pick up a drag that began on hue"
    );
  }
}
