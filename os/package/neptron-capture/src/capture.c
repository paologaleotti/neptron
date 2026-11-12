// SPDX-License-Identifier: MIT OR Apache-2.0
//
// neptron-capture: waits until the frontend shows a screen, then takes the
// writeback picture of the display that the launcher holds, and prints it as
// a binary PPM file in base64 between two marker lines. It is a test tool for
// the vkms driver in QEMU (`cargo xtask os boot-check`).
//
// Usage: neptron-capture [--screen ID] [--timeout-ms N] [--settle-ms N]
//
// Only the DRM master can commit, and the launcher holds master for the
// frontend. So the tool stops the frontend and the launcher (SIGSTOP), takes
// a copy of the launcher's descriptor with pidfd_getfd (the same open file
// description, so the same master), and routes the writeback connector to
// the CRTC. It changes no mode and no plane: the picture is the one that the
// frontend put on the screen. The processes stay stopped; the guest stops
// after the check.
//
// With --screen it first waits for the first frame of the frontend, then for
// the screen. A first boot shows the confirm prompt of the guard before the
// screen.
//
// Output lines:
//   frame 1 uptime <SECONDS>            when the frontend drew its first frame
//   screen <ID> uptime <SECONDS>        when the frontend reported the screen
//   mode <W>x<H> clock <KHZ> ...        the mode on the CRTC
//   NEPTRON-CAPTURE-BEGIN <W> <H>
//   <base64 of the PPM file, 76 characters a line>
//   NEPTRON-CAPTURE-END
// The exit code is 0 when the picture was printed, else 1.

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <limits.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

#include <drm_fourcc.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

#define SOCKET_PATH "/run/neptron/control.sock"
#define DRM_PATH "/dev/dri/card0"
// The control protocol cuts a longer wait to 60 s.
#define WAIT_SLICE_MS 60000
#define RETRY_MS 200
#define FLIP_SETTLE_MS 200
#define FENCE_WAIT_MS 2000
#define BASE64_LINE_CHARS 76

static int failed(const char *what, int err)
{
	printf("FAIL: %s: %s\n", what, strerror(err));
	return 1;
}

static uint64_t now_ms(void)
{
	struct timespec ts;

	clock_gettime(CLOCK_MONOTONIC, &ts);
	return (uint64_t)ts.tv_sec * 1000 + (uint64_t)ts.tv_nsec / 1000000;
}

static void sleep_ms(unsigned ms)
{
	struct timespec ts = { .tv_sec = ms / 1000, .tv_nsec = (long)(ms % 1000) * 1000000 };

	while (nanosleep(&ts, &ts) && errno == EINTR)
		;
}

// One request and one response line on the control socket. condition is
// the JSON of the wait condition. Returns 1 when the frontend answered the
// wait, 0 when it should be asked again (no socket yet, or the wait timed
// out), -1 for any other answer.
static int ask_wait(const char *condition, unsigned timeout_ms)
{
	struct sockaddr_un addr = { .sun_family = AF_UNIX };
	char line[4096];
	size_t used = 0;
	int fd, n;

	fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
	if (fd < 0)
		return 0;
	snprintf(addr.sun_path, sizeof(addr.sun_path), "%s", SOCKET_PATH);
	if (connect(fd, (struct sockaddr *)&addr, sizeof(addr))) {
		close(fd);
		return 0;
	}
	n = snprintf(line, sizeof(line),
		     "{\"v\":1,\"id\":1,\"cmd\":\"wait\",\"args\":{\"until\":%s,"
		     "\"timeout_ms\":%u}}\n", condition, timeout_ms);
	if (n < 0 || (size_t)n >= sizeof(line) || write(fd, line, (size_t)n) != n) {
		close(fd);
		return 0;
	}
	while (used < sizeof(line) - 1) {
		ssize_t got = read(fd, line + used, sizeof(line) - 1 - used);

		if (got <= 0)
			break;
		used += (size_t)got;
		if (memchr(line, '\n', used))
			break;
	}
	close(fd);
	line[used] = '\0';
	if (strstr(line, "\"reply\":\"wait\""))
		return 1;
	if (used == 0 || strstr(line, "\"code\":\"timeout\"") || strstr(line, "\"code\":\"unavailable\""))
		return 0;
	printf("FAIL: the frontend answered the wait with: %s", line);
	return -1;
}

