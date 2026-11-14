//! The programs of the renderer, in GLSL ES 1.00: one source, built once
//! for each pair of axes that blend. They draw the game quad and the UI
//! quad. The fragment shader repeats the integer rules of the reference
//! renderer in 32-bit floats: every value is an integer below `2^24`, so
//! it is exact, and every division is corrected to the exact quotient.
//!
//! The shader blends the taps itself, from `NEAREST` texture reads. The
//! `LINEAR` filter of the texture unit would need one read, but the unit of
//! `vc4` computes its weights in fixed point: on the Raspberry Pi 3 it
//! differs from the reference by up to 5 per channel even with exact
//! sample points, above the tolerance of 1.

/// One build of the fragment program. Which axes blend is a constant of
/// the source, not a uniform: the `vc4` compiler of the Raspberry Pi turns
/// a branch on a uniform into a select, so with a uniform every pixel would
/// read all four taps. An axis blends only with sharp bilinear at a scale
/// that is not a whole number; at a whole scale its weight is always 0, so
/// such an axis reads one tap, and sharp bilinear at a whole scale on both
/// axes is the nearest program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Variant {
    /// The columns blend: two taps across.
    pub(super) blend_x: bool,
    /// The lines blend: two taps down.
    pub(super) blend_y: bool,
}

/// The number of programs: one per pair of axes that blend.
pub(super) const VARIANTS: usize = 4;

impl Variant {
    /// Every variant, in [`index`](Self::index) order.
    pub(super) const ALL: [Variant; VARIANTS] = [
        Self::new(false, false),
        Self::new(true, false),
        Self::new(false, true),
        Self::new(true, true),
    ];

    const fn new(blend_x: bool, blend_y: bool) -> Self {
        Self { blend_x, blend_y }
    }

    /// The place of this variant in [`ALL`](Self::ALL).
    pub(super) fn index(self) -> usize {
        usize::from(self.blend_x) + 2 * usize::from(self.blend_y)
    }

    /// The name of the program, for an error.
    pub(super) fn name(self) -> &'static str {
        match (self.blend_x, self.blend_y) {
            (false, false) => "nearest",
            (true, false) => "sharp bilinear across columns",
            (false, true) => "sharp bilinear across lines",
            (true, true) => "sharp bilinear across columns and lines",
        }
    }

    /// The fragment shader source of this variant.
    pub(super) fn fragment(self) -> String {
        format!(
            "#version 100\n#define BLEND_X {}\n#define BLEND_Y {}\n{FRAGMENT_BODY}",
            u8::from(self.blend_x),
            u8::from(self.blend_y)
        )
    }
}

/// Maps the unit square to the visible rectangle, given in output pixels
/// from the top left, and gives the fragment shader the destination index
/// of each corner.
pub(super) const VERTEX: &str = r"#version 100
attribute vec2 a_corner;
uniform vec4 u_visible;
uniform vec4 u_output;
uniform vec4 u_dest;
// The destination column and line, counted from the top left; at a pixel
// centre the index plus one half.
varying vec2 v_index;

void main() {
    vec2 p = (u_visible.xy + a_corner * u_visible.zw) / u_output.xy * 2.0 - 1.0;
    gl_Position = vec4(p.x, -p.y, 0.0, 1.0);
    v_index = u_dest.xy + a_corner * u_visible.zw;
}
";

/// The texture holds 8-bit channels, so one `NEAREST` read gives a source
/// pixel. `BLEND_X` and `BLEND_Y` come from [`Variant::fragment`]. The quad
/// covers only output pixels whose near source pixel is inside the frame,
/// so no pixel needs a test for it.
const FRAGMENT_BODY: &str = r"#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
#error the renderer needs highp floats in the fragment shader
#endif

uniform sampler2D u_texture;
uniform vec4 u_dest;
uniform vec4 u_crop;
// The frame width and height in pixels, then their inverses.
uniform vec4 u_frame;
// 1 / (2 * dest width) and 1 / (2 * dest height), then the prescale of
// each axis.
uniform vec4 u_axis;
// The first and the last pixel of the crop inside the frame: x, y, x, y.
uniform vec4 u_bounds;
// Period, first dark line, kept percent and inverse period of the
// scanlines; period 0 for none.
uniform vec4 u_scan;
varying vec2 v_index;

// floor(a / b) and the remainder, exact for integers a < 2^24 and b > 0,
// with rb the float inverse of b. The float quotient can be one off, and
// the correction fixes that.
vec2 divmod(float a, float b, float rb) {
    float q = floor(a * rb);
    float r = a - q * b;
    if (r < 0.0) {
        q -= 1.0;
        r += b;
    } else if (r >= b) {
        q += 1.0;
        r -= b;
    }
    return vec2(q, r);
}

