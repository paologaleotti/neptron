# 0019: The GPL frontend loads the non-commercial cores in its own process

- Status: accepted
- Date: 2026-11-23
- Task: M14

## Context

The `neptron` binary links Switchres, which is GPL-2.0-or-later. ADR 0002 makes the binary GPL-3.0-or-later. The same binary runs the libretro cores. The images ship four cores:

| Core | Library in the image | Licence |
| --- | --- | --- |
| FCEUmm | `fceumm_libretro.so` | GPL-2.0-or-later |
| Snes9x | `snes9x_libretro.so` | The Snes9x licence: use, copies and changes for non-commercial purposes, free of charge, with the licence and the copyright notice. No sale, no commercial use. |
| Genesis Plus GX | `genesis_plus_gx_libretro.so` | The Genesis Plus GX licence: the same kind of non-commercial terms. Parts of the library are under other licences, which its `LICENSE.txt` lists: LGPL-2.1-or-later (the Nuked OPN2 core, the NTSC filter, Blip Buffer), BSD-3-Clause, CC0-1.0, MIT, zlib and public domain. |
| FinalBurn Neo | `fbneo_libretro.so` | The FB Neo licence with the MAME licence: no sale, no monetary profit, no requests for donations (ADR 0014). The libraries in it keep their own licences. |

How the frontend uses a core:

- Each core is a separate shared object, built from its own upstream source by its own Buildroot package, and installed in `/usr/lib/libretro`. No core is linked into `neptron` at build time, and no NEPTRON code is in a core.
- At run time `neptron-libretro` opens one core with `dlopen` (through `libloading`) in the frontend process. The two sides talk only through the libretro API (`libretro.h`), a published C interface that many frontends and cores implement. They make function calls to each other and share the buffers of that API (video frames, audio samples, input state, memory regions).
- The user can delete a core file, or build an image without a core. The frontend then runs the other cores.

The image ships the cores beside GPL-2.0-only, GPL-2.0-or-later, GPL-3.0-or-later, LGPL and permissive programs. RetroArch and Lakka give out GPL frontends with the same non-commercial cores in the same way.

The FSF reads a plugin that a program loads into its own process and that shares data structures with it as one combined program (the GPL FAQ on plugins). On that reading, the GPL of the frontend reaches the combination, and the non-commercial terms of the cores are a further restriction that the GPL does not allow. Other readings treat a core behind a published, neutral interface as a separate program. Release review 0.1 (area 8, finding 8) recorded this as a doubt. No court decision settles it for libretro cores.

## Decision

The owner decided on 2026-11-23:

1. The frontend loads the non-commercial cores (Snes9x, Genesis Plus GX, FinalBurn Neo) in its own process, through the libretro API, as separate shared objects. The project takes this as its position: the same distribution model as RetroArch and Lakka.
2. The project does not ask the Switchres authors for an exception, and it does not move Switchres out of the process that loads the cores.
3. The images state the non-commercial condition for the cores that carry it and for the image as an aggregate that holds them, and not as a restriction on the GPL parts. `LICENCES.txt` says: the image contains programs under different licences; FinalBurn Neo, Snes9x and Genesis Plus GX may not be sold or used commercially under their own licences, so this image, which contains them, may not be sold; every other program keeps its own licence, and its rights are not reduced; remove those cores to redistribute the rest under its own terms.

## Consequences

- The image as a whole may not be sold, and no device or service may be sold because it holds the image. This follows from the licences of the three cores, not from the GPL parts.
- A user may remove a core file, or build an image without the non-commercial cores. The rest of the image then carries only the terms of its own licences.
- The owner accepts the risk that a copyright holder of Switchres, or of a core, reads the combination as one work and objects. If a holder objects, the options are those of review finding 8: an exception from the Switchres authors, Switchres in a separate process, or no non-commercial core in the image.
- ADR 0014 point 3 still holds: no donation, sponsorship or payment while FinalBurn Neo is in an image.
- A new core with a non-commercial licence follows this ADR and gets its line in `LICENCES.txt`.

## Alternatives

- Ask the Switchres authors for a plugin exception: a clean answer if they give it, but four authors must agree, and the release waits for them.
- Run Switchres in a separate process, so the process that loads the cores holds no GPL code: a new process and an IPC path in the display path, which the safety rules keep small. Not for 0.1.
- Ship no non-commercial core: no Super Nintendo, Mega Drive or arcade games in 0.1.
