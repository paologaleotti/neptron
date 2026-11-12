// SPDX-License-Identifier: MIT OR Apache-2.0
//
// neptron-vkms-probe: commits user modelines through the atomic KMS API and
// reports what the driver does with them: the test-only and the real commit,
// the mode read back, vblank periods, flip events, frame CRCs and the
// writeback picture. It is a test tool for the QEMU images; it changes no
// state that outlives it. It refuses every device whose driver is not vkms,
// before its first commit.
//
// It reports and does not judge the timing: periods are per vblank sequence
// step, from the event timestamps, next to the period that the mode gives.
//
// Usage: neptron-vkms-probe [-d DEVICE] [-n FRAMES] MODE...
// MODE:  "NAME CLOCK_KHZ HDISP HSS HSE HTOTAL VDISP VSS VSE VTOTAL [FLAGS]"
// FLAGS: any of +hsync -hsync +vsync -vsync interlace doublescan
//
// Every result is one "key: value" line. The exit code is the number of
// failed commits, readbacks, CRC reads and writeback captures, at most 100.

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <poll.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#include <drm_fourcc.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

static int failures;

static void fail(const char *what, int err)
{
	printf("FAIL: %s: %s\n", what, strerror(err));
	failures++;
}

struct buffer {
	uint32_t handle, pitch, fb_id, width, height;
	uint64_t size;
	uint32_t *map;
};

struct kms {
	int fd;
	uint32_t crtc_id, crtc_index, conn_id, plane_id, wb_conn_id;
	char debugfs[300];
};

static uint32_t prop_id(int fd, uint32_t obj, uint32_t type, const char *name)
{
	drmModeObjectProperties *props = drmModeObjectGetProperties(fd, obj, type);
	uint32_t id = 0;

	for (uint32_t i = 0; props && i < props->count_props && !id; i++) {
		drmModePropertyRes *p = drmModeGetProperty(fd, props->props[i]);
		if (p && strcmp(p->name, name) == 0)
			id = p->prop_id;
		drmModeFreeProperty(p);
	}
	drmModeFreeObjectProperties(props);
	if (!id)
		printf("FAIL: no property %s on object %u\n", name, obj), failures++;
	return id;
}

static uint64_t prop_value(int fd, uint32_t obj, uint32_t type, const char *name)
{
	drmModeObjectProperties *props = drmModeObjectGetProperties(fd, obj, type);
	uint64_t value = 0;

	for (uint32_t i = 0; props && i < props->count_props; i++) {
		drmModePropertyRes *p = drmModeGetProperty(fd, props->props[i]);
		if (p && strcmp(p->name, name) == 0)
			value = props->prop_values[i];
		drmModeFreeProperty(p);
	}
	drmModeFreeObjectProperties(props);
	return value;
}

// Adds a property to a request; a missing property (id 0) is already counted
// as a failure by prop_id and is left out of the request.
static void add_prop(drmModeAtomicReq *req, uint32_t obj, uint32_t prop, uint64_t value)
{
	if (prop)
		drmModeAtomicAddProperty(req, obj, prop, value);
}

static const char *conn_type(uint32_t type)
{
	const char *name = drmModeGetConnectorTypeName(type);
	return name ? name : "unknown";
}

static void find_debugfs(struct kms *k)
{
	DIR *dir = opendir("/sys/kernel/debug/dri");
	struct dirent *e;

	k->debugfs[0] = '\0';
	while (dir && (e = readdir(dir))) {
		char path[320], name[64] = "";
		FILE *f;

		if (e->d_name[0] == '.')
			continue;
		snprintf(path, sizeof(path), "/sys/kernel/debug/dri/%s/name", e->d_name);
		f = fopen(path, "r");
		if (!f)
			continue;
		if (fgets(name, sizeof(name), f) && strncmp(name, "vkms", 4) == 0)
			snprintf(k->debugfs, sizeof(k->debugfs), "/sys/kernel/debug/dri/%s", e->d_name);
		fclose(f);
		if (k->debugfs[0])
			break;
	}
	if (dir)
		closedir(dir);
	printf("debugfs: %s\n", k->debugfs[0] ? k->debugfs : "not found");
}