// The tap of destination index i on one axis: (near, far, weight). This is
// AxisMap::tap of the reference renderer. With blend false it is nearest.
vec3 tap(float i, float crop_start, float n, float m, float rm, float p,
         float first, float last, bool blend) {
    float den = 2.0 * m;
    vec2 qr = divmod((2.0 * i + 1.0) * n, den, rm);
    float near = crop_start + qr.x;
    if (!blend) {
        return vec3(near, near, 0.0);
    }
    // The reference weight is max(0, dist * p - (p - 1) * den), over 4m.
    // It equals den - (den - dist) * p, and that product is exact whenever
    // it is below den.
    float e = den - abs(2.0 * qr.y - den);
    float weight = max(0.0, den - e * p);
    float far = 2.0 * qr.y < den ? near - 1.0 : near + 1.0;
    return vec3(near, clamp(far, first, last), weight * rm * 0.5);
}

// The 8-bit channels of source pixel (x, y), from 0 to 255.
vec3 pixel(float x, float y) {
    vec2 uv = vec2((x + 0.5) * u_frame.z, (y + 0.5) * u_frame.w);
    // A channel is n / 255, and no n is within float error of a half.
    return floor(texture2D(u_texture, uv).rgb * 255.0 + 0.5);
}

// round(v * keep / 100) with a half rounded up, exact for an integer
// channel v: v * keep + 50 is an integer below 2^15.
float darken(float v, float keep) {
    return divmod(v * keep + 50.0, 100.0, 0.01).x;
}

void main() {
    // The index comes from a varying, not from gl_FragCoord: vc4 gives
    // gl_FragCoord at the pixel corner, and Mesa turns its y upside down
    // for a window surface and not for a framebuffer object. The varying
    // is half a pixel from any edge, so floor is exact.
    float i = floor(v_index.x);
    float j = floor(v_index.y);
    vec3 tx = tap(i, u_crop.x, u_crop.z, u_dest.z, u_axis.x, u_axis.z,
                  u_bounds.x, u_bounds.z, BLEND_X == 1);
    vec3 ty = tap(j, u_crop.y, u_crop.w, u_dest.w, u_axis.y, u_axis.w,
                  u_bounds.y, u_bounds.w, BLEND_Y == 1);
    // j is the destination line, so the pattern follows the source lines
    // and not the output line parity.
    bool dark = u_scan.x > 0.5 && divmod(j, u_scan.x, u_scan.w).y >= u_scan.y;
    vec3 c = pixel(tx.x, ty.x);
#if BLEND_X == 1 || BLEND_Y == 1
    if (tx.z > 0.0 || ty.z > 0.0) {
#if BLEND_X == 1
        c = mix(c, pixel(tx.y, ty.x), tx.z);
#endif
#if BLEND_Y == 1
#if BLEND_X == 1
        vec3 below = mix(pixel(tx.x, ty.y), pixel(tx.y, ty.y), tx.z);
#else
        vec3 below = pixel(tx.x, ty.y);
#endif
        c = mix(c, below, ty.z);
#endif
        // The reference scales the exact blend before its one rounding.
        if (dark) {
            c = c * (u_scan.z / 100.0);
        }
        // Round half up as the reference does, so the output is an exact
        // n / 255 and the conversion to 8 bits in GL does not round again.
        gl_FragColor = vec4(floor(c + 0.5) / 255.0, 1.0);
        return;
    }
#endif
    if (dark) {
        c = vec3(darken(c.r, u_scan.z), darken(c.g, u_scan.z), darken(c.b, u_scan.z));
    }
    gl_FragColor = vec4(c / 255.0, 1.0);
}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_its_own_index_and_name() {
        for (i, variant) in Variant::ALL.iter().enumerate() {
            assert_eq!(variant.index(), i, "{variant:?}");
            let same_name = Variant::ALL.iter().filter(|v| v.name() == variant.name());
            assert_eq!(same_name.count(), 1, "{variant:?}");
        }
    }

    /// Which axes blend is a constant of each program, and the texture
    /// holds 8-bit channels: no source reads either from a uniform, so no
    /// driver has a branch on them.
    #[test]
    fn a_variant_fixes_its_axes_in_its_source() {
        for variant in Variant::ALL {
            let source = variant.fragment();
            assert!(source.starts_with("#version 100\n"), "{variant:?}");
            let x = format!("#define BLEND_X {}\n", u8::from(variant.blend_x));
            let y = format!("#define BLEND_Y {}\n", u8::from(variant.blend_y));
            assert!(source.contains(&x) && source.contains(&y), "{variant:?}");
            assert!(!source.contains("u_mode"), "{variant:?}");
            assert!(!source.contains("FORMAT"), "{variant:?}");
        }
    }
}
