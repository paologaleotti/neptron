# Network panel: the address, the host name and the share

`Settings > Network` shows the local address of the machine, its host name, the state of the network share, and how to reach the share from another computer. It is the place where a user reads the address: nothing about the address goes to the share (`docs/spec/network-share.md`, section 3).

The panel only shows. It changes no setting, no file and no network state.

## Facts that drive the design

- `/run/neptron/network.toml` exists in every image. The script `/usr/libexec/neptron/net-address` writes it at boot and at each DHCP event. `docs/spec/network-share.md`, section 3, has the format and the rules of the writer: `version = 1`, `host_name`, and one `[[address]]` table (`port`, `ipv4`) for each wired port with a lease, in the order of the port names; no table means no port has an address. The format is append-only. The file is replaced by a rename, so a reader never sees a part of it, and it changes only when its content changes. `/run` is a RAM folder: no read touches a disk.
- The mDNS name is `<host_name>.local`: `mdnsd` announces the system host name.
- The share is `share`, the folder `/share`, read-write for guests and for the published account `neptron` with the password `neptron` (only for Windows 11 24H2, which refuses guest sessions). Windows reaches it at `\\<address>\share`, macOS and Linux at `smb://<address>/share`.
- **The share has an off switch, but no setting.** The file `/state/network-share-off` turns the share off when it is there at the start of the share script (boot, or `/etc/init.d/S45neptron-share restart`). No schema entry and no settings row exist; OP-092 lists the row as a later step (see "Open questions").
- The frame thread does no file I/O. Unlike `storage.toml`, which the start reads once, the address can change at any time after the boot, so the network file needs a reader that runs while the panel is open.
- The menu canvas is 320x240. The font is 6x12, one character is 6 px. A settings row of the generic page screen is 284 px wide, and 276 px (46 characters) inside its padding; with a scroll bar 272 px (45 characters). The box holds 14 rows of 12 px with the default skin.

## The rows

The panel lists rows in this order. Every row is an information row (see "The information row"). A row shows its label at the left and its text at the right.

| Row | Label | Text | When |
| --- | --- | --- | --- |
| Address | `Address <port>`, for example `Address eth0` | the IPv4 address, for example `192.168.0.112` | one row for each `[[address]]` table, in the order of the file |
| Address | `Address` | `None (no cable or no network)` | the file is valid and has no `[[address]]` table |
| Address | `Address` | `Not known` | the file is missing, cannot be read, does not parse, or has another version |
| Address | `Address` | `Reading...` | the reader thread runs and no read has ended since the start of the run |
| Host name | `Host name` | `host_name` of the file | the file is valid and its host name is valid |
| Network name | `Network name` | `<host_name>.local` | the file is valid, its host name is valid, and the share is on |
| Share | `Share` | `On`, or `Off` when `/state/network-share-off` is there | the file is valid |
| Windows | `Windows` | `\\<target>\share` | the file is valid, the share is on, and a target exists |
| macOS and Linux | `macOS, Linux` | `smb://<target>/share` | the file is valid, the share is on, and a target exists |
| Share user | `Share user` | `neptron` | the file is valid and the share is on |
| Share password | `Share password` | `neptron` | the file is valid and the share is on |

`<target>` is the address of the first `[[address]]` table. With no address it is `<host_name>.local`, which works once a lease comes, through mDNS. With no address and no valid host name there is no target, and the two share paths go. An address works on every client; mDNS needs a client that resolves `.local` names.

**The network name is announced only with the share on.** `S45neptron-share` starts `mdnsd` together with the SMB server, and does not start either with the switch file there. So with the share off nothing answers for `<host_name>.local`, and the panel does not show the `Network name` row.

**A host name that is empty or not valid** (see "The reader") gives no `Host name` row and no `Network name` row. The address rows stay, and the share paths use the first address. A host name with dots is shown as it is, and its network name is `<host_name>.local`; what `mdnsd` announces for such a name is a hardware check.

With `Not known` and `Reading...` the panel has the address row only. Nothing else in the panel is sure then: the host name comes from the same file.