static int setup(struct kms *k, const char *device)
{
	drmModeRes *res;
	drmModePlaneRes *planes;
	drmVersion *ver;
	uint64_t cap;

	k->fd = open(device, O_RDWR | O_CLOEXEC);
	if (k->fd < 0) {
		fail(device, errno);
		return -1;
	}
	ver = drmGetVersion(k->fd);
	if (!ver) {
		fail("get version", errno);
		return -1;
	}
	printf("driver: %s %d.%d.%d (%s)\n", ver->name, ver->version_major,
	       ver->version_minor, ver->version_patchlevel, ver->desc);
	// The probe commits modelines that no validator has seen. On a real
	// GPU that drives a monitor this is unsafe, so it runs on vkms only.
	if (strcmp(ver->name, "vkms") != 0) {
		printf("FAIL: the driver of %s is %s, not vkms; the probe runs only on vkms\n",
		       device, ver->name);
		failures++;
		drmFreeVersion(ver);
		return -1;
	}
	drmFreeVersion(ver);

	if (drmSetClientCap(k->fd, DRM_CLIENT_CAP_UNIVERSAL_PLANES, 1) ||
	    drmSetClientCap(k->fd, DRM_CLIENT_CAP_ATOMIC, 1)) {
		fail("atomic client cap", errno);
		return -1;
	}
	printf("cap writeback connectors: %s\n",
	       drmSetClientCap(k->fd, DRM_CLIENT_CAP_WRITEBACK_CONNECTORS, 1) ? "refused" : "accepted");
	if (drmGetCap(k->fd, DRM_CAP_CRTC_IN_VBLANK_EVENT, &cap) == 0)
		printf("cap crtc in vblank event: %" PRIu64 "\n", cap);
	if (drmGetCap(k->fd, DRM_CAP_TIMESTAMP_MONOTONIC, &cap) == 0)
		printf("cap monotonic timestamps: %" PRIu64 "\n", cap);

	res = drmModeGetResources(k->fd);
	if (!res) {
		fail("get resources", errno);
		return -1;
	}
	printf("crtcs: %d\nencoders: %d\nconnectors: %d\n", res->count_crtcs,
	       res->count_encoders, res->count_connectors);
	printf("size limits: %ux%u to %ux%u\n", res->min_width, res->min_height,
	       res->max_width, res->max_height);
	if (res->count_crtcs < 1) {
		printf("FAIL: no CRTC\n");
		failures++;
		drmModeFreeResources(res);
		return -1;
	}
	k->crtc_id = res->crtcs[0];
	k->crtc_index = 0;
	for (int i = 0; i < res->count_connectors; i++) {
		drmModeConnector *c = drmModeGetConnector(k->fd, res->connectors[i]);
		if (!c)
			continue;
		printf("connector %u: %s-%u, %s, %d probed modes\n", c->connector_id,
		       conn_type(c->connector_type), c->connector_type_id,
		       c->connection == DRM_MODE_CONNECTED ? "connected" : "not connected",
		       c->count_modes);
		for (int m = 0; m < c->count_modes && m < 3; m++)
			printf("  probed mode: %s %u kHz%s\n", c->modes[m].name, c->modes[m].clock,
			       c->modes[m].flags & DRM_MODE_FLAG_INTERLACE ? " interlaced" : "");
		if (c->connector_type == DRM_MODE_CONNECTOR_WRITEBACK)
			k->wb_conn_id = c->connector_id;
		else if (!k->conn_id && c->connection == DRM_MODE_CONNECTED)
			k->conn_id = c->connector_id;
		drmModeFreeConnector(c);
	}
	drmModeFreeResources(res);
	printf("writeback connector: %s\n", k->wb_conn_id ? "present" : "absent");

	planes = drmModeGetPlaneResources(k->fd);
	for (uint32_t i = 0; planes && i < planes->count_planes; i++) {
		drmModePlane *p = drmModeGetPlane(k->fd, planes->planes[i]);
		uint64_t type;

		if (!p) {
			fail("get plane", errno);
			continue;
		}
		type = prop_value(k->fd, p->plane_id, DRM_MODE_OBJECT_PLANE, "type");
		printf("plane %u: type %s, %u formats\n", p->plane_id,
		       type == DRM_PLANE_TYPE_PRIMARY ? "primary" :
		       type == DRM_PLANE_TYPE_CURSOR ? "cursor" : "overlay",
		       p->count_formats);
		if (!k->plane_id && type == DRM_PLANE_TYPE_PRIMARY &&
		    (p->possible_crtcs & (1u << k->crtc_index)))
			k->plane_id = p->plane_id;
		drmModeFreePlane(p);
	}
	drmModeFreePlaneResources(planes);
	if (!k->conn_id || !k->plane_id) {
		printf("FAIL: no connected connector or primary plane\n");
		failures++;
		return -1;
	}
	find_debugfs(k);
	return 0;
}

