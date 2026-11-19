# 0018: The PC images carry the 15 kHz kernel patch set under GPL-2.0-only

- Status: accepted
- Date: 2026-11-19
- Task: M12

## Context

The PC kernel applies the 15 kHz patch set of D0023R (github.com/D0023R/linux_kernel_15khz), folder `linux-6.18`, nine patches. The set lets the kernel drivers accept modes with a low pixel clock and interlace. The repository had a GPL-3.0 `LICENSE` file, and the patches had no licence line. The kernel is GPL-2.0-only, and GPL-3.0-only code cannot be combined with it. So no image with the set was given to anybody, and CI built and uploaded the PC image without the set (OP-070).

The owner asked the author in issue 22 (github.com/D0023R/linux_kernel_15khz/issues/22, 2026-11-12). The author answered on 2026-11-15 and closed the issue. The answer says that the licence of the kernel patch part of the repository "is now set to GPL-2.0". The answer does not say "only": the "-only" comes from the `README.md` section below.

These upstream commits of 2026-11-15 record the licence in the tree:

| Commit | What it changes |
| --- | --- |
| `c57e6508831a57c21bb9a66f8f9b79c7df4f759e` | `README.md` gets the section "linux_kernel_15khz patch license and attribution": all code in the repository, except the `xorg/` directory, is GPL-2.0-only, the licence of the kernel. |
| `7abc1ed31667070d897d8bd338abaa69f8b0aa69` | A typo in that section. |
| `e106247a22fc82b94fdb08570b60a47ee260ed22` | `LICENSE` becomes the text of the GNU GPL version 2 (SHA-256 `edaef632cbb643e4e7a221717a6c441a4c1a7c918e6e4d56debc3d8739b233f6`). |
| `d85a343379f7f54155f69ef56dc544b666635894` | "Updated for kernel 6.18.63": one line of `README.md`, no patch file. |

Between the old pin `8c96dd9f0b343a6d1ef3c61041783ee106957886` (2026-11-07) and `d85a3433`, only `LICENSE`, `README.md` and files below `xorg/` change. The nine files of `linux-6.18` are the same, byte for byte, with the same SHA-256 values. The patch files still have no SPDX line. Patch 01 adds two new files (`drm_modes_low_dotclock.c` and `.h`) with the MIT-style notice of the DRM core files, which is compatible with GPL-2.0.

### Authors other than the maintainer

The statement of D0023R is that of the maintainer of the set. A maintainer cannot relicense the contributions of other authors: each author keeps the copyright of the lines they wrote, under the licence they gave. These authors contributed to the set or to its history. The evidence is the upstream history (`gh api`, 2026-11-19).

| Author | Contribution | Licence when it was given | Evidence |
| --- | --- | --- | --- |
| amxcs | All of patch 09, `09_linux_15khz_i915_gen9_interlace.patch` | The repository `LICENSE` was GPL-3.0 | Commit `0e52204574d6c408a9f79435da619de79cf48b74` (author amxcs, 2026-08-09), pull request 17, merged 2026-09-20 |
| amxcs, again | The same change in the repository `amxcs/batocera-crt-15khz-intel`, file `patches/0001-drm-i915-no-Y-tiled-scanout-on-gen9.patch` | GPL-2.0: that repository has the GPL version 2 `LICENSE` since commit `0c9cce13be5c463d860695fe914b329fa1332e63` (2026-08-07); the patch is at commit `e2138f887ab045fff36001e0bfd8f2723f5cfeaf` (head on 2026-11-19) | The added and removed code lines of the two patches are the same; only the patch description differs |
| substring | "New version of the 15kHz patch", `linux-5.5/03_linux_15khz.diff`, the base of the current patch 01 | The repository `LICENSE` was GPL-3.0 (the first commit `9d425dc0`, 2019-03-12, added it) | Commit `dcba42cc5bf0d56fed45a3addd4cfae116e7923e` (author substring, 2020-01-24). 42 of its 89 added lines of more than 25 characters are in the current set |
| Substring and Calamity | Named as contributors in `README.md` | Not stated | `README.md` at `d85a3433`, section "CONTRIBUTORS". Calamity has no commit in the repository |
| Ansa89 | github.com/Ansa89/linux-15khz-patch, up to Linux 4.11 (2017), named in the history of `README.md` | GPL-3.0 (`LICENSE` and `COPYING` of that repository) | 6 of its 65 added lines of more than 25 characters are in the current set |
| MaKoTo | A patch of 2012, named in the history of `README.md` | Not stated | `README.md`, section "SOURCES/HISTORY" |

The other contributor of the repository, Redemp, changed files below `xorg/` only. NEPTRON does not use them.

The project judges the statement of the maintainer sufficient for these reasons:

- Each contribution is a change to GPL-2.0-only kernel files. Such a change is a derived work of the kernel, and it can be distributed only under GPL-2.0. A GPL-3.0 file beside it cannot give a permission that the kernel licence does not allow. GPL-2.0 is the only licence under which these authors could lawfully distribute their changes, so the project takes their intent as GPL-2.0. This is a reading of their intent, not a statement by them, and the risk below stays as recorded.
- The statement of the maintainer is in the tree, in `LICENSE` and `README.md`, at the pinned commit, not only in an issue comment.
- For patch 09, the one patch by an outside author in the current form, that author publishes the same change under a GPL-2.0 `LICENSE` in a repository of their own.

## Decision