With the share off, the panel does not show how to reach the share. `Off` follows the switch file as it is now. The share script reads that file only at its start, so a switch file made without a restart of the share shows `Off` while the share still runs. Only the development image can make the file (`docs/spec/network-share.md`, section 4).

Several ports give several address rows. Each label holds its port name, so the labels stay different. The UI crawler stops on a label it saw before, and a test asks for different labels.

### Examples

One port with a lease, share on:

```
Address eth0                       192.168.0.112
Host name                                neptron
Network name                       neptron.local
Share                                         On
Windows                    \\192.168.0.112\share
macOS, Linux           smb://192.168.0.112/share
Share user                               neptron
Share password                           neptron
```

No lease (no cable, or no DHCP answer):

```
Address            None (no cable or no network)
Host name                                neptron
Network name                       neptron.local
Share                                         On
Windows                    \\neptron.local\share
macOS, Linux           smb://neptron.local/share
Share user                               neptron
Share password                           neptron
```

## The information row

The snapshot value kind is `SettingValue::Text(String)`, and the tree row kind `RowKind::Info`.

- An information row shows a text that the player reads. It takes no change: accept, left, right and Menu do nothing.
- It takes the focus, as every row of a list does, so the player can scroll a long panel. With the focus it has the look of a disabled row with the focus: a solid bar in the `border` colour with the text in `text`. That look says "nothing to do here", and a scanline cannot hide it.
- Without the focus, its label and its text are in `text`, at full brightness. A disabled row is dim; an information row is not, because its text is the content of the panel.
- The label is drawn at the left in its own width. The text is drawn right-aligned in the rest of the row, after one character of space, in the arrow column too: an information row has no arrow. So the label and the text together hold 45 characters with one space between them (44 with a scroll bar).
- A text that does not fit is cut with `...`, and the tree row is marked as clipped, with the whole text as its value. The crawler fails on a clipped row.
- The footer has no left hint on an information row, as on a disabled row.
- The tree row has the kind `Info`, the label, the whole text as its value, and no source.
- `ui tree` of `neptronctl` shows the label and the value, as for every row. The control protocol does not carry the kind (OP-093).

`RowKind::Info` is not `Disabled`: a disabled row is a control that cannot act now, and it has a reason. An information row is not a control.

## Text limits

In characters of the font. The row holds 45 characters: the label, one space, the text.

| Row | Longest label | Longest text | Total |
| --- | --- | --- | --- |
| Address of a port | `Address enx001122334455` (23: an interface name has at most 15 characters) | `255.255.255.255` (15) | 39 |
| No address | `Address` (7) | `None (no cable or no network)` (29) | 37 |
| Host name | `Host name` (9) | 35 | 45 |
| Network name | `Network name` (12) | 32, so a host name of at most 26 | 45 |
| Windows | `Windows` (7) | `\\255.255.255.255\share` (22) | 30 |
| macOS and Linux | `macOS, Linux` (12) | `smb://255.255.255.255/share` (27) | 40 |
| macOS and Linux, no address | `macOS, Linux` (12) | `smb://<host_name>.local/share`, so a host name of at most 14 | 45 |
| Share password | `Share password` (14) | `neptron` (7) | 22 |

Every image sets the host name `neptron` (7 characters): every row fits. A longer host name is not made by NEPTRON (changing the host name is out of scope). With one, a row that does not fit is cut with `...`, and the tree marks it as clipped.

## The reader

**The parse is pure, in `neptron-platform`.** The crate holds the detection of the board and its state from files of the system. It depends only on `neptron-types` and has `serde` and `toml` already. `neptron-ui` must depend on no other NEPTRON crate, and the app holds only wiring.

