//! The one program of the renderer, in GLSL ES 1.00. It draws the game
//! quad and the UI quad. The fragment shader repeats the integer rules of
//! the reference renderer in 32-bit floats: every value is an integer below
//! `2^24`, so it is exact, and every division is corrected to the exact
//! quotient.

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

/// The source pixels arrive as a one-byte texture: one texel per byte of a
/// row, so the shader reads the pixel bytes as they are in memory
/// (little-endian) and expands the channels itself.
pub(super) const FRAGMENT: &str = r"#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
#error the renderer needs highp floats in the fragment shader
#endif

uniform sampler2D u_texture;
uniform vec4 u_output;
uniform vec4 u_visible;
uniform vec4 u_dest;
uniform vec4 u_crop;
uniform vec4 u_frame;
uniform vec4 u_mode;

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
    if (u_mode.y < 0.5) {
        return vec4(near, near, 0.0, 1.0);
    }
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
}

float byte_at(float x, float y) {
    vec2 uv = vec2((x + 0.5) / u_frame.z, (y + 0.5) / u_frame.w);
    return floor(texture2D(u_texture, uv).r * 255.0 + 0.5);
}

// round(v * 255 / 31) and round(v * 255 / 63), as the reference rounds.
// The true fraction is a multiple of 1/31 (1/63), so a bias of half a step
// keeps floor away from an error of the float division.
float expand5(float v) {
    return floor((v * 255.0 + 15.0) / 31.0 + 0.5 / 31.0);
}

float expand6(float v) {
    return floor((v * 255.0 + 31.0) / 63.0 + 0.5 / 63.0);
}

// The 8-bit channels of source pixel (x, y), from 0 to 255.
vec3 pixel(float x, float y) {
    if (u_mode.x < 0.5) {
        float b = 4.0 * x;
        return vec3(byte_at(b + 2.0, y), byte_at(b + 1.0, y), byte_at(b, y));
    }
    float b = 2.0 * x;
    float v = byte_at(b, y) + 256.0 * byte_at(b + 1.0, y);
    // Products with powers of two are exact; so is each floor below.
    float blue = v - 32.0 * floor(v * (1.0 / 32.0));
    float high = floor(v * (1.0 / 32.0));
    if (u_mode.x < 1.5) {
        float green = high - 64.0 * floor(high * (1.0 / 64.0));
        float red = floor(v * (1.0 / 2048.0));
        return vec3(expand5(red), expand6(green), expand5(blue));
    }
    float green = high - 32.0 * floor(high * (1.0 / 32.0));
    float red5 = floor(v * (1.0 / 1024.0));
    float red = red5 - 32.0 * floor(red5 * (1.0 / 32.0));
    return vec3(expand5(red), expand5(green), expand5(blue));
}

void main() {
    // gl_FragCoord is at a pixel centre, half a pixel away from any edge,
    // so floor gives the pixel without a rounding risk.
    float i = floor(gl_FragCoord.x) - u_visible.x + u_dest.x;
    float j = floor(u_output.y - gl_FragCoord.y) - u_visible.y + u_dest.y;
    if (i < 0.0 || j < 0.0 || i >= u_dest.z || j >= u_dest.w) {
        discard;
    }
    vec4 tx = tap(i, u_crop.x, u_crop.z, u_dest.z, u_frame.x);
    vec4 ty = tap(j, u_crop.y, u_crop.w, u_dest.w, u_frame.y);
    if (tx.w < 0.5 || ty.w < 0.5) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    vec3 c = pixel(tx.x, ty.x);
    if (tx.z > 0.0 || ty.z > 0.0) {
        c = c * (1.0 - tx.z) * (1.0 - ty.z)
            + pixel(tx.y, ty.x) * tx.z * (1.0 - ty.z)
            + pixel(tx.x, ty.y) * (1.0 - tx.z) * ty.z
            + pixel(tx.y, ty.y) * tx.z * ty.z;
        // Round half up as the reference does, so the output is an exact
        // n / 255 and the conversion to 8 bits in GL does not round again.
        c = floor(c + 0.5);
    }
    gl_FragColor = vec4(c / 255.0, 1.0);
}
";
