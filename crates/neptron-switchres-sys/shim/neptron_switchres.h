/*
 * The C interface of the NEPTRON shim around the Switchres calculation
 * sources. The Rust declarations in src/ffi.rs mirror this file.
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

#ifndef NEPTRON_SWITCHRES_H
#define NEPTRON_SWITCHRES_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* MAX_RANGES of monitor.h. */
#define NSR_MAX_RANGES 10

#define NSR_OK 0
#define NSR_BAD_ARGUMENT 1
#define NSR_BAD_RANGE 2
#define NSR_EXCEPTION 3
#define NSR_BAD_REQUEST 4
#define NSR_BAD_SETTINGS 5

/*
 * The limits of the values that the safe Rust types can still hold and
 * Switchres cannot take. Below NSR_REFRESH_MIN_HZ the float that get_mode
 * makes can reach 0, and scale_into_range then loops forever; above
 * NSR_REFRESH_MAX_HZ the float conversion can overflow. Outside the aspect
 * limits, the width that modeline_create scales by the aspect can overflow
 * int.
 */
#define NSR_REFRESH_MIN_HZ 0.001
#define NSR_REFRESH_MAX_HZ 10000000.0
#define NSR_MONITOR_ASPECT_MIN 0.25
#define NSR_MONITOR_ASPECT_MAX 4.0
#define NSR_BAD_BLANKING 6

/*
 * The limits on the blanking of a range. Switchres loops forever (and
 * overflows int) in get_line_params when the horizontal blanking fills the
 * line, divides by zero or less in total_lines_for_yres when the vertical
 * blanking fills the field, and divides by zero in max_vfreq_for_yres when
 * the vertical blanking is shorter than half a line and the range allows
 * zero lines.
 *
 * - Horizontal blanking below 0.75 of the shortest line (1 / hfreq_max).
 *   The line rate of a mode is at most about 1.2% above hfreq_max (whole
 *   lines, plus the half line of interlace), so the blanking fills at most
 *   0.759 of the line of a mode, and get_line_params ends for every
 *   fraction below 1.
 * - Vertical blanking below 0.5 of the shortest field (1 / vfreq_max). No
 *   mode has a field rate above vfreq_max and the border stays 0, so the
 *   divisor 1 - vfreq * vblank stays above 0.5.
 * - Vertical blanking of at least one line at hfreq_max, so that the
 *   divisor of max_vfreq_for_yres is at least 1.
 *
 * Every shipped preset passes: the largest horizontal fraction is 0.501
 * (pc_70_120), the largest vertical fraction is 0.141 (d9200).
 */
#define NSR_HBLANK_MAX_FRACTION 0.75
#define NSR_VBLANK_MAX_FRACTION 0.5
#define NSR_VBLANK_MIN_LINES 1.0

#define NSR_FAULT_NONE 0
#define NSR_FAULT_HBLANK_TOO_LONG 1
#define NSR_FAULT_VBLANK_TOO_LONG 2
#define NSR_FAULT_VBLANK_TOO_SHORT 3

/* The result bits of modeline.h. */
#define NSR_WEIGHT_REFRESH_OFF 0x1
#define NSR_WEIGHT_STRETCHED 0x2
#define NSR_WEIGHT_OUT_OF_RANGE 0x4

/*
 * One crt_range line, in the units of switchres.ini: frequencies in Hz,
 * horizontal times in microseconds, vertical times in milliseconds.
 */
typedef struct nsr_range {
	double hfreq_min_hz;
	double hfreq_max_hz;
	double vfreq_min_hz;
	double vfreq_max_hz;
	double hfront_porch_us;
	double hsync_pulse_us;
	double hback_porch_us;
	double vfront_porch_ms;
	double vsync_pulse_ms;
	double vback_porch_ms;
	int32_t hsync_positive;
	int32_t vsync_positive;
	int32_t progressive_lines_min;
	int32_t progressive_lines_max;
	int32_t interlaced_lines_min;
	int32_t interlaced_lines_max;
} nsr_range;

/*
 * The generator settings that NEPTRON sets. The geometry adjustment and the
 * vertical shift correction stay off: with the correction on, a large
 * interlaced range gives a negative border and an overflow in Switchres.
 */
typedef struct nsr_settings {
	uint64_t pclock_min_hz;
	double monitor_aspect;
	double refresh_tolerance_hz;
	int32_t interlace;
	int32_t doublescan;
	int32_t super_width;
	int32_t pixel_precision;
	int32_t interlace_force_even;
	int32_t scale_proportional;
} nsr_settings;

/* The arguments of display_manager::get_mode. */
typedef struct nsr_request {
	double refresh_hz;
	int32_t width;
	int32_t height;
	int32_t interlaced;
	int32_t rotated;
} nsr_request;

/* One calculated modeline with its score. */
typedef struct nsr_mode {
	uint64_t pclock_hz;
	double x_scale;
	double y_scale;
	double v_scale;
	double x_diff;
	double y_diff;
	double v_diff_hz;
	int32_t hactive;
	int32_t hbegin;
	int32_t hend;
	int32_t htotal;
	int32_t vactive;
	int32_t vbegin;
	int32_t vend;
	int32_t vtotal;
	int32_t interlace;
	int32_t doublescan;
	int32_t hsync_positive;
	int32_t vsync_positive;
	int32_t weight;
	int32_t range_index;
} nsr_mode;

/*
 * Why the shim refuses a range. For NSR_BAD_BLANKING, `kind` is an
 * NSR_FAULT_ value, and `blanking` and `limit` are in microseconds for the
 * horizontal blanking and in milliseconds for the vertical blanking.
 */
typedef struct nsr_fault {
	double blanking;
	double limit;
	uint64_t range_index;
	int32_t kind;
	int32_t padding;
} nsr_fault;

/*
 * Calculates one modeline per range and writes the ones in range to `out`,
 * best first, as Switchres ranks them. Returns an NSR_ status. On
 * NSR_BAD_RANGE and NSR_BAD_BLANKING, `*fault` names the range: a range
 * with a value that is not finite, one that Switchres refuses, or one
 * whose blanking is outside the limits above. NSR_BAD_REQUEST and
 * NSR_BAD_SETTINGS report a value outside the limits above. The function
 * catches every C++ exception.
 */
int32_t nsr_modelines(
	const nsr_range *ranges,
	size_t range_count,
	const nsr_settings *settings,
	const nsr_request *request,
	nsr_mode *out,
	size_t out_capacity,
	size_t *out_count,
	nsr_fault *fault);

#ifdef __cplusplus
}
#endif

#endif