static int make_buffer(int fd, uint32_t w, uint32_t h, struct buffer *b)
{
	struct drm_mode_create_dumb create = { .width = w, .height = h, .bpp = 32 };
	struct drm_mode_map_dumb map = { 0 };
	uint32_t handles[4] = { 0 }, pitches[4] = { 0 }, offsets[4] = { 0 };

	memset(b, 0, sizeof(*b));
	if (drmIoctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &create))
		return -errno;
	b->handle = create.handle;
	b->pitch = create.pitch;
	b->size = create.size;
	b->width = w;
	b->height = h;
	handles[0] = b->handle;
	pitches[0] = b->pitch;
	if (drmModeAddFB2(fd, w, h, DRM_FORMAT_XRGB8888, handles, pitches, offsets, &b->fb_id, 0))
		return -errno;
	map.handle = b->handle;
	if (drmIoctl(fd, DRM_IOCTL_MODE_MAP_DUMB, &map))
		return -errno;
	b->map = mmap(NULL, b->size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, map.offset);
	if (b->map == MAP_FAILED) {
		b->map = NULL;
		return -errno;
	}
	return 0;
}

static void free_buffer(int fd, struct buffer *b)
{
	struct drm_mode_destroy_dumb destroy = { .handle = b->handle };

	if (b->map)
		munmap(b->map, b->size);
	if (b->fb_id)
		drmModeRmFB(fd, b->fb_id);
	if (b->handle)
		drmIoctl(fd, DRM_IOCTL_MODE_DESTROY_DUMB, &destroy);
	memset(b, 0, sizeof(*b));
}

static uint32_t pattern(uint32_t x, uint32_t y)
{
	return ((x * 255 / 64) & 0xff) << 16 | ((y * 255 / 64) & 0xff) << 8 | ((x ^ y) & 0xff);
}

static void fill(struct buffer *b)
{
	for (uint32_t y = 0; y < b->height; y++)
		for (uint32_t x = 0; x < b->width; x++)
			b->map[y * (b->pitch / 4) + x] = pattern(x, y);
}