static int wait_for(const char *condition, uint64_t deadline)
{
	for (;;) {
		uint64_t now = now_ms();
		unsigned slice;
		int answer;

		if (now >= deadline) {
			printf("FAIL: the frontend did not reach %s in time\n", condition);
			return -1;
		}
		slice = deadline - now > WAIT_SLICE_MS ? WAIT_SLICE_MS : (unsigned)(deadline - now);
		answer = ask_wait(condition, slice);
		if (answer)
			return answer > 0 ? 0 : -1;
		sleep_ms(RETRY_MS);
	}
}

static void print_uptime(const char *what)
{
	char text[64] = "";
	FILE *f = fopen("/proc/uptime", "r");

	if (f) {
		if (!fgets(text, sizeof(text), f))
			text[0] = '\0';
		fclose(f);
	}
	text[strcspn(text, " \n")] = '\0';
	// The frontend logs to the same console and can leave a line open.
	printf("\n%s uptime %s\n", what, text);
	fflush(stdout);
}

static int read_text(const char *path, char *out, size_t size)
{
	FILE *f = fopen(path, "r");
	int ok;

	if (!f)
		return -1;
	ok = fgets(out, (int)size, f) != NULL;
	fclose(f);
	out[strcspn(out, "\n")] = '\0';
	return ok ? 0 : -1;
}

// The process with this command name and, when parent is not 0, this
// parent. Returns 0 when there is none.
static pid_t find_process(const char *name, pid_t parent)
{
	DIR *dir = opendir("/proc");
	struct dirent *e;
	pid_t found = 0;

	while (dir && !found && (e = readdir(dir))) {
		char path[300], text[512];
		pid_t pid = (pid_t)strtol(e->d_name, NULL, 10);
		char *close_paren;
		long ppid;

		if (pid <= 0)
			continue;
		snprintf(path, sizeof(path), "/proc/%d/comm", pid);
		if (read_text(path, text, sizeof(text)) || strcmp(text, name))
			continue;
		if (!parent) {
			found = pid;
			continue;
		}
		snprintf(path, sizeof(path), "/proc/%d/stat", pid);
		if (read_text(path, text, sizeof(text)))
			continue;
		close_paren = strrchr(text, ')');
		if (close_paren && sscanf(close_paren + 1, " %*c %ld", &ppid) == 1 && ppid == parent)
			found = pid;
	}
	if (dir)
		closedir(dir);
	return found;
}

// The number of the descriptor of pid that is open on the DRM device, or -1.
static int find_drm_fd(pid_t pid)
{
	char path[64];
	DIR *dir;
	struct dirent *e;
	int found = -1;

	snprintf(path, sizeof(path), "/proc/%d/fd", pid);
	dir = opendir(path);
	while (dir && found < 0 && (e = readdir(dir))) {
		char link[400], target[PATH_MAX];
		ssize_t n;

		if (e->d_name[0] == '.')
			continue;
		snprintf(link, sizeof(link), "%s/%s", path, e->d_name);
		n = readlink(link, target, sizeof(target) - 1);
		if (n <= 0)
			continue;
		target[n] = '\0';
		if (strcmp(target, DRM_PATH) == 0)
			found = (int)strtol(e->d_name, NULL, 10);
	}
	if (dir)
		closedir(dir);
	return found;
}

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
	return id;
}

static void print_base64(const uint8_t *data, size_t size)
{
	static const char digits[] =
		"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
	char line[BASE64_LINE_CHARS + 2];
	size_t used = 0;

	for (size_t i = 0; i < size; i += 3) {
		uint32_t v = (uint32_t)data[i] << 16;
		size_t left = size - i;

		if (left > 1)
			v |= (uint32_t)data[i + 1] << 8;
		if (left > 2)
			v |= data[i + 2];
		line[used++] = digits[(v >> 18) & 63];
		line[used++] = digits[(v >> 12) & 63];
		line[used++] = left > 1 ? digits[(v >> 6) & 63] : '=';
		line[used++] = left > 2 ? digits[v & 63] : '=';
		if (used == BASE64_LINE_CHARS) {
			line[used++] = '\n';
			fwrite(line, 1, used, stdout);
			used = 0;
		}
	}
	if (used) {
		line[used++] = '\n';
		fwrite(line, 1, used, stdout);
	}
}

struct dumb {
	uint32_t handle, pitch, fb_id;
	uint64_t size;
	uint8_t *map;
};

