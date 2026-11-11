# Fixtures

`sysfs/` holds trees in the layout of `/sys/class/drm`, as plain
directories and files. The real `/sys` uses symbolic links; the reader
follows them, so plain directories test the same code.

Each tree is made by hand from the kernel sysfs layout. No tree is a copy
of a real machine:

- `i3_6100t_vga`: Intel HD 530 (PCI `8086:1912`) with a VGA connector, a
  CRT on it, and two disconnected digital connectors. The connector names
  on the real machine depend on the board and are checked on hardware.
- `pi3_composite`: a Raspberry Pi 3 with `v3d` as `card0` and `vc4` as
  `card1`, composite status `unknown`.
- `qemu_bochs`: the QEMU standard VGA (`1234:1111`) with one virtual
  connector, and two cards with no uevent, to test the sort by number.
- `no_drm`, `drm_is_file`, `bad_uevent`, `bad_status`, `missing_status`:
  error paths.

`sysfs/i3_6100t_vga/class/drm/card0-VGA-1/edid` is the blob that
`pc_crt_30_71` in `integration/blobs.rs` builds; a test checks that they
are equal.
