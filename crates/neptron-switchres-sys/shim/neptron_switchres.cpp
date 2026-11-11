/*
 * The NEPTRON shim around the Switchres calculation sources.
 *
 * Switchres selects a mode in display_manager::get_mode (display.cpp). That
 * file also holds the display backends, so NEPTRON does not compile it. This
 * shim repeats the steps of get_mode for a display that can add a mode (the
 * case of the "--calc" option of the standalone program), and calls the
 * calculation functions of modeline.cpp and monitor.cpp for each step.
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

#include "neptron_switchres.h"

#include <cmath>
#include <cstring>

#include "modeline.h"
#include "monitor.h"

static_assert(sizeof(nsr_range) == 104, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, hfreq_min_hz) == 0, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, hfreq_max_hz) == 8, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, vfreq_min_hz) == 16, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, vfreq_max_hz) == 24, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, hfront_porch_us) == 32, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, hsync_pulse_us) == 40, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, hback_porch_us) == 48, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, vfront_porch_ms) == 56, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, vsync_pulse_ms) == 64, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, vback_porch_ms) == 72, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, hsync_positive) == 80, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, vsync_positive) == 84, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, progressive_lines_min) == 88, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, progressive_lines_max) == 92, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, interlaced_lines_min) == 96, "nsr_range layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_range, interlaced_lines_max) == 100, "nsr_range layout changed: update src/ffi.rs");
static_assert(sizeof(nsr_settings) == 48, "nsr_settings layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_settings, pclock_min_hz) == 0, "nsr_settings layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_settings, monitor_aspect) == 8, "nsr_settings layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_settings, refresh_tolerance_hz) == 16, "nsr_settings layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_settings, interlace) == 24, "nsr_settings layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_settings, doublescan) == 28, "nsr_settings layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_settings, super_width) == 32, "nsr_settings layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_settings, pixel_precision) == 36, "nsr_settings layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_settings, interlace_force_even) == 40, "nsr_settings layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_settings, scale_proportional) == 44, "nsr_settings layout changed: update src/ffi.rs");
static_assert(sizeof(nsr_request) == 24, "nsr_request layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_request, refresh_hz) == 0, "nsr_request layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_request, width) == 8, "nsr_request layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_request, height) == 12, "nsr_request layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_request, interlaced) == 16, "nsr_request layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_request, rotated) == 20, "nsr_request layout changed: update src/ffi.rs");
static_assert(sizeof(nsr_mode) == 112, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, pclock_hz) == 0, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, x_scale) == 8, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, y_scale) == 16, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, v_scale) == 24, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, x_diff) == 32, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, y_diff) == 40, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, v_diff_hz) == 48, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, hactive) == 56, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, hbegin) == 60, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, hend) == 64, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, htotal) == 68, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, vactive) == 72, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, vbegin) == 76, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, vend) == 80, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, vtotal) == 84, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, interlace) == 88, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, doublescan) == 92, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, hsync_positive) == 96, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, vsync_positive) == 100, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, weight) == 104, "nsr_mode layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_mode, range_index) == 108, "nsr_mode layout changed: update src/ffi.rs");
static_assert(sizeof(nsr_fault) == 32, "nsr_fault layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_fault, blanking) == 0, "nsr_fault layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_fault, limit) == 8, "nsr_fault layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_fault, range_index) == 16, "nsr_fault layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_fault, kind) == 24, "nsr_fault layout changed: update src/ffi.rs");
static_assert(offsetof(nsr_fault, padding) == 28, "nsr_fault layout changed: update src/ffi.rs");
static_assert(NSR_MAX_RANGES == MAX_RANGES, "MAX_RANGES of monitor.h changed");
static_assert(NSR_WEIGHT_REFRESH_OFF == R_V_FREQ_OFF, "result bits of modeline.h changed");
static_assert(NSR_WEIGHT_STRETCHED == R_RES_STRETCH, "result bits of modeline.h changed");
static_assert(NSR_WEIGHT_OUT_OF_RANGE == R_OUT_OF_RANGE, "result bits of modeline.h changed");

static monitor_range to_monitor_range(const nsr_range &in)
{
	monitor_range range;
	memset(&range, 0, sizeof(range));
	range.hfreq_min = in.hfreq_min_hz;
	range.hfreq_max = in.hfreq_max_hz;
	range.vfreq_min = in.vfreq_min_hz;
	range.vfreq_max = in.vfreq_max_hz;
	range.hfront_porch = in.hfront_porch_us;
	range.hsync_pulse = in.hsync_pulse_us;
	range.hback_porch = in.hback_porch_us;
	range.vfront_porch = in.vfront_porch_ms;
	range.vsync_pulse = in.vsync_pulse_ms;
	range.vback_porch = in.vback_porch_ms;
	range.hsync_polarity = in.hsync_positive;
	range.vsync_polarity = in.vsync_positive;
	range.progressive_lines_min = in.progressive_lines_min;
	range.progressive_lines_max = in.progressive_lines_max;
	range.interlaced_lines_min = in.interlaced_lines_min;
	range.interlaced_lines_max = in.interlaced_lines_max;

	// The same operations, in the same order, as monitor_fill_range after it
	// parses a crt_range line, so that the doubles are bit for bit the same.
	range.vfront_porch /= 1000;
	range.vsync_pulse /= 1000;
	range.vback_porch /= 1000;
	range.vertical_blank = (range.vfront_porch + range.vsync_pulse + range.vback_porch);
	return range;
}

static generator_settings to_generator_settings(const nsr_settings &in)
{
	generator_settings gs;
	memset(&gs, 0, sizeof(gs));
	gs.interlace = in.interlace;
	gs.doublescan = in.doublescan;
	gs.pclock_min = in.pclock_min_hz;
	gs.monitor_aspect = in.monitor_aspect;
	gs.refresh_tolerance = in.refresh_tolerance_hz;
	gs.super_width = in.super_width;
	gs.h_size = 1.0;
	gs.h_shift = 0;
	gs.v_shift = 0;
	gs.v_shift_correct = 0;
	gs.pixel_precision = in.pixel_precision;
	gs.interlace_force_even = in.interlace_force_even;
	gs.scale_proportional = in.scale_proportional;
	return gs;
}

static nsr_mode to_nsr_mode(const modeline &mode)
{
	nsr_mode out;
	memset(&out, 0, sizeof(out));
	out.pclock_hz = mode.pclock;
	out.x_scale = mode.result.x_scale;
	out.y_scale = mode.result.y_scale;
	out.v_scale = mode.result.v_scale;
	out.x_diff = mode.result.x_diff;
	out.y_diff = mode.result.y_diff;
	out.v_diff_hz = mode.result.v_diff;
	out.hactive = mode.hactive;
	out.hbegin = mode.hbegin;
	out.hend = mode.hend;
	out.htotal = mode.htotal;
	out.vactive = mode.vactive;
	out.vbegin = mode.vbegin;
	out.vend = mode.vend;
	out.vtotal = mode.vtotal;
	out.interlace = mode.interlace;
	out.doublescan = mode.doublescan;
	out.hsync_positive = mode.hsync;
	out.vsync_positive = mode.vsync;
	out.weight = mode.result.weight;
	out.range_index = mode.range;
	return out;
}

// Runs after monitor_evaluate_range, which keeps hfreq_max and vfreq_max
// above zero. The comparisons are written so that a NaN fails them.
static bool blanking_fits(const monitor_range &range, nsr_fault *fault)
{
	double hblank_us = range.hfront_porch + range.hsync_pulse + range.hback_porch;
	double hlimit_us = NSR_HBLANK_MAX_FRACTION * 1000000.0 / range.hfreq_max;
	if (!(hblank_us < hlimit_us))
	{
		fault->kind = NSR_FAULT_HBLANK_TOO_LONG;
		fault->blanking = hblank_us;
		fault->limit = hlimit_us;
		return false;
	}
	double vlimit_s = NSR_VBLANK_MAX_FRACTION / range.vfreq_max;
	if (!(range.vertical_blank < vlimit_s))
	{
		fault->kind = NSR_FAULT_VBLANK_TOO_LONG;
		fault->blanking = range.vertical_blank * 1000;
		fault->limit = vlimit_s * 1000;
		return false;
	}
	double vminimum_s = NSR_VBLANK_MIN_LINES / range.hfreq_max;
	if (!(range.vertical_blank >= vminimum_s))
	{
		fault->kind = NSR_FAULT_VBLANK_TOO_SHORT;
		fault->blanking = range.vertical_blank * 1000;
		fault->limit = vminimum_s * 1000;
		return false;
	}
	return true;
}

// The Rust types keep the sizes in 0 to 65535 and the booleans in 0 and 1.
// A zero size makes scale_into_range loop forever.
static bool request_is_valid(const nsr_request &request)
{
	return request.width > 0 && request.height > 0 &&
		request.refresh_hz >= NSR_REFRESH_MIN_HZ && request.refresh_hz <= NSR_REFRESH_MAX_HZ;
}

// The comparisons are written so that a NaN fails them.
static bool settings_are_valid(const nsr_settings &settings)
{
	return settings.monitor_aspect >= NSR_MONITOR_ASPECT_MIN &&
		settings.monitor_aspect <= NSR_MONITOR_ASPECT_MAX;
}

// monitor_evaluate_range lets a NaN through, because every comparison with
// it is false.
static bool range_is_finite(const nsr_range &range)
{
	const double values[] = {
		range.hfreq_min_hz, range.hfreq_max_hz, range.vfreq_min_hz, range.vfreq_max_hz,
		range.hfront_porch_us, range.hsync_pulse_us, range.hback_porch_us,
		range.vfront_porch_ms, range.vsync_pulse_ms, range.vback_porch_ms,
	};
	for (double value : values)
		if (!std::isfinite(value))
			return false;
	return true;
}

static int32_t modelines(
	const nsr_range *ranges,
	size_t range_count,
	const nsr_settings *settings,
	const nsr_request *request,
	nsr_mode *out,
	size_t out_capacity,
	size_t *out_count)
{
	monitor_range range[MAX_RANGES];
	modeline candidate[MAX_RANGES];
	bool taken[MAX_RANGES] = {};
	generator_settings gs = to_generator_settings(*settings);

	// get_mode receives the refresh as a float, so the value is rounded to
	// a float before Switchres sees it.
	float refresh = (float)request->refresh_hz;

	modeline s_mode = {};
	s_mode.interlace = request->interlaced ? 1 : 0;
	s_mode.vfreq = refresh;
	s_mode.hactive = request->width;
	s_mode.vactive = request->height;
	if (request->rotated)
	{
		int width = s_mode.hactive;
		s_mode.hactive = s_mode.vactive;
		s_mode.vactive = width;
		s_mode.type |= MODE_ROTATED;
	}

	// The mode that get_mode adds when the display can add a mode. The
	// desktop is not rotated.
	modeline new_mode = {};
	new_mode.type = XYV_EDITABLE | V_FREQ_EDITABLE | SCAN_EDITABLE | MODE_ADD;

	for (size_t i = 0; i < range_count; i++)
	{
		range[i] = to_monitor_range(ranges[i]);
		modeline t_mode = new_mode;
		t_mode.hactive = s_mode.hactive;
		t_mode.vactive = s_mode.vactive;
		t_mode.vfreq = s_mode.vfreq;
		modeline_create(&s_mode, &t_mode, &range[i], &gs);
		t_mode.range = (int)i;
		candidate[i] = t_mode;
	}

	// Each pass is the selection loop of get_mode over the candidates not
	// yet taken, so the first entry is the mode that get_mode returns.
	size_t count = 0;
	while (count < out_capacity)
	{
		modeline best_mode = {};
		best_mode.result.weight |= R_OUT_OF_RANGE;
		size_t best = MAX_RANGES;
		for (size_t i = 0; i < range_count; i++)
		{
			if (taken[i] || (candidate[i].result.weight & R_OUT_OF_RANGE))
				continue;
			if (modeline_compare(&candidate[i], &best_mode))
			{
				best_mode = candidate[i];
				best = i;
			}
		}
		if (best == MAX_RANGES)
			break;
		taken[best] = true;

		// modeline_adjust may clamp the settings it gets, so each mode gets
		// its own copy.
		generator_settings adjust_gs = gs;
		modeline_adjust(&best_mode, range[best].hfreq_max, &adjust_gs);
		out[count] = to_nsr_mode(best_mode);
		count++;
	}

	*out_count = count;
	return NSR_OK;
}

extern "C" int32_t nsr_modelines(
	const nsr_range *ranges,
	size_t range_count,
	const nsr_settings *settings,
	const nsr_request *request,
	nsr_mode *out,
	size_t out_capacity,
	size_t *out_count,
	nsr_fault *fault)
{
	try
	{
		if (ranges == nullptr || settings == nullptr || request == nullptr || out == nullptr ||
			out_count == nullptr || fault == nullptr)
			return NSR_BAD_ARGUMENT;
		*out_count = 0;
		memset(fault, 0, sizeof(*fault));
		if (range_count > MAX_RANGES || out_capacity < range_count)
			return NSR_BAD_ARGUMENT;
		if (!request_is_valid(*request))
			return NSR_BAD_REQUEST;
		if (!settings_are_valid(*settings))
			return NSR_BAD_SETTINGS;

		for (size_t i = 0; i < range_count; i++)
		{
			monitor_range range = to_monitor_range(ranges[i]);
			fault->range_index = i;
			if (!range_is_finite(ranges[i]) || monitor_evaluate_range(&range) != 0)
				return NSR_BAD_RANGE;
			if (!blanking_fits(range, fault))
				return NSR_BAD_BLANKING;
		}

		fault->range_index = 0;
		return modelines(ranges, range_count, settings, request, out, out_capacity, out_count);
	}
	catch (...)
	{
		return NSR_EXCEPTION;
	}
}