- `neptron_platform::parse_network(text: &str) -> Result<NetworkStatus, NetworkError>`.
- `NetworkStatus { host_name: Option<String>, addresses: Vec<PortAddress> }`; `PortAddress { port: String, ipv4: Ipv4Addr }`.
- `NETWORK_FILE_VERSION` is 1.
- **Version first.** The parse reads `version` before any other field. A missing `version` gives `NetworkError::NoVersion`. A version that is not an integer gives `NetworkError::VersionType`. Another integer gives `NetworkError::Version { found, supported }`, for example `the network file has version 2; this build reads version 1`. A later version can add fields. So a reader of version 1 refuses version 2 as a whole and does not guess what its fields mean.
- **Unknown fields are ignored**, at the top and in each `[[address]]` table. The format is append-only, so a later version 1 writer may add a field.
- **Bounds.** A text over `NETWORK_FILE_MAX_BYTES` (64 KiB) gives `NetworkError::TooLarge`; the app reads at most that many bytes and one more, so a damaged file never fills the memory. More than `ADDRESSES_MAX` (16) address tables give `NetworkError::TooManyAddresses`. Two tables for one port give `NetworkError::DuplicatePort`. Each of these means a damaged file: the panel says `Not known`.
- **The port and the address.** `port`: 1 to 15 characters, letters, digits, `_` and `-`, starting with `eth` or `en`. `ipv4`: a dotted IPv4 address as `std::net::Ipv4Addr` parses it (four groups, each 0 to 255, no leading zero). A value that fails gives an error with the value: `NetworkError::Port`, `NetworkError::Address`. The file comes from a script of the image, so a bad value means a damaged file: the panel says `Not known`.
- **The host name.** Labels of letters, digits and `-`, each 1 to 63 characters, joined by dots, at most 253 characters in all (`HOST_LABEL_MAX_CHARS`, `HOST_NAME_MAX_CHARS`). An empty or invalid host name is not an error: `host_name` is `None`, and the addresses stay usable.
- **Where the reader differs from the writer.** `net-address` keeps letters, digits, `.` and `-` of the host name with no length bound, and can write an empty name: the reader gives no host name then (see "The rows"). The script keeps a port name of any length; the reader refuses one over 15 characters, which the kernel never makes. The script keeps a group of an address with a leading zero, such as `10.0.0.01`; `Ipv4Addr` refuses it, so the whole file is damaged and the panel says `Not known`. A DHCP server does not give such an address, and `udhcpc` prints an address with no leading zero.
- A file that is not TOML, or has a field of the wrong type, gives `NetworkError::Parse` with the message of the parser.

**The I/O is in the app**, in `apps/neptron/src/network.rs`: it reads `<run>/network.toml` and the presence of `<state>/network-share-off`, and gives a `NetworkInfo`: the status and the share switch, or `Unknown` with the reason for the log.

## Which thread, and how often

- A thread of the app, `network-watch`, does every read. The frame thread never reads the file.
- The frame thread asks for a read over a bounded channel of one request (`sync_channel(1)`). A full channel means that a request waits already: the frame thread drops the new one and never waits.
- It asks once at the start of the run, so the panel has its rows when the player opens it. It asks again in the first frame after the panel comes on screen, and then every `NETWORK_REFRESH_NS` (1 s, on the clock of the frame loop) while it stays on screen. The screen on show of the last frame (`NETWORK_SCREEN`, `network`) decides, not the page in the snapshot: the snapshot keeps the page after the panel closes. With the panel closed it asks nothing and builds no page; a confirm question or a notice over the panel counts as closed.
- When the panel comes on screen again, the page is built from the last status at once, and the read of that frame updates it.
- The thread sends a result only when it differs from the last result it sent, over a bounded channel of one result. When that channel is full, the thread drops the result and forgets it, so the next read sends again.
- The frame thread takes a result with `try_recv` at each frame, keeps it, logs a changed reason of `Unknown` (`network: ...`), and builds the Network page again while the panel is on screen.
- So an address that a lease gives after the boot shows within about 1 s while the panel is open, with no need to leave the panel. A lost lease or an unplugged port goes in the same way.
- The shell owns the thread. At the end of the run the shell drops the request channel; the thread ends its wait and the shell joins it. A read is one small file in RAM, so the join never waits long.
- A thread that does not start gives a log line, and the panel shows `Not known` for the run. The start of a run never fails for it.

## Missing, damaged or other version

