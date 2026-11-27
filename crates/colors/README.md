# woocraft-colors

HCT color space and perceptual color utilities for the Woocraft design system.

Implemented here rather than pulled from a dependency so the design system owns
its own color semantics: HCT is what the theme's contrast guarantees rest on, and
those guarantees should not move when a third-party crate changes.

## Why HCT

A design system needs two things from a color space that are in tension:

1. **Orthogonal hue, chroma and lightness**, so a picker can change one without
   dragging the others. OKLCH provides this.
2. **Lightness that determines contrast**, so an accessible pairing can be
   stated as a rule instead of audited color by color. OKLCH does not provide
   this: its `L` is not a function of relative luminance, and in this
   repository's own palette two colors at the same OKLCH lightness differ by
   more than 2x in WCAG contrast against white.

HCT splits the two concerns. Hue and chroma come from CAM16, so equal steps look
equal and hue holds when chroma changes. Tone is CIE L\*, which *is* a fixed
function of relative luminance. Two colors that share a tone therefore share a
contrast ratio against any background, whatever their hue and chroma — which is
why Material can state its roles as tone numbers and why this crate can do the
same.

```rust
use woocraft_colors::{Hct, Tone};

let surface = Hct::new(250.0, 20.0, Tone::new(40.0));
let text = surface.foreground(4.5);
assert!(text.has_contrast(surface, 4.5));
```

## What it provides

- [`Hct`] — hue, chroma, tone, plus `with_tone` / `with_hue` / `with_chroma`,
  `foreground`, `contrast_ratio`, `distance` and `lerp`.
- [`Tone`] — CIE L\* as its own type, so a tone cannot be confused with a chroma
  or an angle, with `luminance` and `from_luminance` conversions.
- [`Contrast`] — WCAG ratios from tones, and `lighter` / `darker` /
  `contrasting_tone` to solve for a ratio instead of searching.
- [`HctSolver`] — the gamut-mapping solver, for callers that want to know how
  much chroma a request gave up.
- [`Cam16`] and [`ViewingConditions`] — the appearance model underneath, with
  CAM16-UCS distance for perceptual color difference.
- [`Srgb`] and [`Hsl`] — the encoding the view layer consumes. `Hsl` is the form
  GPUI's `Hsla` is built from, so a component converts once at the boundary and
  reasons in HCT everywhere else.

## Correctness

The CAM16 and HCT algorithms are implemented from the published equations, then
pinned against reference values generated from the upstream Material Color
Utilities implementation (Apache-2.0) in `src/reference.rs`. The test suite
checks:

- 16 sRGB colors decomposed to HCT,
- 702 `(hue, chroma, tone)` → sRGB solves, byte-identical to the reference,
- 16 CAM16 coordinate sets, including UCS,
- 77 tone-pair contrast ratios,
- and the properties the design system relies on: contrast independent of hue
  and chroma at equal tone, tone changes preserving hue, and gamut mapping
  keeping hue and tone while reducing chroma.

## Scope

sRGB only. Gamut mapping keeps hue and tone and reduces chroma until the color
fits, which is the trade-off a design system wants: hue is intent, tone is
accessibility, chroma is the flexible axis. Wider gamuts would need a second
output encoding, not a different solver.

## License

Apache-2.0, matching the workspace.
