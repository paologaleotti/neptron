# Network share: SMB for the games folder

This spec settles the network share of the MVP (OP-079, task C13). The owner decided: an SMB share of the games folder in every image, development and release, on by default, as Batocera does. A user on the same network copies ROMs to `\\NEPTRON\share` (or to the address of the PC) with no login.

Before this work only the development image has a network (DHCP on `eth0` through the Buildroot `ifup` script), and no image has an SMB server.

## Facts that drive the design

Read from the pinned Buildroot (2026.08.1, in the container image `neptron-dev`, read-only):

| Package | Version | Licence | Needs |
| --- | --- | --- | --- |
| `samba4` | 4.24.6 | GPL-3.0-or-later | host Python 3 with SSL, GnuTLS, cmocka, popt, e2fsprogs, libxcrypt, zlib; a `waf` cross build |
| `ksmbd-tools` | 3.5.2 | GPL-2.0-or-later | libglib2 (2.88.3, LGPL-2.1-or-later), libnl (3.12.0, LGPL-2.1-or-later); and the kernel server `ksmbd` (`CONFIG_SMB_SERVER`, GPL-2.0) |
| `mdnsd` (troglobit) | 1.1 | BSD-3-Clause | nothing (about 90 kB) |
| `avahi` | | LGPL-2.1-or-later | D-Bus or its own daemon, expat, libdaemon |

Buildroot has no package for a WS-Discovery responder (`wsdd`, `wsdd2`) and no NetBIOS name server outside Samba (`nmbd`).

Read from `ksmbd-tools` 3.5.2 (`ksmbd.conf.5.in`, `tools/config_parser.c`, `include/tools.h`):

- `server min protocol` defaults to `SMB2_10` and `server max protocol` to `SMB3_11`.
- `map to guest` works only with `bad user`; the other values are disabled in the source.
- `guest account` defaults to `nobody`.
- The tools read `/etc/ksmbd/ksmbd.conf`, `/etc/ksmbd/ksmbdpwd.db` and `/etc/ksmbd/ksmbd.subauth`, and write the lock file `/run/ksmbd.lock` (`RUNSTATEDIR`). A missing user database is accepted ("No user database"). A missing `ksmbd.subauth` is made at the first start, which a read-only root does not allow, so the image holds that file.
- There is no `follow symlinks` or `wide links` option. The kernel server does the path walk.

Read from Linux 6.18 (`fs/smb/server/transport_tcp.c`): with no `interfaces` list, the server listens on port 445 on every interface that is up and on every interface that comes up later, except bridge ports. It binds the IPv6 wildcard with dual stack, so one socket serves IPv4 and IPv6.

Read on the bench PC (i3-6100T, development image, 2026-11-13, read-only): one Ethernet port, `eth0`, an RTL8168h on `r8169`. The kernel logged `Unable to load firmware rtl_nic/rtl8168h-2.fw (-2)`, and the port still got its address from the router by DHCP. The release image holds no firmware file and needs none for this chip. The PC has 8 GB of memory.

## 1. The server

**Choice: the in-kernel server `ksmbd` with `ksmbd-tools`.** `docs/definition/12-technology-choices.md` names it too.

| | ksmbd and ksmbd-tools | Samba 4 |
| --- | --- | --- |
| Size in the root filesystem | about 3 to 5 MiB before EROFS compression: `ksmbd.mountd` and its links, libglib2 (glib, gobject, gio, gmodule), pcre2, libffi, libnl. About 0.3 MiB more kernel | 30 MiB or more: `smbd`, `nmbd` and their many libraries, GnuTLS, popt |
| Cold build | about 10 to 20 min on 4 jobs (libglib2 and its host tools are most of it; host Python and Meson are already built for Mesa), and a kernel rebuild | about 40 to 60 min more, a `waf` cross build |
| Licence | GPL-2.0 (kernel), GPL-2.0-or-later (tools), LGPL-2.1-or-later (glib, libnl) | GPL-3.0-or-later |
| Protocols | SMB 2.1, 3.0, 3.0.2, 3.1.1 only. The 6.18 kernel has no SMB1 code to turn on | SMB1 to 3.1.1; SMB1 must be turned off by hand |
| Where the code runs | in the kernel | user space, as root |

