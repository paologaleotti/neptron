# 0014: FinalBurn Neo is the arcade core, under its non-commercial licence

- Status: accepted
- Date: 2026-11-16
- Task: M06

## Context

NEPTRON 0.1 gets arcade games. `docs/spec/arcade.md` compares the libretro arcade cores. FinalBurn Neo (FBNeo) covers Neo Geo, CPS-1, CPS-2, CPS-3, Cave 68000 and most 2D boards, is maintained, and Batocera builds it with Buildroot for the Pi 3 (aarch64) and for x86_64. MAME 2003-Plus is lighter but uses another set version (MAME 0.78). Current MAME is GPL but too heavy for the Pi 3. Geolith (BSD-3-Clause) runs only Neo Geo, from `.neo` files.

The FBNeo licence (`src/license.txt`, read at master on 2026-11-16) has five rules for the FBNeo original material. In short:

1. Nobody may sell, lease or rent FBNeo, or "otherwise seek to gain monetary profit" from it.
2. Changes to the source code must be made public.
3. The full text of the licence goes with it, verbatim.
4. FBNeo may not be distributed with ROM images unless the distributor has the legal right to distribute them.
5. Nobody may ask for donations to support work on any project that uses the FBNeo source code.

The licence says that FBNeo is also subject to the MAME licence, because of the MAME code in it; that licence forbids sale and commercial use. The licence text lists third-party libraries with their own licences, among them the LZMA SDK (7z), libspng (BSD-2-Clause, its text is in `license.txt`), zlib and Xbyak (the x86 and x64 JIT assembler). M07b reads the licence at the pinned commit again and records any difference.

Snes9x and Genesis Plus GX are non-commercial already, and the Definition accepts non-commercial cores, FBNeo by name. Rules 2 and 5 are new limits. The README of the libretro port (`src/burner/libretro/README.md`) says that it is unclear whether the libretro Patreon is a commercial activity, and that the members of the FBNeo team who maintain the port receive no donations.

## Decision

1. FBNeo, the full build, is the default arcade core on every board: the PC image and the Raspberry Pi image. It runs the folders `arcade` (aliases `fbneo`, `fba`) and `neogeo`.
2. The project takes no donations, no sponsorship and no payment of any kind, and plans none. On this basis the owner accepts the FBNeo licence.
3. **A later plan to take donations, sponsorship or any payment for NEPTRON must remove FBNeo first**: its package, its data files, its index and its lines in the licence files, from every image that the plan covers. The removal is merged before the plan starts. Any task that adds a way to give money to the project (a link, a button, a page) checks this ADR first.
4. Each image ships `license.txt` of FBNeo verbatim, in the licence folder of Buildroot and with a line in `os/board/*/LICENCES.txt`. The About screen names the licence, as for every core. M07b also lists, per architecture, the third-party parts that the built `.so` contains, and ships the licence text of each: libspng (BSD-2-Clause), zlib, the LZMA SDK when the 7z support is built in (`INCLUDE_7Z_SUPPORT`, on by default in the makefile), and Xbyak (BSD-3-Clause) on x86_64 when the build sets `USE_X64_DRC=1`. M07b reads each licence file in the source tree of the pin; a part that is not in the built `.so` gets no line.
5. Every NEPTRON patch to FBNeo is in source form in the `legal-info` output of the image and in the repository (`os/package/libretro-fbneo`).
6. The arcade index that NEPTRON makes from the FBNeo DAT ships in the image only, under the FBNeo licence, with its own line in `LICENCES.txt`.
7. NEPTRON's own rule is stricter than FBNeo's rule 4: the repository and the images hold no ROM and no BIOS file at all, and the repository holds no excerpt of the DAT. Test fixtures are synthetic: invented set names, sizes and CRCs. Test ROMs that are free to download stay outside the repository (OP-053).
8. `data/cores/fbneo.toml` names the licence `LicenseRef-FBNeo`.
9. MAME 2003-Plus is the planned second core, after the MVP, for a `mame` folder. Its own licence (the MAME 0.78 licence, non-commercial) needs no new rule, but its task records it in a new ADR.

## Consequences

- NEPTRON can show Neo Geo, CPS and Cave games on both images with one core and one set version.
- The project can never ask for money while FBNeo is in it. This is now a written rule, not an assumption.
- The image build must keep the licence text, the patches and the index licence line in step with the pinned commit. M07b adds a test that the package hashes its licence file, as the tests in `xtask/src/os/defconfig.rs` do for the other cores.
- The Definition says that the build can produce an image without the non-commercial cores. FBNeo is one more core for that list.

## Alternatives

- MAME 2003-Plus as the first core: lighter on the Pi 3, but a second set version for users to learn, and it turns vertical games in the core, which costs CPU on the Pi. It stays the second core.
- Current MAME (GPL-2.0-or-later): no licence limit, but too slow for the Pi 3 boards (research judgement) and a 116 MB core.
- Geolith (BSD-3-Clause): no licence limit, but Neo Geo only, and every MAME zip needs a conversion to `.neo`.
- FB Alpha 2012: the same licence family, with frozen and old sets, known audio faults, and no Cave in its split cores.
- No arcade in 0.1: the taskboard of 2026-11-15 puts arcade in 0.1.