static int make_dumb(int fd, uint32_t w, uint32_t h, struct dumb *b)
{
	struct drm_mode_create_dumb create = { .width = w, .height = h, .bpp = 32 };
	struct drm_mode_map_dumb map = { 0 };
	uint32_t handles[4] = { 0 }, pitches[4] = { 0 }, offsets[4] = { 0 };

	if (drmIoctl(fd, DRM_IOCTL_MODE_CREATE_DUMB, &create))
		return -errno;
	b->handle = create.handle;
	b->pitch = create.pitch;
	b->size = create.size;
	handles[0] = b->handle;
	pitches[0] = b->pitch;
	if (drmModeAddFB2(fd, w, h, DRM_FORMAT_XRGB8888, handles, pitches, offsets, &b->fb_id, 0))
		return -errno;
	map.handle = b->handle;
	if (drmIoctl(fd, DRM_IOCTL_MODE_MAP_DUMB, &map))
		return -errno;
	b->map = mmap(NULL, b->size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, (off_t)map.offset);
	if (b->map == MAP_FAILED) {
		b->map = NULL;
		return -errno;
	}
	memset(b->map, 0, b->size);
	return 0;
}

// Takes the writeback picture on the master descriptor fd and prints it.
static int capture(int fd)
{
	drmModeRes *res;
	drmModeCrtc *crtc = NULL;
	uint32_t wb_conn = 0, crtc_id = 0, w, h;
	struct dumb wb = { 0 };
	drmModeAtomicReq *req;
	int32_t fence = -1;
	struct pollfd pfd;
	uint8_t *ppm;
	size_t header, size;
	int ret;

	if (drmSetClientCap(fd, DRM_CLIENT_CAP_UNIVERSAL_PLANES, 1) ||
	    drmSetClientCap(fd, DRM_CLIENT_CAP_ATOMIC, 1) ||
	    drmSetClientCap(fd, DRM_CLIENT_CAP_WRITEBACK_CONNECTORS, 1))
		return failed("client caps", errno);
	res = drmModeGetResources(fd);
	if (!res)
		return failed("get resources", errno);
	for (int i = 0; i < res->count_crtcs && !crtc_id; i++) {
		drmModeCrtc *c = drmModeGetCrtc(fd, res->crtcs[i]);

		if (c && c->mode_valid) {
			crtc = c;
			crtc_id = c->crtc_id;
		} else {
			drmModeFreeCrtc(c);
		}
	}
	for (int i = 0; i < res->count_connectors && !wb_conn; i++) {
		drmModeConnector *c = drmModeGetConnector(fd, res->connectors[i]);

		if (c && c->connector_type == DRM_MODE_CONNECTOR_WRITEBACK)
			wb_conn = c->connector_id;
		drmModeFreeConnector(c);
	}
	drmModeFreeResources(res);
	if (!crtc_id) {
		printf("FAIL: no CRTC has a mode\n");
		return 1;
	}
	if (!wb_conn) {
		printf("FAIL: no writeback connector\n");
		drmModeFreeCrtc(crtc);
		return 1;
	}
	w = crtc->mode.hdisplay;
	h = crtc->mode.vdisplay;
	printf("mode %ux%u clock %u kHz h %u %u %u %u v %u %u %u %u flags 0x%x\n", w, h,
	       crtc->mode.clock, crtc->mode.hdisplay, crtc->mode.hsync_start,
	       crtc->mode.hsync_end, crtc->mode.htotal, crtc->mode.vdisplay,
	       crtc->mode.vsync_start, crtc->mode.vsync_end, crtc->mode.vtotal,
	       crtc->mode.flags);
	drmModeFreeCrtc(crtc);

	ret = make_dumb(fd, w, h, &wb);
	if (ret)
		return failed("writeback buffer", -ret);
	req = drmModeAtomicAlloc();
	drmModeAtomicAddProperty(req, wb_conn,
		prop_id(fd, wb_conn, DRM_MODE_OBJECT_CONNECTOR, "CRTC_ID"), crtc_id);
	drmModeAtomicAddProperty(req, wb_conn,
		prop_id(fd, wb_conn, DRM_MODE_OBJECT_CONNECTOR, "WRITEBACK_FB_ID"), wb.fb_id);
	drmModeAtomicAddProperty(req, wb_conn,
		prop_id(fd, wb_conn, DRM_MODE_OBJECT_CONNECTOR, "WRITEBACK_OUT_FENCE_PTR"),
		(uint64_t)(uintptr_t)&fence);
	// The routing of the writeback connector is a modeset for the atomic
	// helpers. The CRTC keeps its mode and the plane keeps its framebuffer.
	ret = drmModeAtomicCommit(fd, req, DRM_MODE_ATOMIC_TEST_ONLY | DRM_MODE_ATOMIC_ALLOW_MODESET, NULL);
	if (!ret)
		ret = drmModeAtomicCommit(fd, req, DRM_MODE_ATOMIC_ALLOW_MODESET, NULL);
	drmModeAtomicFree(req);
	if (ret)
		return failed("writeback commit", errno);
	pfd = (struct pollfd){ .fd = fence, .events = POLLIN };
	if (fence < 0 || poll(&pfd, 1, FENCE_WAIT_MS) != 1) {
		printf("FAIL: the writeback fence %d did not signal in %d ms\n", fence, FENCE_WAIT_MS);
		return 1;
	}
	close(fence);

	size = (size_t)w * h * 3 + 32;
	ppm = malloc(size);
	if (!ppm)
		return failed("allocate the picture", ENOMEM);
	header = (size_t)snprintf((char *)ppm, 32, "P6\n%u %u\n255\n", w, h);
	size = header;
	for (uint32_t y = 0; y < h; y++) {
		const uint8_t *row = wb.map + (size_t)y * wb.pitch;

		for (uint32_t x = 0; x < w; x++) {
			// XRGB8888 is little endian: B, G, R, X in memory.
			ppm[size++] = row[x * 4 + 2];
			ppm[size++] = row[x * 4 + 1];
			ppm[size++] = row[x * 4];
		}
	}
	printf("\nNEPTRON-CAPTURE-BEGIN %u %u\n", w, h);
	print_base64(ppm, size);
	printf("NEPTRON-CAPTURE-END\n");
	fflush(stdout);
	free(ppm);
	return 0;
}