Both serve a guest share to Windows, macOS and Linux file managers in the same way: the client protocol decides what works, not the server. So the smaller one wins. The cost of `ksmbd` is that a flaw in it is a flaw in the kernel (see "Attack surface").

**Guest access from current Windows.** These are facts:

- Since Windows 10 version 1709 and Windows Server 2019, the Enterprise and Education editions refuse an "insecure guest logon" over SMB2 and SMB3 by default (the policy "Enable insecure guest logons", registry value `AllowInsecureGuestAuth`).
- Since Windows 11 version 24H2, the Pro edition refuses insecure guest logons by default too, and the SMB client of every edition requires SMB signing by default.
- A guest session has no session key, so it cannot sign. A client that requires signing cannot use any guest share, whatever the server is.

So no server lets Windows 11 24H2 connect as guest with no change on the Windows side. The user changes two settings on the PC (the policies "Enable insecure guest logons" on and "Digitally sign communications (always)" off, or the PowerShell command `Set-SmbClientConfiguration -EnableInsecureGuestLogons $true -RequireSecuritySignature $false`). Batocera has the same limit. Windows 10 Home and Pro and Windows 11 before 24H2 are expected to connect as guest with no change. What Windows 11 24H2 Home does is not known here: a hardware check.

A way out for Windows 11 24H2 that needs no change on Windows: a fixed account with a published password (for example user `neptron`, password `neptron`). NTLMv2 gives that session a key, so it can sign. It is not more secure than a guest share, because the password is public, but it is a login. The owner decided "no login", so this task does not add it. It is a later step for the owner (see "Later steps").

**macOS and Linux.** Finder ("Connect As: Guest") and the file managers of GNOME (gvfs) and KDE (KIO, smbclient) connect to an SMB2 or SMB3 guest share. Whether a current macOS requires signing on its client by default is not sure here: a hardware check.

**Licence notes.** `board/pc/LICENCES.txt` gets a section for the network share. `make legal-info` lists each package and keeps the GPL and LGPL sources, as for every other package.

## 2. What is shared

- One share, named `share`: `\\NEPTRON\share`, `smb://neptron.local/share`.
- Its path is `/share`, the exFAT partition with `roms/<system>` and `bios`. Never `/state` and never the root.
- Read-write for guests. Every client is mapped to the guest account `nobody` (`map to guest = bad user`), so a client that sends its own user name and password gets the same access as one that sends none.
- The share partition is mounted with `uid=65534,gid=65534` (the user `nobody`), `dmask=0022,fmask=0133`, and `nosuid,nodev,noexec`. exFAT stores no owner, so every file shows as owned by `nobody`, whoever wrote it. The guest owns what it writes and can change and delete it; no other user but root can write there. The frontend runs as root and reads and writes all of it, as before. Nothing on the share can run as a program.
- `store dos attributes = no`: exFAT has no extended attributes.
- A client writes files with the names it chooses. macOS writes `._<name>` files (its resource forks) and `.DS_Store` files on a share that has no extended attributes. The share does not refuse them, because a refusal can stop a Finder copy. The library must not show a file whose name starts with `._` as a game (a check for C14 and the library, see "Later steps").

**`/share` as a RAM folder** (no share partition, or one that the storage script left untouched): the share still serves it, read-write, as the owner's default says. The frontend shows its notice already, and what a client copies there is lost at the next boot. The RAM folder of `/share` is owned by `nobody`, with the same `nosuid,nodev,noexec`, so the guest can write to it. Its size is the tmpfs default, half of the memory: a client can fill it, which takes memory from the frontend; this case is rare and the notice says that the share partition is not in use.

## 3. The network on every image

