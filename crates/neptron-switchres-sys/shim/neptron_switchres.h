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
 * Calculates one modeline per range and writes the ones in range to `out`,
 * best first, as Switchres ranks them. Returns an NSR_ status. On
 * NSR_BAD_RANGE, `*bad_range` is the index of the range that is not finite
 * or that Switchres refuses. NSR_BAD_REQUEST and NSR_BAD_SETTINGS report a
 * value outside the limits above. The function catches every C++ exception.
 */
int32_t nsr_modelines(
	const nsr_range *ranges,
	size_t range_count,
	const nsr_settings *settings,
	const nsr_request *request,
	nsr_mode *out,
	size_t out_capacity,
	size_t *out_count,
	size_t *bad_range);

#ifdef __cplusplus
}
#endif

#endif