int main(int argc, char **argv)
{
	const char *screen = NULL;
	unsigned timeout_ms = 300000, settle_ms = 2000;
	pid_t launcher, frontend;
	int pidfd, drm_fd, fd;

	for (int i = 1; i < argc; i++) {
		if (strcmp(argv[i], "--screen") == 0 && i + 1 < argc)
			screen = argv[++i];
		else if (strcmp(argv[i], "--timeout-ms") == 0 && i + 1 < argc)
			timeout_ms = (unsigned)strtoul(argv[++i], NULL, 10);
		else if (strcmp(argv[i], "--settle-ms") == 0 && i + 1 < argc)
			settle_ms = (unsigned)strtoul(argv[++i], NULL, 10);
		else {
			printf("usage: neptron-capture [--screen ID] [--timeout-ms N] [--settle-ms N]\n");
			return 2;
		}
	}
	setvbuf(stdout, NULL, _IOLBF, 0);

	if (screen) {
		uint64_t deadline = now_ms() + timeout_ms;
		char condition[160], what[160];

		if (wait_for("{\"frame\":1}", deadline))
			return 1;
		print_uptime("frame 1");
		snprintf(condition, sizeof(condition), "{\"screen\":\"%s\"}", screen);
		if (wait_for(condition, deadline))
			return 1;
		snprintf(what, sizeof(what), "screen %s", screen);
		print_uptime(what);
	}
	sleep_ms(settle_ms);

	launcher = find_process("neptron-launch", 0);
	if (!launcher) {
		printf("FAIL: no neptron-launch process\n");
		return 1;
	}
	frontend = find_process("neptron", launcher);
	if (frontend && kill(frontend, SIGSTOP))
		return failed("stop the frontend", errno);
	if (kill(launcher, SIGSTOP))
		return failed("stop the launcher", errno);
	// A flip that the frontend queued before it stopped completes at the
	// next vblank.
	sleep_ms(FLIP_SETTLE_MS);

	drm_fd = find_drm_fd(launcher);
	if (drm_fd < 0) {
		printf("FAIL: the launcher holds no descriptor on %s\n", DRM_PATH);
		return 1;
	}
	pidfd = (int)syscall(SYS_pidfd_open, launcher, 0);
	if (pidfd < 0)
		return failed("pidfd_open", errno);
	fd = (int)syscall(SYS_pidfd_getfd, pidfd, drm_fd, 0);
	if (fd < 0)
		return failed("pidfd_getfd", errno);
	close(pidfd);
	return capture(fd);
}