- **DHCP.** The new init script `S40neptron-network` (both images) starts one `udhcpc` for each wired interface (`eth*`, `en*`) in the background, with `-f` (it stays in the foreground of its own background job, so the script never waits), `-R` (release the lease at stop), `-S` (log to syslog), and the host name `neptron` in the request (option 12). A router that keeps a local DNS (a Fritz!Box, for example) then resolves `neptron`. With no cable, `udhcpc` tries again every 20 s, which costs nothing. The development image drops the Buildroot `ifup` DHCP (`BR2_SYSTEM_DHCP`), which waited at boot, and uses the same script.
- **Host name**: `neptron`, from `BR2_TARGET_GENERIC_HOSTNAME`, as before.
- **Name discovery.** Only mDNS, with `mdnsd`: it answers for `neptron.local` and announces the service `_smb._tcp` on port 445. macOS Finder and the GNOME and KDE file managers list the PC under "Network" from that service. Windows 10 and 11 resolve `neptron.local` over mDNS, so `\\neptron.local\share` works there.
  - No WS-Discovery: the Windows "Network" view lists computers through it, but Buildroot has no responder for it, and `wsdd` needs Python on the target. A later step (`wsdd2` is a small C program, but it would be a new package of this tree).
  - No NetBIOS name service: only Samba has one (`nmbd`), and Windows 10 and 11 no longer list computers through NetBIOS browsing. `\\NEPTRON` resolves through the router's DNS when the router registers DHCP host names; else the user types `\\neptron.local` or the address. A hardware check.
- **No Wi-Fi** in this task. It is a later step (`iwd`, as the definition says).
- **The boot.** Nothing waits for the network. `S40neptron-network` sets the links up and starts `udhcpc` as background jobs. The share script starts `ksmbd.mountd` and `mdnsd` in a background job. The kernel server listens on each interface when it comes up, so a cable plugged in later works too. A PC with no cable boots as fast as before.

## 4. Attack surface

The release image now opens an **open, writable guest share on the local network, on by default**. The owner decided this. Anyone on the same network can read, add, change and delete every file of the share partition: the games, the BIOS files, and anything else there. This is the Batocera habit. It is for a home network only: do not forward port 445 to the internet, and do not plug the PC into a network you do not trust.

What this design does to limit it:

- **Only the share folder.** The one share is `/share`. `/state` (settings, saves, the guard record), the root and the boot partition are not shared.
- **No way out of the folder.** The kernel server resolves every path of a request below the share root with `LOOKUP_BENEATH`, so a path with `..` or an absolute link cannot leave it, and it opens, creates, links and renames with `LOOKUP_NO_SYMLINKS`, so it does not follow a symbolic link. exFAT cannot store a symbolic link at all. ksmbd has no `wide links` or `follow symlinks` option to turn this off; the configuration sets no `root directory`.
- **SMB2 or newer only**: `server min protocol = SMB2_10`. The kernel check fails the build when `CONFIG_SMB_INSECURE_SERVER` (SMB1, gone from current kernels) is on, and `CONFIG_SMB_SERVER_SMBDIRECT` (RDMA) and `CONFIG_SMB_SERVER_KERBEROS5` stay off.
- **Least rights.** A guest works as the user `nobody`: the kernel server switches to that user's file system identity for each request and drops the capabilities that pass file permissions. `nobody` can write the share and `/tmp`, and nothing else. `ksmbd.mountd` runs as root, because it talks to the kernel server over netlink, which needs `CAP_NET_ADMIN`; it handles the logins and the share list, not the files.
- **No other open port on the release image.** It opens TCP 445 (the share) and UDP 5353 (mDNS) only. `udhcpc` is a client and accepts no connection. There is still no SSH, no telnet, no web server; the release post-build still fails when it finds an SSH or telnet server. `ksmbd.mountd` gets no user database, so no account exists on the share.
- **The code in the kernel.** ksmbd had serious remote flaws in the past (for example CVE-2022-47939, a use-after-free before login, in Linux 5.15 to 5.19). A flaw in it is a flaw in the kernel. The image must follow the 6.18 LTS releases. This is the main cost of the small server.
- **Connections**: at most 16 at a time (`max connections = 16`).
- **The switch.** A file on the state partition turns the share off: `/state/network-share-off` (its content does not matter). When it is there at boot, the share script starts neither the server nor `mdnsd`, so port 445 and port 5353 stay closed. The network (DHCP) stays on. Another computer cannot easily write the state partition (it is ext4), so today the file is made on the development image: `touch /state/network-share-off` over SSH, then `/etc/init.d/S45neptron-share restart` or a reboot. A state partition that does not mount (a RAM folder) cannot hold the file, so the share is on in that case.
- **The later settings row** ("Network share: On / Off" in the system settings) writes or removes that file (an empty file, then `fsync`; or an unlink), then runs `/etc/init.d/S45neptron-share restart`, which applies the change at once. This task adds no UI: another task changes the UI now. The hook is the file and the `restart` action of the script.