The panel shows the one row `Address` `Not known`. Nothing stops: no notice, no error for the player. The log of the run gets one line with the reason when the reason changes, for example `network: cannot read /run/neptron/network.toml: No such file or directory`. A file that becomes valid later shows at the next read.

## A development host

A run on a desktop or a bench has no `network.toml` in its run folder. The panel shows `Address` `Not known`, as for a missing file. The panel is listed on every rig, so the crawler finds it everywhere. The desktop does not read the addresses of the computer it runs on: the panel shows the state of a NEPTRON image, not of a development computer.

## The Settings page and the ids

- `NETWORK_PAGE` is `PageId(RESERVED_ID_MIN + 9)`.
- `SETTINGS_PANELS` holds `Network` between `Library` and `System`: `Display`, `Audio`, `Input`, `Library`, `Network`, `System`. System stays last, as the panel that ends the run.
- The screen id is `network`. The generic page screen shows the panel; no new screen code.
- In a game the panel is listed and shows the same rows: it only shows.

## Out of scope

- Wi-Fi.
- Static addresses, and any change of the network configuration.
- A change of the host name.
- IPv6: the images have none.
- A settings row for the switch of the share (see "Open questions").
- The share path as a QR code, or any picture.

## Open questions

- **A settings row for the share switch.** Default: no row; the panel shows the state of the switch file, `On` or `Off`, as text. The share has a switch (the file), but no schema entry; OP-092 lists the row as a later step. Where: `crates/neptron-settings` (a schema entry), `apps/neptron/src/pages.rs`, the share script. How to change: a schema entry `network.share` (a switch, global), whose change writes or removes `/state/network-share-off` with the write rule of user data and runs `/etc/init.d/S45neptron-share restart`; it then needs a privileged hook in the launcher, since the frontend must not run init scripts.
- **The account rows.** Default: the panel shows `Share user` and `Share password` with the published account `neptron`. The account is public by design (`docs/spec/network-share.md`, section 1) and only helps Windows 11 24H2. The values are constants of the frontend and must follow `os/board/common/rootfs-overlay/etc/ksmbd/ksmbdpwd.db`. How to change: put the account name in `network.toml` (a new field of version 1) and read it from there.

## Scenes and goldens

- `screens/settings`: the Settings page with the `Network` row.
- `screens/network`: one port with an address, share on, focus on the first row.
- `screens/network-two-ports`: two ports with addresses.
- `screens/network-no-address`: a valid file with no address.
- `screens/network-unknown`: a missing or damaged file, the one row `Not known`.
- `screens/network-share-off`: one address, share off: no network name and no share path.

## Tests

- `neptron-platform`: the parse of one port, two ports, no address, unknown fields at the top and in a table, a missing version, a version of the wrong type, version 2, a file that is not TOML, a missing `host_name`, a file over 64 KiB, 17 address tables, two tables for one port, host names with dots, an empty or invalid host name (no name, the addresses kept), a bad port, a bad address (six values: too few groups, a group over 255, a leading zero, letters, an IPv6 address, empty).
- `neptron-ui`: the information row: its kind, no change for any action, no hint, the tree row (kind `Info`, whole value, no source), the right-aligned text in the arrow column, a cut text marked as clipped.
- `apps/neptron` unit tests: the rows of each state (`pages::network_page`), the share target (first address, else `.local`, else none), the share off (no network name), no valid host name, a host name with dots; the reader on a folder with a valid file, no file, a damaged file, a version 2 file, a file over 64 KiB, and the switch file. Each test removes its folders.
- `apps/neptron` tier 2 (`tests/app/network.rs`): the panel with a file of one port; a file written after the panel opened (a late lease) shows without leaving the panel, and a removed address goes; a damaged file shows `Not known` and no notice; a run with no file shows `Not known`; the share switch hides the network name and the share paths; an empty host name keeps the address and its share path; with the panel closed for 2 s of the clock no read is asked and no page is built, and a new open reads at once and shows a file written meanwhile; in a game the panel shows the same rows. Each test removes its folders. The crawler reaches the screen `network`.
- Hardware checks in `docs/hardware-checklist.md`, "Network panel (M03)".