static int parse_mode(const char *text, drmModeModeInfo *m)
{
	char name[DRM_DISPLAY_MODE_LEN] = "", rest[128] = "";
	unsigned v[9];
	int n;

	memset(m, 0, sizeof(*m));
	n = sscanf(text, "%31s %u %u %u %u %u %u %u %u %u %127[^\n]", name, &v[0], &v[1],
		   &v[2], &v[3], &v[4], &v[5], &v[6], &v[7], &v[8], rest);
	if (n < 10)
		return -1;
	snprintf(m->name, sizeof(m->name), "%s", name);
	m->clock = v[0];
	m->hdisplay = v[1], m->hsync_start = v[2], m->hsync_end = v[3], m->htotal = v[4];
	m->vdisplay = v[5], m->vsync_start = v[6], m->vsync_end = v[7], m->vtotal = v[8];
	for (char *tok = strtok(rest, " "); tok; tok = strtok(NULL, " ")) {
		if (!strcmp(tok, "+hsync"))
			m->flags |= DRM_MODE_FLAG_PHSYNC;
		else if (!strcmp(tok, "-hsync"))
			m->flags |= DRM_MODE_FLAG_NHSYNC;
		else if (!strcmp(tok, "+vsync"))
			m->flags |= DRM_MODE_FLAG_PVSYNC;
		else if (!strcmp(tok, "-vsync"))
			m->flags |= DRM_MODE_FLAG_NVSYNC;
		else if (!strcmp(tok, "interlace"))
			m->flags |= DRM_MODE_FLAG_INTERLACE;
		else if (!strcmp(tok, "doublescan"))
			m->flags |= DRM_MODE_FLAG_DBLSCAN;
		else
			return -1;
	}
	m->type = DRM_MODE_TYPE_USERDEF;
	m->vrefresh = (uint32_t)((uint64_t)m->clock * 1000 / ((uint64_t)m->htotal * m->vtotal));
	return 0;
}

static void print_mode(const char *label, const drmModeModeInfo *m)
{
	printf("%s: %u kHz %u %u %u %u %u %u %u %u flags 0x%x\n", label, m->clock, m->hdisplay,
	       m->hsync_start, m->hsync_end, m->htotal, m->vdisplay, m->vsync_start,
	       m->vsync_end, m->vtotal, m->flags);
}

static int commit_mode(struct kms *k, const drmModeModeInfo *mode, struct buffer *fb,
		       uint32_t flags)
{
	drmModeAtomicReq *req = drmModeAtomicAlloc();
	uint32_t blob = 0;
	int fd = k->fd, ret;

	if (drmModeCreatePropertyBlob(fd, mode, sizeof(*mode), &blob))
		return -errno;
	add_prop(req, k->crtc_id, prop_id(fd, k->crtc_id, DRM_MODE_OBJECT_CRTC, "MODE_ID"), blob);
	add_prop(req, k->crtc_id, prop_id(fd, k->crtc_id, DRM_MODE_OBJECT_CRTC, "ACTIVE"), 1);
	add_prop(req, k->conn_id, prop_id(fd, k->conn_id, DRM_MODE_OBJECT_CONNECTOR, "CRTC_ID"), k->crtc_id);
#define PLANE(name, value) \
	add_prop(req, k->plane_id, prop_id(fd, k->plane_id, DRM_MODE_OBJECT_PLANE, name), value)
	PLANE("FB_ID", fb->fb_id);
	PLANE("CRTC_ID", k->crtc_id);
	PLANE("SRC_X", 0);
	PLANE("SRC_Y", 0);
	PLANE("SRC_W", (uint64_t)fb->width << 16);
	PLANE("SRC_H", (uint64_t)fb->height << 16);
	PLANE("CRTC_X", 0);
	PLANE("CRTC_Y", 0);
	PLANE("CRTC_W", fb->width);
	PLANE("CRTC_H", fb->height);
#undef PLANE
	ret = drmModeAtomicCommit(fd, req, flags | DRM_MODE_ATOMIC_ALLOW_MODESET, NULL) ? -errno : 0;
	drmModeAtomicFree(req);
	drmModeDestroyPropertyBlob(fd, blob);
	return ret;
}