## 5. Safe shutdown

BusyBox init runs the `shutdown` lines of `inittab` in order: `/usr/libexec/neptron/stop` (the launcher and the frontend), then `/etc/init.d/rcK` (the stop action of each init script, in reverse order), then `umount -a -r`.

The stop action of `S45neptron-share` runs in `rcK`, before the unmount:

1. Stop `mdnsd`, so the PC leaves the network list.
2. `ksmbd.control --shutdown`: the kernel server closes its listeners and every session, and so every open file of a client; `ksmbd.mountd` ends.
3. Wait until no socket on port 445 is left in `/proc/net/tcp` and `/proc/net/tcp6`, at most 5 s (`SHARE_STOP_WAIT_DS`). The kernel closes the sessions in a work queue, after the command returns.
4. `sync`, so the data of a copy that was in progress reaches the stick.

Then `umount -a -r` unmounts `/share`: no file is open on it any more, so the unmount succeeds, and exFAT writes its clean state. A file that a client was copying stays on the share as far as it came; copy it again.

A power cut during a copy is not a safe shutdown: the file is cut, and the exFAT volume can be left marked dirty. The Linux exFAT driver mounts it as it is. A hardware check.

## 6. The library

A file that arrives over the share appears in the game list after a rescan. Task C14 adds the rescan (a menu row, and a scan when the list gets the focus again); until then a restart of the frontend shows it. The share writes into the same `roms/<system>` folders that the storage script makes.

## What the images get

| | PC image, release and development | aarch64 QEMU test image |
| --- | --- | --- |
| Kernel | `board/pc/network.config`: `CONFIG_SMB_SERVER=y` and its options | `board/qemu/network.config`: the same options |
| Packages | `ksmbd-tools`, `mdnsd` | `ksmbd-tools`, `mdnsd` |
| Init | `S40neptron-network`, `S45neptron-share` (common overlay) | the same |
| Kernel check | `check-kernel-config.sh` fails on SMB1, SMB Direct, Kerberos | (no post-build check, as before) |

The test image gets the same server so that the boot check can see port 445 listen. Its `/share` is a RAM folder. The QEMU runs have no network card (`-nic none`) except an SSH run, and the server listens on the loopback interface anyway.

## Checks

- xtask tests read the files of `os/` and pin: the share path, guest only, `bad user`, SMB2 minimum, no `wide links` and no `follow symlinks`, `store dos attributes = no`, the off switch in the share script, the mount options of the share partition, the kernel options and the check script, both images and both variants with the packages, the network script that never waits, and the stop order.
- The boot check (`cargo xtask os boot-check`, tier 3) waits, inside its deadline, until a socket listens on port 445 and `mdnsd` runs, before it waits for the picture.
- Hardware checks in `docs/hardware-checklist.md`: Windows 11, Windows 10, macOS, Linux; a large copy; the cable pulled during a copy; a power cut during a copy; the off switch; a boot with no cable.

## Later steps

- A fixed account with a published password, for Windows 11 24H2 with no change on Windows (owner decision).
- The settings row "Network share" (the hook above).
- WS-Discovery (`wsdd2`), so Windows lists the PC under "Network".
- Wi-Fi with `iwd`.
- The shares of `docs/definition/10-os-layer.md` for `config`, `saves` and `states` on the state partition. This task shares `/share` only, as the owner asked.
- The library ignores `._*` and `.DS_Store` files from macOS (C14 or a library task).
- Show the address of the PC in the UI, so a user can type `\\<address>\share`.
- A USB Ethernet adapter plugged in after the boot gets no DHCP client until the next boot.
