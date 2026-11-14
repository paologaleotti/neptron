//! The programs of the renderer, in GLSL ES 1.00: one source, built once
//! for each filter. They draw the game quad and the UI quad. The fragment
//! shader repeats the integer rules of the reference renderer in 32-bit
//! floats: every value is an integer below `2^24`, so it is exact, and
//! every division is corrected to the exact quotient.

use neptron_types::Filter;

/// One build of the fragment program: the filter is a constant of the
/// source, not a uniform. The `vc4` compiler of the Raspberry Pi turns a
/// branch on a uniform into a select, so with a uniform every pixel runs
/// the sharp bilinear path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Variant {
    /// Sharp bilinear, else nearest.
    pub(super) sharp: bool,
}

/// The number of programs: one per filter.
pub(super) const VARIANTS: usize = 2;

impl Variant {
    /// The program for `filter`.
    pub(super) fn of(filter: Filter) -> Self {
        Self {
            sharp: filter == Filter::SharpBilinear,
        }
    }

    /// Every variant, in [`index`](Self::index) order.
    pub(super) const ALL: [Variant; VARIANTS] = [Self { sharp: false }, Self { sharp: true }];

    /// The place of this variant in [`ALL`](Self::ALL).
    pub(super) fn index(self) -> usize {
        usize::from(self.sharp)
    }

    /// The fragment shader source of this variant.
    pub(super) fn fragment(self) -> String {
        format!(
            "#version 100\n#define SHARP {}\n{FRAGMENT_BODY}",
            u8::from(self.sharp)
        )
    }
}

/// Maps the unit square to the visible rectangle, given in output pixels
/// from the top left.
pub(super) const VERTEX: &str = r"#version 100
attribute vec2 a_corner;
uniform vec4 u_visible;
uniform vec4 u_output;

void main() {
    vec2 p = (u_visible.xy + a_corner * u_visible.zw) / u_output.xy * 2.0 - 1.0;
    gl_Position = vec4(p.x, -p.y, 0.0, 1.0);
}
";

/// The texture unit decodes the source pixels, from a texture in a GL
/// format of the pixel format. `SHARP` comes from [`Variant::fragment`].
const FRAGMENT_BODY: &str = r"#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
#error the renderer needs highp floats in the fragment shader
#endif

uniform sampler2D u_texture;
uniform vec4 u_output;
uniform vec4 u_visible;
uniform vec4 u_dest;
uniform vec4 u_crop;
// The frame width and height in pixels, then their inverses.
uniform vec4 u_frame;
// Period, first dark line and kept percent of the scanlines; period 0 for
// none.
uniform vec4 u_scan;

// floor(a / b) and the remainder, exact for integers a < 2^24 and b > 0.
// The float quotient can be one off, and the correction fixes that.
vec2 divmod(float a, float b) {
    float q = floor(a / b);
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

// The tap of destination index i on one axis: (near, far, weight, inside).
// This is AxisMap::tap of the reference renderer.
vec4 tap(float i, float crop_start, float n, float m, float frame_len) {
    float den = 2.0 * m;
    vec2 qr = divmod((2.0 * i + 1.0) * n, den);
    float near = crop_start + qr.x;
    if (near < 0.0 || near >= frame_len) {
        return vec4(0.0);
    }
#if SHARP == 0
    return vec4(near, near, 0.0, 1.0);
#else
    float prescale = max(1.0, divmod(m, n).x);
    // The reference weight is max(0, dist * p - (p - 1) * den), over 4m.
    // It equals den - (den - dist) * p, and that product is exact whenever
    // it is below den.
    float e = den - abs(2.0 * qr.y - den);
    float weight = max(0.0, den - e * prescale);
    float first = max(crop_start, 0.0);
    float last = min(crop_start + n, frame_len) - 1.0;
    float far = 2.0 * qr.y < den ? near - 1.0 : near + 1.0;
    return vec4(near, clamp(far, first, last), weight / (4.0 * m), 1.0);
#endif
}

// The 8-bit channels of source pixel (x, y), from 0 to 255. GL gives a
// channel v of b bits as v / (2^b - 1); the nearest integer of 255 times
// it is the expansion of the reference, and no such value is within float
// error of a half.
vec3 pixel(float x, float y) {
    vec2 uv = vec2((x + 0.5) * u_frame.z, (y + 0.5) * u_frame.w);
    return floor(texture2D(u_texture, uv).rgb * 255.0 + 0.5);
}

// round(v * keep / 100) with a half rounded up, exact for an integer
// channel v: v * keep + 50 is an integer below 2^15.
float darken(float v, float keep) {
    return divmod(v * keep + 50.0, 100.0).x;
}

void main() {
    // Only the integer part of gl_FragCoord names the pixel: GL puts it at
    // the pixel centre, but vc4 gives the corner, and the output row
    // counted from the top must not depend on which.
    float i = floor(gl_FragCoord.x) - u_visible.x + u_dest.x;
    float j = u_output.y - 1.0 - floor(gl_FragCoord.y) - u_visible.y + u_dest.y;
    if (i < 0.0 || j < 0.0 || i >= u_dest.z || j >= u_dest.w) {
        discard;
    }
    vec4 tx = tap(i, u_crop.x, u_crop.z, u_dest.z, u_frame.x);
    vec4 ty = tap(j, u_crop.y, u_crop.w, u_dest.w, u_frame.y);
    if (tx.w < 0.5 || ty.w < 0.5) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    // j is the destination line, so the pattern follows the source lines
    // and not the output line parity.
    bool dark = u_scan.x > 0.5 && divmod(j, u_scan.x).y >= u_scan.y;
    vec3 c = pixel(tx.x, ty.x);
    if (tx.z > 0.0 || ty.z > 0.0) {
        c = c * (1.0 - tx.z) * (1.0 - ty.z)
            + pixel(tx.y, ty.x) * tx.z * (1.0 - ty.z)
            + pixel(tx.x, ty.y) * (1.0 - tx.z) * ty.z
            + pixel(tx.y, ty.y) * tx.z * ty.z;
        // The reference scales the exact blend before its one rounding.
        if (dark) {
            c = c * (u_scan.z / 100.0);
        }
        // Round half up as the reference does, so the output is an exact
        // n / 255 and the conversion to 8 bits in GL does not round again.
        c = floor(c + 0.5);
    } else if (dark) {
        c = vec3(darken(c.r, u_scan.z), darken(c.g, u_scan.z), darken(c.b, u_scan.z));
    }
    gl_FragColor = vec4(c / 255.0, 1.0);
}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_its_own_index() {
        let all = Variant::ALL;
        for (i, variant) in all.iter().enumerate() {
            assert_eq!(variant.index(), i, "{variant:?}");
        }
        for filter in [Filter::Nearest, Filter::SharpBilinear] {
            let variant = Variant::of(filter);
            assert_eq!(all[variant.index()], variant);
        }
    }

    /// The filter is a constant of each program, and the texture unit
    /// decodes the pixel format: no source reads either from a uniform, so
    /// no driver has a branch on them.
    #[test]
    fn a_variant_fixes_the_filter_in_its_source() {
        for variant in Variant::ALL {
            let source = variant.fragment();
            assert!(source.starts_with("#version 100\n"), "{variant:?}");
            let sharp = format!("#define SHARP {}\n", u8::from(variant.sharp));
            assert!(source.contains(&sharp), "{variant:?}");
            assert!(!source.contains("u_mode"), "{variant:?}");
            assert!(!source.contains("FORMAT"), "{variant:?}");
        }
    }
}