static void disable(struct kms *k)
{
	drmModeAtomicReq *req = drmModeAtomicAlloc();
	int fd = k->fd;

	add_prop(req, k->crtc_id, prop_id(fd, k->crtc_id, DRM_MODE_OBJECT_CRTC, "MODE_ID"), 0);
	add_prop(req, k->crtc_id, prop_id(fd, k->crtc_id, DRM_MODE_OBJECT_CRTC, "ACTIVE"), 0);
	add_prop(req, k->conn_id, prop_id(fd, k->conn_id, DRM_MODE_OBJECT_CONNECTOR, "CRTC_ID"), 0);
	add_prop(req, k->plane_id, prop_id(fd, k->plane_id, DRM_MODE_OBJECT_PLANE, "FB_ID"), 0);
	add_prop(req, k->plane_id, prop_id(fd, k->plane_id, DRM_MODE_OBJECT_PLANE, "CRTC_ID"), 0);
	if (drmModeAtomicCommit(fd, req, DRM_MODE_ATOMIC_ALLOW_MODESET, NULL))
		fail("disable", errno);
	drmModeAtomicFree(req);
}

static void check_readback(struct kms *k, const drmModeModeInfo *want)
{
	drmModeCrtc *crtc = drmModeGetCrtc(k->fd, k->crtc_id);
	const drmModeModeInfo *got;
	bool equal;

	if (!crtc || !crtc->mode_valid) {
		printf("FAIL: readback: no active mode\n");
		failures++;
		drmModeFreeCrtc(crtc);
		return;
	}
	got = &crtc->mode;
	print_mode("readback", got);
	equal = got->clock == want->clock && got->hdisplay == want->hdisplay &&
		got->hsync_start == want->hsync_start && got->hsync_end == want->hsync_end &&
		got->htotal == want->htotal && got->vdisplay == want->vdisplay &&
		got->vsync_start == want->vsync_start && got->vsync_end == want->vsync_end &&
		got->vtotal == want->vtotal && got->flags == want->flags;
	printf("readback equal: %s\n", equal ? "yes" : "no");
	if (!equal)
		failures++;
	drmModeFreeCrtc(crtc);
}

// The refresh period that the mode gives: one field for an interlaced mode.
static uint64_t expected_period_ns(const drmModeModeInfo *m)
{
	uint64_t ns = (uint64_t)m->htotal * m->vtotal * 1000000ull / m->clock;

	if (m->flags & DRM_MODE_FLAG_DBLSCAN)
		ns *= 2;
	if (m->flags & DRM_MODE_FLAG_INTERLACE)
		ns /= 2;
	return ns;
}

static void check_vblank(struct kms *k, const drmModeModeInfo *m, int frames)
{
	drmVBlank vbl;
	uint64_t first_us = 0, last_us = 0;
	uint32_t first_seq = 0, last_seq = 0;

	for (int i = 0; i <= frames; i++) {
		memset(&vbl, 0, sizeof(vbl));
		vbl.request.type = DRM_VBLANK_RELATIVE;
		vbl.request.sequence = 1;
		if (drmWaitVBlank(k->fd, &vbl)) {
			fail("wait vblank", errno);
			return;
		}
		last_us = (uint64_t)vbl.reply.tval_sec * 1000000 + vbl.reply.tval_usec;
		last_seq = vbl.reply.sequence;
		if (i == 0)
			first_us = last_us, first_seq = last_seq;
	}
	printf("vblank: %d waits, sequence %u to %u, period %" PRIu64 " ns, mode period %" PRIu64 " ns\n",
	       frames, first_seq, last_seq,
	       last_seq > first_seq ? (last_us - first_us) * 1000 / (last_seq - first_seq) : 0,
	       expected_period_ns(m));
	if (m->flags & DRM_MODE_FLAG_INTERLACE)
		printf("vblank note: for an interlaced mode the vkms vblank interval is half the field "
		       "period on this kernel: crtc_vtotal is already halved for interlace, and the "
		       "timestamping code halves the frame duration again\n");
}

struct flip_state {
	int count;
	uint32_t first_seq, last_seq;
	uint64_t first_us, last_us;
	uint32_t crtc_id;
};

static void on_flip(int fd, unsigned seq, unsigned sec, unsigned usec, unsigned crtc_id, void *data)
{
	struct flip_state *s = data;
	uint64_t us = (uint64_t)sec * 1000000 + usec;

	(void)fd;
	if (s->count++ == 0)
		s->first_seq = seq, s->first_us = us;
	s->last_seq = seq, s->last_us = us;
	s->crtc_id = crtc_id;
}