The owner decided on 2026-11-19: the PC images carry the set, and the owner accepts the remaining risk of the authors other than the maintainer, on the basis of the full list in the table above (Ansa89, MaKoTo, substring, Calamity and amxcs) and of the reasons under that table. The owner also decided on 2026-11-19 to add the written offer to every image that is given out, PC and Raspberry Pi (point 6).

1. The licence of the 15 kHz patch set, as NEPTRON applies it, is GPL-2.0-only. The evidence is the `LICENSE` file and the `README.md` section at the pinned commit, and the answer in issue 22. With that licence, a kernel built with the set can be distributed under GPL-2.0, as the rest of the kernel.
2. The pin moves to `d85a343379f7f54155f69ef56dc544b666635894`, the newest commit of the repository on 2026-11-19, which has the licence. The nine patch URLs in `os/configs/neptron_pc_x86_64_defconfig` name this commit. The SHA-256 values in `os/patches/linux/linux.hash` do not change, because the files do not change.
3. Every PC image, release and development, applies the set. `cargo xtask os build --arch x86_64 --no-kernel-15khz` stays, for a comparison with the unpatched driver.
4. The Raspberry Pi image still takes no kernel patch. Its kernel is a different fork, and the set changes the PC GPU drivers and the DRM core only. `os/board/rpi/check-kernel-patches.sh` and its test stay.
5. The image names the set, its source, its commit and its licence in `LICENCES.txt` on the boot partition and in `/usr/share/licenses`. The same file goes into the image built with `--no-kernel-15khz`, so it describes both variants and says which root PARTUUID each one has. The kernel source of an image is the `legal-info` output of its build: `sources/linux-6.18.63/` holds the kernel archive, the patches that the image applies and the `series` file in the order of application.
6. Every image that is given out, PC and Raspberry Pi, carries the same written offer (GPL-2.0 section 3(b), and the same rule for every GPL and LGPL package of the image) in its `LICENCES.txt` (`os/board/pc` and `os/board/rpi`): for at least three years from the last date on which the project gives out the image, the project gives any third party, on request, a complete machine-readable copy of the corresponding source, the kernel with any patches that the image applies included, on a medium customarily used for software interchange (usually a download), under the terms of the licence of each package (the GPL for a GPL package, the LGPL for an LGPL package), for no more than the cost of the physical act of the source distribution. The period counts from the last distribution because the image holds no build date. The request goes to an issue in the public repository, https://github.com/paologaleotti/neptron, and names the image file and its SHA-256 (and the NEPTRON commit, if the person knows it). The rule for the source is the same in both notes and in `os/README.md`: the project gives out the source together with the image, from the same place, and the offer applies in every case, also for an image that reached somebody without its source. The offer names no person. The binary-only firmware of the Pi has no source and is not covered. The QEMU aarch64 test image has no `LICENCES.txt` and is not given to anybody (CI does not upload it), so it needs no offer. The owner decided to add the offer on 2026-11-19.
7. The repository has no copy of the set. A change of the pin is a new check of the licence: read `LICENSE` and the licence section of `README.md` at the new commit, and the history of any new author. A licence that does not allow distribution under GPL-2.0-only stops the update.

## Consequences

- The release PC image and its CI artifact can carry the patched kernel. CI must change for that: `.github/image-targets.json` and `.github/workflows/images.yml` still build the PC image with `--no-kernel-15khz`, and the check step of OP-070 still refuses a patched kernel. The task report gives the change.
- What the repository does today for the source: `cargo xtask os build` runs `make legal-info`, and the sources stay in the Docker output volume of the build (`legal-info/sources`). CI copies them into a sources artifact beside the image artifact, with 7 days of retention, the same as the image. Nothing yet publishes a lasting source bundle beside an image. The written offer covers an image whose source is not beside it, and the project must be able to answer a request for three years, so it must keep the `legal-info` sources of every image it gives out.
- Requirement for the release task M14: the release publishes the `legal-info` sources archive beside each image, PC and Raspberry Pi, in the same place and for as long as the image. `os/README.md` records this in its section "Licences".
- Requirement for the release task M14: the image carries the NEPTRON commit and the build date in a file on its boot partition, so a person with only the image can name it in a request. Today the image holds neither. `os/README.md` records this beside the requirement above.
- Open choice for the owner: the contact is an issue in the public repository, and an issue needs a GitHub account. A project mailbox would let any third party ask. None exists today.
- The set changes the DRM core (`drm_modes.c`, `drm_probe_helper.c`, `drm_edid.c`, `drm_connector.c` and others). The tier 3 checks on `vkms` (`kms-check`, `selftest-check`) see those changes only when they run on the patched image.
- Remaining risk: the authors in the table above, other than D0023R, did not state GPL-2.0 for their contributions to this repository. The reasons above make the risk small, but they do not remove it. The owner accepted it on 2026-11-19 (see Decision). If an author objects, the plan of OP-070 applies: NEPTRON writes its own patch from the kernel source and documentation and removes the set (or the patch of that author).

## Alternatives

- Keep the old pin `8c96dd9f`: the files are the same, but that commit has the GPL-3.0 `LICENSE`. A pin with the licence in its own tree is a better record.
- Copy the nine patches into `os/patches/linux`: no download at build time, but a copy of third-party files in a repository whose files are `MIT OR Apache-2.0`, and a second place to keep in step with upstream.
- Write NEPTRON's own patch now (the plan of OP-070 for a GPL-3.0 answer): no dependency on the set, but bench work to learn which modes the unpatched driver refuses, and new kernel code to keep for each kernel version. Not needed while the set is GPL-2.0-only.