static void check_flips(struct kms *k, struct buffer *fb, int frames)
{
	drmEventContext ctx = { .version = 3, .page_flip_handler2 = on_flip };
	struct flip_state s = { 0 };
	uint32_t fb_prop = prop_id(k->fd, k->plane_id, DRM_MODE_OBJECT_PLANE, "FB_ID");

	for (int i = 0; i <= frames; i++) {
		drmModeAtomicReq *req = drmModeAtomicAlloc();
		struct pollfd pfd = { .fd = k->fd, .events = POLLIN };

		add_prop(req, k->plane_id, fb_prop, fb->fb_id);
		if (drmModeAtomicCommit(k->fd, req, DRM_MODE_PAGE_FLIP_EVENT | DRM_MODE_ATOMIC_NONBLOCK, &s)) {
			fail("flip commit", errno);
			drmModeAtomicFree(req);
			return;
		}
		drmModeAtomicFree(req);
		if (poll(&pfd, 1, 1000) != 1) {
			printf("FAIL: no flip event within 1 s\n");
			failures++;
			return;
		}
		drmHandleEvent(k->fd, &ctx);
	}
	printf("flips: %d events, crtc %u, sequence %u to %u, period %" PRIu64 " ns\n",
	       s.count, s.crtc_id, s.first_seq, s.last_seq,
	       s.last_seq > s.first_seq ? (s.last_us - s.first_us) * 1000 / (s.last_seq - s.first_seq) : 0);
}

static void check_crc(struct kms *k, int frames)
{
	char path[400], line[128];
	int control, data;
	ssize_t n;

	if (!k->debugfs[0]) {
		printf("FAIL: crc: no debugfs directory\n");
		failures++;
		return;
	}
	snprintf(path, sizeof(path), "%s/crtc-%u/crc/control", k->debugfs, k->crtc_index);
	control = open(path, O_WRONLY);
	if (control < 0 || write(control, "auto", 4) != 4) {
		fail(path, errno);
		if (control >= 0)
			close(control);
		return;
	}
	close(control);
	snprintf(path, sizeof(path), "%s/crtc-%u/crc/data", k->debugfs, k->crtc_index);
	data = open(path, O_RDONLY);
	if (data < 0) {
		fail(path, errno);
		return;
	}
	for (int i = 0; i < frames; i++) {
		struct pollfd pfd = { .fd = data, .events = POLLIN };

		if (poll(&pfd, 1, 1000) != 1) {
			printf("FAIL: crc: no entry within 1 s\n");
			failures++;
			break;
		}
		n = read(data, line, sizeof(line) - 1);
		if (n <= 0) {
			fail("crc read", n < 0 ? errno : EIO);
			break;
		}
		line[n] = '\0';
		line[strcspn(line, "\n")] = '\0';
		printf("crc: %s\n", line);
	}
	close(data);
}

static void check_writeback(struct kms *k, struct buffer *src)
{
	struct buffer wb;
	drmModeAtomicReq *req;
	int32_t fence = -1;
	struct pollfd pfd;
	uint64_t equal = 0, total = (uint64_t)src->width * src->height;
	int ret;

	if (!k->wb_conn_id) {
		printf("writeback: skipped, no connector\n");
		return;
	}
	ret = make_buffer(k->fd, src->width, src->height, &wb);
	if (ret) {
		fail("writeback buffer", -ret);
		free_buffer(k->fd, &wb);
		return;
	}
	memset(wb.map, 0, wb.size);
	req = drmModeAtomicAlloc();
	add_prop(req, k->wb_conn_id, prop_id(k->fd, k->wb_conn_id, DRM_MODE_OBJECT_CONNECTOR, "CRTC_ID"), k->crtc_id);
	add_prop(req, k->wb_conn_id, prop_id(k->fd, k->wb_conn_id, DRM_MODE_OBJECT_CONNECTOR, "WRITEBACK_FB_ID"), wb.fb_id);
	add_prop(req, k->wb_conn_id, prop_id(k->fd, k->wb_conn_id, DRM_MODE_OBJECT_CONNECTOR, "WRITEBACK_OUT_FENCE_PTR"), (uint64_t)(uintptr_t)&fence);
	ret = drmModeAtomicCommit(k->fd, req, DRM_MODE_ATOMIC_ALLOW_MODESET, NULL);
	drmModeAtomicFree(req);
	if (ret) {
		fail("writeback commit", errno);
		free_buffer(k->fd, &wb);
		return;
	}
	pfd = (struct pollfd){ .fd = fence, .events = POLLIN };
	if (fence < 0 || poll(&pfd, 1, 1000) != 1) {
		printf("FAIL: writeback: fence %d did not signal within 1 s\n", fence);
		failures++;
	} else {
		for (uint32_t y = 0; y < src->height; y++)
			for (uint32_t x = 0; x < src->width; x++)
				equal += (wb.map[y * (wb.pitch / 4) + x] & 0xffffff) == pattern(x, y);
		printf("writeback: %" PRIu64 " of %" PRIu64 " pixels equal the source\n", equal, total);
		if (equal != total)
			failures++;
	}
	if (fence >= 0)
		close(fence);
	req = drmModeAtomicAlloc();
	add_prop(req, k->wb_conn_id, prop_id(k->fd, k->wb_conn_id, DRM_MODE_OBJECT_CONNECTOR, "CRTC_ID"), 0);
	if (drmModeAtomicCommit(k->fd, req, DRM_MODE_ATOMIC_ALLOW_MODESET, NULL))
		fail("writeback detach", errno);
	drmModeAtomicFree(req);
	free_buffer(k->fd, &wb);
}

static void probe_mode(struct kms *k, const char *text, int frames)
{
	drmModeModeInfo mode;
	struct buffer fb;
	int ret;

	printf("\nmode: %s\n", text);
	if (parse_mode(text, &mode)) {
		printf("FAIL: cannot parse the mode\n");
		failures++;
		return;
	}
	print_mode("requested", &mode);
	ret = make_buffer(k->fd, mode.hdisplay, mode.vdisplay, &fb);
	if (ret) {
		fail("framebuffer", -ret);
		free_buffer(k->fd, &fb);
		return;
	}
	fill(&fb);
	ret = commit_mode(k, &mode, &fb, DRM_MODE_ATOMIC_TEST_ONLY);
	printf("test-only commit: %s\n", ret ? strerror(-ret) : "accepted");
	ret = commit_mode(k, &mode, &fb, 0);
	printf("commit: %s\n", ret ? strerror(-ret) : "accepted");
	if (ret) {
		failures++;
		free_buffer(k->fd, &fb);
		return;
	}
	check_readback(k, &mode);
	check_vblank(k, &mode, frames);
	check_flips(k, &fb, frames);
	check_crc(k, 3);
	check_writeback(k, &fb);
	disable(k);
	free_buffer(k->fd, &fb);
}

int main(int argc, char **argv)
{
	const char *device = "/dev/dri/card0";
	int frames = 60, opt;
	struct kms k = { 0 };

	while ((opt = getopt(argc, argv, "d:n:")) != -1) {
		if (opt == 'd')
			device = optarg;
		else if (opt == 'n' && atoi(optarg) > 0)
			frames = atoi(optarg);
		else
			return fprintf(stderr, "usage: %s [-d DEVICE] [-n FRAMES] MODE...\n", argv[0]), 100;
	}
	if (setup(&k, device))
		return failures ? failures : 1;
	for (int i = optind; i < argc; i++)
		probe_mode(&k, argv[i], frames);
	close(k.fd);
	printf("\nfailures: %d\n", failures);
	return failures > 100 ? 100 : failures;
}
