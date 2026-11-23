# Arcade policy table

This file is the output of `crates/neptron-modeline/tests/integration/arcade_table.rs` (test `arcade_table`), a copy of a table that the code makes. `docs/spec/arcade.md`, Appendix A, says what it is and how to read it; the boards are in its Appendix B. Output of 2026-11-23; it is equal to the run of 2026-11-16.

```text
cargo xtask test -p neptron-modeline --no-capture arcade_table
```

### `ddonpach`: 320x240 frame, R90, 57.550 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| arcade_15 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| arcade_15ex | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| h9110 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| polo | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| k7131 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| k7000 | interlaced 1:1 | 597x448i | 545 | 15.682 | 57.550 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| ntsc | interlaced 1:1 | 597x448i | 525 | 15.734 | 59.940 | nearest in range +4.15 % | 1 | nearest | 71.4 % | 128 | +0.1 % | none | ok |
| pal | interlaced 1:1 | 597x448i | 625 | 15.625 | 50.000 | nearest in range -13.12 % | 1 | nearest | 71.4 % | 128 | +0.1 % | none | ok |
| arcade_15_25 | native | 533x400p | 433 | 24.919 | 57.550 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_15_25_31 | native | 533x400p | 433 | 24.919 | 57.550 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_15_31 | native | 668x501p | 546 | 31.422 | 57.550 | exact | 1 | nearest | 63.8 % | 181 | +0.0 % | 1x 320p | ok |
| m3129 | native | 659x494p | 539 | 31.019 | 57.550 | exact | 1 | nearest | 64.7 % | 174 | -0.1 % | 1x 320p | ok |
| d9200 | native | 659x494p | 539 | 31.019 | 57.550 | exact | 1 | nearest | 64.7 % | 174 | -0.1 % | 1x 320p | ok |
| d9400 | native | 427x320p | 357 | 20.545 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 320p | ok |
| d9800 | native | 427x320p | 357 | 20.545 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 320p | ok |
| ms929 | native | 512x384p | 425 | 24.458 | 57.550 | exact | 1 | nearest | 83.3 % | 64 | +0.0 % | 1x 320p | ok |
| ms2930 | native | 512x384p | 427 | 24.573 | 57.550 | exact | 1 | nearest | 83.3 % | 64 | +0.0 % | 1x 320p | ok |
| r666b | native | 512x384p | 427 | 24.573 | 57.550 | exact | 1 | nearest | 83.3 % | 64 | +0.0 % | 1x 320p | ok |
| pstar | native | 533x400p | 440 | 25.322 | 57.550 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_25 | native | 533x400p | 433 | 24.919 | 57.550 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_31 | native | 668x501p | 546 | 31.422 | 57.550 | exact | 1 | nearest | 63.8 % | 181 | +0.0 % | 1x 320p | ok |
| pc_31_120 | native | 683x512p | 546 | 31.422 | 57.550 | exact | 1 | nearest | 62.5 % | 192 | +0.0 % | 1x 320p | ok |
| pc_70_120 | 2x | 427x640p | 668 | 38.443 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 3x 960p | ok |
| m2929 | 2x | 427x640p | 662 | 38.098 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 2x 640p | ok |
| edid_31 | native | 693x520p | 539 | 31.019 | 57.550 | exact | 1 | nearest | 61.5 % | 200 | +0.0 % | 1x 320p | ok |
| edid_30_54 | 2x | 427x640p | 664 | 38.213 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 2x 640p | ok |
| edid_30_70 | 2x | 427x640p | 664 | 38.213 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 3x 960p | ok |
| edid_30_96 | 2x | 427x640p | 665 | 38.270 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 4x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +4.26 % | 2 | nearest | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +3.89 % | 2 | nearest | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +4.26 % | 3 | nearest | 88.8 % | 120 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +4.17 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +4.26 % | 6.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +4.15 % | 1 | sharp | 66.6 % | 160 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -13.12 % | 1 | sharp | 55.5 % | 256 | +0.0 % | - | ok |

### `batsugun`: 320x240 frame, R90, 59.630 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| arcade_15 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| arcade_15ex | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| h9110 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| polo | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| k7131 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| k7000 | interlaced 1:1 | 597x448i | 527 | 15.712 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| ntsc | interlaced 1:1 | 597x448i | 525 | 15.734 | 59.940 | nearest in range +0.52 % | 1 | nearest | 71.4 % | 128 | +0.1 % | none | ok |
| pal | interlaced 1:1 | 597x448i | 625 | 15.625 | 50.000 | nearest in range -16.15 % | 1 | nearest | 71.4 % | 128 | +0.1 % | none | ok |
| arcade_15_25 | native | 515x386p | 418 | 24.925 | 59.630 | exact | 1 | nearest | 82.9 % | 66 | -0.1 % | 1x 320p | ok |
| arcade_15_25_31 | native | 515x386p | 418 | 24.925 | 59.630 | exact | 1 | nearest | 82.9 % | 66 | -0.1 % | 1x 320p | ok |
| arcade_15_31 | native | 643x482p | 527 | 31.424 | 59.630 | exact | 1 | nearest | 66.3 % | 162 | -0.1 % | 1x 320p | ok |
| m3129 | native | 633x475p | 520 | 31.007 | 59.630 | exact | 1 | nearest | 67.3 % | 155 | +0.1 % | 1x 320p | ok |
| d9200 | native | 633x475p | 520 | 31.007 | 59.630 | exact | 1 | nearest | 67.3 % | 155 | +0.1 % | 1x 320p | ok |
| d9400 | native | 427x320p | 355 | 21.168 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 320p | ok |
| d9800 | native | 427x320p | 355 | 21.168 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 320p | ok |
| ms929 | interlaced 1:1 | 597x448i | 521 | 15.533 | 59.630 | exact | 1 | nearest | 71.4 % | 128 | +0.1 % | 1x 320i | ok |
| ms2930 | native | 640x480p | 528 | 31.484 | 59.630 | exact | 1 | nearest | 66.6 % | 160 | +0.0 % | 1x 320p | ok |
| r666b | native | 631x473p | 520 | 31.007 | 59.630 | exact | 1 | nearest | 67.6 % | 153 | -0.1 % | 1x 320p | ok |
| pstar | native | 533x400p | 425 | 25.342 | 59.630 | exact | 1 | nearest | 80.0 % | 80 | +0.1 % | 1x 320p | ok |
| arcade_25 | native | 515x386p | 418 | 24.925 | 59.630 | exact | 1 | nearest | 82.9 % | 66 | -0.1 % | 1x 320p | ok |
| arcade_31 | native | 643x482p | 527 | 31.424 | 59.630 | exact | 1 | nearest | 66.3 % | 162 | -0.1 % | 1x 320p | ok |
| pc_31_120 | native | 679x509p | 527 | 31.425 | 59.630 | exact | 1 | nearest | 62.8 % | 189 | +0.0 % | 1x 320p | ok |
| pc_70_120 | 2x | 427x640p | 669 | 39.892 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 3x 960p | ok |
| m2929 | 2x | 427x640p | 663 | 39.535 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 2x 640p | ok |
| edid_31 | native | 668x501p | 520 | 31.007 | 59.629 | exact | 1 | nearest | 63.8 % | 181 | +0.0 % | 1x 320p | ok |
| edid_30_54 | 2x | 427x640p | 665 | 39.654 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 2x 640p | ok |
| edid_30_70 | 2x | 427x640p | 665 | 39.654 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 3x 960p | ok |
| edid_30_96 | 2x | 427x640p | 666 | 39.713 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | -0.1 % | 4x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.62 % | 2 | nearest | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +0.27 % | 2 | nearest | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.62 % | 3 | nearest | 88.8 % | 120 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +0.54 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.62 % | 6.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +0.52 % | 1 | sharp | 66.6 % | 160 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -16.15 % | 1 | sharp | 55.5 % | 256 | +0.0 % | - | ok |

### `gunbird`: 320x224 frame, R90, 59.290 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| arcade_15 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| arcade_15ex | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| h9110 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| polo | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| k7131 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| k7000 | interlaced 1:1 | 558x448i | 529 | 15.682 | 59.290 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| ntsc | interlaced 1:1 | 558x448i | 525 | 15.734 | 59.940 | nearest in range +1.10 % | 1 | nearest | 71.4 % | 128 | -0.1 % | none | ok |
| pal | interlaced 1:1 | 558x448i | 625 | 15.625 | 50.000 | nearest in range -15.67 % | 1 | nearest | 71.4 % | 128 | -0.1 % | none | ok |
| arcade_15_25 | native | 483x388p | 420 | 24.901 | 59.290 | exact | 1 | nearest | 82.4 % | 68 | +0.0 % | 1x 320p | ok |
| arcade_15_25_31 | native | 483x388p | 420 | 24.901 | 59.290 | exact | 1 | nearest | 82.4 % | 68 | +0.0 % | 1x 320p | ok |
| arcade_15_31 | native | 604x485p | 530 | 31.423 | 59.290 | exact | 1 | nearest | 65.9 % | 165 | -0.1 % | 1x 320p | ok |
| m3129 | native | 595x478p | 523 | 31.008 | 59.289 | exact | 1 | nearest | 66.9 % | 158 | +0.0 % | 1x 320p | ok |
| d9200 | native | 595x478p | 523 | 31.008 | 59.289 | exact | 1 | nearest | 66.9 % | 158 | +0.0 % | 1x 320p | ok |
| d9400 | native | 398x320p | 355 | 21.047 | 59.289 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 320p | ok |
| d9800 | native | 398x320p | 355 | 21.047 | 59.289 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 320p | ok |
| ms929 | interlaced 1:1 | 558x448i | 523 | 15.504 | 59.289 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| ms2930 | native | 597x480p | 527 | 31.245 | 59.290 | exact | 1 | nearest | 66.6 % | 160 | +0.1 % | 1x 320p | ok |
| r666b | native | 592x476p | 523 | 31.008 | 59.289 | exact | 1 | nearest | 67.2 % | 156 | +0.1 % | 1x 320p | ok |
| pstar | native | 498x400p | 427 | 25.317 | 59.290 | exact | 1 | nearest | 80.0 % | 80 | +0.0 % | 1x 320p | ok |
| arcade_25 | native | 483x388p | 420 | 24.901 | 59.290 | exact | 1 | nearest | 82.4 % | 68 | +0.0 % | 1x 320p | ok |
| arcade_31 | native | 604x485p | 530 | 31.423 | 59.290 | exact | 1 | nearest | 65.9 % | 165 | -0.1 % | 1x 320p | ok |
| pc_31_120 | native | 637x512p | 530 | 31.423 | 59.290 | exact | 1 | nearest | 62.5 % | 192 | +0.0 % | 1x 320p | ok |
| pc_70_120 | 2x | 398x640p | 669 | 39.664 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 960p | ok |
| m2929 | 2x | 398x640p | 663 | 39.309 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 640p | ok |
| edid_31 | native | 627x504p | 523 | 31.008 | 59.290 | exact | 1 | nearest | 63.4 % | 184 | +0.0 % | 1x 320p | ok |
| edid_30_54 | 2x | 398x640p | 665 | 39.428 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 640p | ok |
| edid_30_70 | 2x | 398x640p | 665 | 39.428 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 960p | ok |
| edid_30_96 | 2x | 398x640p | 666 | 39.487 | 59.290 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +1.20 % | 2 | sharp | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +0.84 % | 2 | sharp | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +1.20 % | 3 | sharp | 88.8 % | 120 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +1.11 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +1.20 % | 6.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +1.10 % | 1 | sharp | 66.6 % | 160 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -15.67 % | 1 | nearest | 55.5 % | 256 | +1.8 % | - | ok |

### `gunlock`: 320x224 frame, R270, 58.940 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| arcade_15 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| arcade_15ex | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| h9110 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| polo | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| k7131 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| k7000 | interlaced 1:1 | 558x448i | 533 | 15.707 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| ntsc | interlaced 1:1 | 558x448i | 525 | 15.734 | 59.940 | nearest in range +1.70 % | 1 | nearest | 71.4 % | 128 | -0.1 % | none | ok |
| pal | interlaced 1:1 | 558x448i | 625 | 15.625 | 50.000 | nearest in range -15.17 % | 1 | nearest | 71.4 % | 128 | -0.1 % | none | ok |
| arcade_15_25 | native | 487x391p | 423 | 24.931 | 58.940 | exact | 1 | nearest | 81.8 % | 71 | -0.1 % | 1x 320p | ok |
| arcade_15_25_31 | native | 487x391p | 423 | 24.931 | 58.940 | exact | 1 | nearest | 81.8 % | 71 | -0.1 % | 1x 320p | ok |
| arcade_15_31 | native | 607x488p | 533 | 31.415 | 58.940 | exact | 1 | nearest | 65.5 % | 168 | +0.0 % | 1x 320p | ok |
| m3129 | native | 599x481p | 526 | 31.002 | 58.940 | exact | 1 | nearest | 66.5 % | 161 | -0.1 % | 1x 320p | ok |
| d9200 | native | 599x481p | 526 | 31.002 | 58.940 | exact | 1 | nearest | 66.5 % | 161 | -0.1 % | 1x 320p | ok |
| d9400 | native | 398x320p | 355 | 20.923 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 320p | ok |
| d9800 | native | 398x320p | 355 | 20.923 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 320p | ok |
| ms929 | interlaced 1:1 | 558x448i | 527 | 15.530 | 58.940 | exact | 1 | nearest | 71.4 % | 128 | -0.1 % | 1x 320i | ok |
| ms2930 | native | 597x480p | 527 | 31.061 | 58.940 | exact | 1 | nearest | 66.6 % | 160 | +0.1 % | 1x 320p | ok |
| r666b | native | 596x479p | 526 | 31.002 | 58.941 | exact | 1 | nearest | 66.8 % | 159 | +0.0 % | 1x 320p | ok |
| pstar | native | 498x400p | 430 | 25.344 | 58.940 | exact | 1 | nearest | 80.0 % | 80 | +0.0 % | 1x 320p | ok |
| arcade_25 | native | 487x391p | 423 | 24.931 | 58.940 | exact | 1 | nearest | 81.8 % | 71 | -0.1 % | 1x 320p | ok |
| arcade_31 | native | 607x488p | 533 | 31.415 | 58.940 | exact | 1 | nearest | 65.5 % | 168 | +0.0 % | 1x 320p | ok |
| pc_31_120 | native | 637x512p | 533 | 31.415 | 58.940 | exact | 1 | nearest | 62.5 % | 192 | +0.0 % | 1x 320p | ok |
| pc_70_120 | 2x | 398x640p | 669 | 39.430 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 960p | ok |
| m2929 | 2x | 398x640p | 662 | 39.017 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 640p | ok |
| edid_31 | native | 631x507p | 526 | 31.002 | 58.940 | exact | 1 | nearest | 63.1 % | 187 | +0.0 % | 1x 320p | ok |
| edid_30_54 | 2x | 398x640p | 665 | 39.195 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 640p | ok |
| edid_30_70 | 2x | 398x640p | 665 | 39.195 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 960p | ok |
| edid_30_96 | 2x | 398x640p | 666 | 39.254 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +1.80 % | 2 | sharp | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +1.44 % | 2 | sharp | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +1.80 % | 3 | sharp | 88.8 % | 120 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +1.72 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +1.80 % | 6.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +1.70 % | 1 | sharp | 66.6 % | 160 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -15.17 % | 1 | nearest | 55.5 % | 256 | +1.8 % | - | ok |

### `pacman`: 288x224 frame, R270, 60.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15ex | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| h9110 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| polo | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| k7131 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| k7000 | interlaced 1:1 | 619x448i | 523 | 15.689 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| ntsc | interlaced 1:1 | 619x448i | 525 | 15.734 | 59.940 | nearest in range -0.10 % | 1 | nearest | 64.2 % | 160 | +0.1 % | none | ok |
| pal | native | 398x288p | 312 | 15.600 | 50.000 | nearest in range -16.67 % | 1 | nearest | 100.0 % | 0 | +0.1 % | none | ok |
| arcade_15_25 | native | 531x384p | 416 | 24.959 | 59.999 | exact | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_15_25_31 | native | 531x384p | 416 | 24.959 | 59.999 | exact | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_15_31 | native | 662x479p | 524 | 31.440 | 60.000 | exact | 1 | nearest | 60.1 % | 191 | +0.0 % | 1x 288p | ok |
| m3129 | native | 653x472p | 517 | 31.019 | 60.000 | exact | 1 | nearest | 61.0 % | 184 | -0.1 % | 1x 288p | ok |
| d9200 | 2x | 398x576p | 617 | 37.019 | 59.999 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| d9400 | native | 398x288p | 312 | 18.720 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| d9800 | native | 398x288p | 312 | 18.720 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| ms929 | interlaced 1:1 | 619x448i | 517 | 15.509 | 60.000 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| ms2930 | native | 664x480p | 528 | 31.679 | 60.000 | exact | 1 | nearest | 60.0 % | 192 | +0.0 % | 1x 288p | ok |
| r666b | native | 650x470p | 517 | 31.020 | 60.000 | exact | 1 | nearest | 61.2 % | 182 | +0.0 % | 1x 288p | ok |
| pstar | native | 553x400p | 422 | 25.320 | 60.000 | exact | 1 | nearest | 72.0 % | 112 | +0.0 % | 1x 288p | ok |
| arcade_25 | native | 531x384p | 416 | 24.959 | 59.999 | exact | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_31 | native | 662x479p | 524 | 31.440 | 60.000 | exact | 1 | nearest | 60.1 % | 191 | +0.0 % | 1x 288p | ok |
| pc_31_120 | native | 700x506p | 524 | 31.439 | 60.000 | exact | 1 | nearest | 56.9 % | 218 | +0.0 % | 1x 288p | ok |
| pc_70_120 | 2x | 398x576p | 602 | 36.119 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 864p | ok |
| m2929 | 2x | 398x576p | 597 | 35.819 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| edid_31 | native | 689x498p | 517 | 31.019 | 60.000 | exact | 1 | nearest | 57.8 % | 210 | -0.1 % | 1x 288p | ok |
| edid_30_54 | 2x | 398x576p | 599 | 35.939 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 864p | ok |
| edid_30_70 | 2x | 398x576p | 599 | 35.939 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1120p | ok |
| edid_30_96 | 2x | 398x576p | 599 | 35.939 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1152p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.00 % | 2 | nearest | 80.0 % | 144 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -0.35 % | 2 | nearest | 75.0 % | 192 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.00 % | 3 | sharp | 80.0 % | 216 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -0.08 % | 5 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.00 % | 7.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | -0.10 % | 1 | sharp | 60.0 % | 192 | +0.2 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -16.53 % | 1 | sharp | 100.0 % | 0 | +0.0 % | - | ok |

### `pacman_60606`: 288x224 frame, R270, 60.606 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15ex | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| h9110 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| polo | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| k7131 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| k7000 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| ntsc | interlaced 1:1 | 619x448i | 525 | 15.734 | 59.940 | nearest in range -1.10 % | 1 | nearest | 64.2 % | 160 | +0.1 % | none | ok |
| pal | native | 398x288p | 312 | 15.600 | 50.000 | nearest in range -17.50 % | 1 | nearest | 100.0 % | 0 | +0.1 % | none | ok |
| arcade_15_25 | interlaced 1:1 | 619x448i | 517 | 15.666 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| arcade_15_25_31 | native | 654x473p | 519 | 31.454 | 60.606 | exact | 1 | nearest | 60.8 % | 185 | +0.0 % | 1x 288p | ok |
| arcade_15_31 | native | 654x473p | 519 | 31.454 | 60.606 | exact | 1 | nearest | 60.8 % | 185 | +0.0 % | 1x 288p | ok |
| m3129 | native | 646x467p | 512 | 31.029 | 60.605 | exact | 1 | nearest | 61.6 % | 179 | +0.0 % | 1x 288p | ok |
| d9200 | 2x | 398x576p | 611 | 37.030 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| d9400 | native | 398x288p | 312 | 18.908 | 60.606 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| d9800 | native | 398x288p | 312 | 18.908 | 60.606 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| ms929 | interlaced 1:1 | 619x448i | 511 | 15.484 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| ms2930 | interlaced 1:1 | 619x448i | 511 | 15.484 | 60.606 | exact | 1 | nearest | 64.2 % | 160 | +0.1 % | 1x 288i | ok |
| r666b | native | 643x465p | 512 | 31.030 | 60.606 | exact | 1 | nearest | 61.9 % | 177 | +0.0 % | 1x 288p | ok |
| pstar | native | 552x399p | 418 | 25.333 | 60.606 | exact | 1 | nearest | 72.1 % | 111 | -0.1 % | 1x 288p | ok |
| arcade_25 | native | 531x384p | 416 | 24.959 | 59.999 | nearest in range -1.00 % | 1 | nearest | 75.0 % | 96 | +0.0 % | none | ok: no window at the game's rate |
| arcade_31 | native | 654x473p | 519 | 31.454 | 60.606 | exact | 1 | nearest | 60.8 % | 185 | +0.0 % | 1x 288p | ok |
| pc_31_120 | native | 693x501p | 519 | 31.454 | 60.606 | exact | 1 | nearest | 57.4 % | 213 | +0.0 % | 1x 288p | ok |
| pc_70_120 | 2x | 398x576p | 603 | 36.545 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 864p | ok |
| m2929 | 2x | 398x576p | 597 | 36.181 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 576p | ok |
| edid_31 | native | 682x493p | 512 | 31.030 | 60.606 | exact | 1 | nearest | 58.4 % | 205 | +0.0 % | 1x 288p | ok |
| edid_30_54 | 2x | 398x576p | 599 | 36.303 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 852p | ok |
| edid_30_70 | 2x | 398x576p | 599 | 36.303 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1104p | ok |
| edid_30_96 | 2x | 398x576p | 600 | 36.363 | 60.606 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1152p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | -1.00 % | 2 | nearest | 80.0 % | 144 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -1.35 % | 2 | nearest | 75.0 % | 192 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | -1.00 % | 3 | sharp | 80.0 % | 216 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -1.08 % | 5 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | -1.00 % | 7.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | -1.10 % | 1 | sharp | 60.0 % | 192 | +0.2 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -17.37 % | 1 | sharp | 100.0 % | 0 | +0.0 % | - | ok |

### `dkong`: 256x224 frame, R270, 60.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 373x240p | 261 | 15.659 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 240p | ok |
| arcade_15 | native | 386x248p | 269 | 16.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 248p | ok |
| arcade_15ex | native | 392x252p | 273 | 16.380 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 252p | ok |
| h9110 | native | 398x256p | 277 | 16.620 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 256p | ok |
| polo | native | 398x256p | 277 | 16.620 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 256p | ok |
| k7131 | native | 398x256p | 277 | 16.620 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 256p | ok |
| k7000 | native | 376x242p | 262 | 15.720 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 1x 242p | ok |
| ntsc | native | 373x240p | 262 | 15.704 | 59.940 | nearest in range -0.10 % | 1 | nearest | 100.0 % | 0 | +0.1 % | none | ok |
| pal | native | 398x256p | 312 | 15.600 | 50.000 | nearest in range -16.67 % | 1 | nearest | 100.0 % | 0 | +0.1 % | none | ok |
| arcade_15_25 | native | 386x248p | 269 | 16.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 256p | ok |
| arcade_15_25_31 | native | 386x248p | 269 | 16.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 256p | ok |
| arcade_15_31 | native | 386x248p | 269 | 16.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 256p | ok |
| m3129 | native | 389x250p | 273 | 16.380 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 484p | ok |
| d9200 | 2x | 398x512p | 617 | 37.019 | 59.999 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| d9400 | native | 398x256p | 279 | 16.739 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| d9800 | native | 398x256p | 279 | 16.739 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| ms929 | native | 380x244p | 266 | 15.960 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | -0.1 % | 1x 244p | ok |
| ms2930 | native | 376x242p | 267 | 16.020 | 60.001 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 484p | ok |
| r666b | native | 376x242p | 267 | 16.020 | 60.001 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 488p | ok |
| pstar | native | 398x256p | 270 | 16.200 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.1 % | 2x 500p | ok |
| arcade_25 | native | 597x384p | 416 | 24.959 | 60.000 | exact | 1 | nearest | 66.6 % | 128 | +0.1 % | 1x 256p | ok |
| arcade_31 | native | 745x479p | 524 | 31.439 | 60.000 | exact | 1 | nearest | 53.4 % | 223 | +0.0 % | 1x 256p | ok |
| pc_31_120 | 2x | 392x504p | 524 | 31.439 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 504p | ok |
| pc_70_120 | 2x | 398x512p | 535 | 32.100 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1024p | ok |
| m2929 | 2x | 398x512p | 530 | 31.800 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| edid_31 | 2x | 398x512p | 531 | 31.859 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 2x 512p | ok |
| edid_30_54 | 2x | 398x512p | 532 | 31.919 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 3x 768p | ok |
| edid_30_70 | 2x | 398x512p | 532 | 31.919 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1024p | ok |
| edid_30_96 | 2x | 398x512p | 533 | 31.980 | 60.000 | exact | 2 | nearest | 100.0 % | 0 | +0.1 % | 4x 1024p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.00 % | 2 | nearest | 71.1 % | 208 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -0.35 % | 3 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.00 % | 4.22 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -0.08 % | 5.62 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.00 % | 8.44 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +0.09 % | 1 | sharp | 100.0 % | 0 | -0.1 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -16.53 % | 1 | sharp | 88.8 % | 32 | +0.0 % | - | ok |

### `gground`: 496x384 frame, R270, 57.524 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| arcade_15 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| arcade_15ex | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| h9110 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| polo | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| k7131 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| k7000 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| ntsc | fitted | 640x480i | 525 | 15.734 | 59.940 | nearest in range +4.20 % | 0.97 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| pal | interlaced 1:1 | 683x496i | 625 | 15.625 | 50.000 | nearest in range -13.08 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | interlaced 1:1 | 683x496i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| arcade_15_25_31 | native | 683x496p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| arcade_15_31 | native | 683x496p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| m3129 | native | 683x496p | 540 | 31.063 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| d9200 | native | 683x496p | 540 | 31.063 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| d9400 | native | 683x496p | 557 | 32.040 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| d9800 | native | 683x496p | 557 | 32.040 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| ms929 | interlaced 1:1 | 683x496i | 541 | 15.560 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496i | ok |
| ms2930 | native | 683x496p | 543 | 31.235 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| r666b | native | 683x496p | 543 | 31.235 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| pstar | native | 683x496p | 548 | 31.523 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| arcade_25 | interlaced 1:1 | 1057x768i | 867 | 24.936 | 57.524 | exact | 1 | nearest | 64.5 % | 272 | +0.0 % | 1x 496i | ok |
| arcade_31 | native | 683x496p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| pc_31_120 | native | 683x496p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| pc_70_120 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 992p | ok |
| m2929 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| edid_31 | native | 683x496p | 539 | 31.005 | 57.525 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| edid_30_54 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 496p | ok |
| edid_30_70 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 992p | ok |
| edid_30_96 | native | 683x496p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 3x 1488p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +4.30 % | 1 | sharp | 68.8 % | 224 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +3.94 % | 1 | sharp | 64.5 % | 272 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +4.30 % | 2 | nearest | 91.8 % | 88 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +4.22 % | 2 | nearest | 68.8 % | 448 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +4.30 % | 4.35 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | fitted | 720x480i | 525 | 15.734 | 59.940 | +4.20 % | 0.97 | sharp | 100.0 % | 0 | +0.0 % | - | ok: no exact mode exists on this profile |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -13.08 % | 1 | sharp | 86.1 % | 80 | +0.0 % | - | ok |

### `ikaruga`: 640x480 frame, R270, 60.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | fitted | 640x480i | 523 | 15.689 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| arcade_15 | fitted | 661x496i | 539 | 16.169 | 60.000 | exact | 0.78 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| arcade_15ex | fitted | 672x504i | 547 | 16.409 | 60.000 | exact | 0.79 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| h9110 | fitted | 682x512i | 555 | 16.650 | 60.000 | exact | 0.80 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| polo | fitted | 682x512i | 555 | 16.650 | 60.000 | exact | 0.80 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| k7131 | fitted | 682x512i | 555 | 16.650 | 60.000 | exact | 0.80 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| k7000 | fitted | 640x480i | 523 | 15.689 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| ntsc | fitted | 640x480i | 525 | 15.734 | 59.940 | nearest in range -0.10 % | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| pal | fitted | 768x576i | 625 | 15.625 | 50.000 | nearest in range -16.67 % | 0.90 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| arcade_15_25 | interlaced 1:1 | 1024x768i | 831 | 24.929 | 60.000 | exact | 1 | nearest | 83.3 % | 128 | +0.0 % | 1x 640i | ok |
| arcade_15_25_31 | interlaced 1:1 | 1024x768i | 831 | 24.929 | 60.000 | exact | 1 | nearest | 83.3 % | 128 | +0.0 % | 1x 640i | ok |
| arcade_15_31 | fitted | 640x480p | 525 | 31.500 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| m3129 | fitted | 640x480p | 525 | 31.500 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| d9200 | fitted | 800x600p | 628 | 37.680 | 60.000 | exact | 0.94 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| d9400 | fitted | 800x600p | 628 | 37.680 | 60.000 | exact | 0.94 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| d9800 | fitted | 800x600p | 628 | 37.680 | 60.000 | exact | 0.94 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| ms929 | fitted | 650x488i | 533 | 15.989 | 59.999 | exact | 0.76 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| ms2930 | fitted | 640x480p | 528 | 31.679 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: no exact mode exists on this profile |
| r666b | fitted | 650x488p | 537 | 32.220 | 60.000 | exact | 0.76 | sharp | 100.0 % | 0 | +0.1 % | none | ok: no exact mode exists on this profile |
| pstar | interlaced 1:1 | 1024x768i | 845 | 25.349 | 60.000 | exact | 1 | nearest | 83.3 % | 128 | +0.0 % | 1x 640i | ok |
| arcade_25 | interlaced 1:1 | 1024x768i | 831 | 24.929 | 60.000 | exact | 1 | nearest | 83.3 % | 128 | +0.0 % | 1x 640i | ok |
| arcade_31 | fitted | 640x480p | 525 | 31.500 | 60.000 | exact | 0.75 | sharp | 100.0 % | 0 | +0.0 % | none | ok: the profile declares no interlace |
| pc_31_120 | fitted | 672x504p | 524 | 31.440 | 60.000 | exact | 0.79 | sharp | 100.0 % | 0 | +0.0 % | none | ok: the profile declares no interlace |
| pc_70_120 | native | 853x640p | 669 | 40.140 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 640p | ok |
| m2929 | native | 853x640p | 663 | 39.780 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 640p | ok |
| edid_31 | fitted | 678x509p | 528 | 31.679 | 60.000 | exact | 0.80 | sharp | 100.0 % | 0 | -0.1 % | none | ok: the profile declares no interlace |
| edid_30_54 | native | 853x640p | 665 | 39.900 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 640p | ok |
| edid_30_70 | native | 853x640p | 666 | 39.959 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 640p | ok |
| edid_30_96 | native | 853x640p | 666 | 39.959 | 60.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 1280p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.00 % | 1 | nearest | 88.8 % | 80 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -0.35 % | 1 | nearest | 83.3 % | 128 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.00 % | 1 | nearest | 59.2 % | 440 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -0.08 % | 2 | nearest | 88.8 % | 160 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.00 % | 3 | nearest | 88.8 % | 240 | +0.0 % | - | ok |
| tv_ntsc | fitted | 720x480i | 525 | 15.734 | 59.940 | -0.10 % | 0.75 | sharp | 100.0 % | 0 | +0.0 % | - | ok: no exact mode exists on this profile |
| tv_pal | fitted | 720x576i | 625 | 15.625 | 50.000 | -16.67 % | 0.90 | sharp | 100.0 % | 0 | +0.0 % | - | ok: no exact mode exists on this profile |

### `neogeo`: 304x224 frame, R0, 59.180 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15ex | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| h9110 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| polo | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7131 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7000 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ntsc | native | 304x224p | 262 | 15.704 | 59.940 | nearest in range +1.28 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 304x224p | 312 | 15.600 | 50.000 | nearest in range -15.51 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15_25_31 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_15_31 | native | 304x224p | 265 | 15.682 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| m3129 | native | 304x224p | 258 | 15.268 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9200 | native | 304x224p | 258 | 15.268 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9400 | native | 304x224p | 258 | 15.268 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9800 | native | 304x224p | 258 | 15.268 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| ms929 | native | 304x224p | 262 | 15.505 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ms2930 | native | 304x224p | 262 | 15.505 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| r666b | native | 304x224p | 262 | 15.505 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pstar | native | 304x224p | 266 | 15.741 | 59.180 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_25 | native | 528x389p | 421 | 24.914 | 59.180 | exact | 1 | nearest | 57.5 % | 165 | +0.0 % | 1x 224p | ok |
| arcade_31 | 2x | 304x448p | 531 | 31.424 | 59.180 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_31_120 | 2x | 304x448p | 531 | 31.424 | 59.180 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_70_120 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| m2929 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 636p | ok |
| edid_31 | 2x | 304x448p | 524 | 31.010 | 59.180 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| edid_30_54 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 872p | ok |
| edid_30_70 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| edid_30_96 | 2x | 304x448p | 507 | 30.005 | 59.182 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +1.39 % | 3 | sharp | 93.3 % | 48 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +1.03 % | 3 | sharp | 87.5 % | 96 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +1.39 % | 4.82 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +1.30 % | 6.43 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +1.39 % | 9.64 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +1.48 % | 1 | sharp | 93.3 % | 16 | +0.0 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -15.38 % | 1 | sharp | 77.7 % | 64 | -0.1 % | - | ok |

### `cps2`: 384x224 frame, R0, 59.630 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15ex | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| h9110 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| polo | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7131 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7000 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ntsc | native | 384x224p | 262 | 15.704 | 59.940 | nearest in range +0.52 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 384x224p | 312 | 15.600 | 50.000 | nearest in range -16.15 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15_25_31 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_15_31 | native | 384x224p | 263 | 15.682 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| m3129 | native | 384x224p | 256 | 15.265 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9200 | native | 384x224p | 256 | 15.265 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9400 | native | 384x224p | 256 | 15.265 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9800 | native | 384x224p | 256 | 15.265 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| ms929 | native | 384x224p | 260 | 15.503 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ms2930 | native | 384x224p | 260 | 15.504 | 59.631 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| r666b | native | 384x224p | 260 | 15.504 | 59.631 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pstar | native | 384x224p | 264 | 15.742 | 59.630 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_25 | native | 662x386p | 418 | 24.925 | 59.630 | exact | 1 | nearest | 58.0 % | 162 | +0.0 % | 1x 224p | ok |
| arcade_31 | 2x | 384x448p | 527 | 31.425 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_31_120 | 2x | 384x448p | 527 | 31.425 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_70_120 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| m2929 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 636p | ok |
| edid_31 | 2x | 384x448p | 520 | 31.008 | 59.631 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| edid_30_54 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 864p | ok |
| edid_30_70 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| edid_30_96 | 2x | 384x448p | 504 | 30.053 | 59.630 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.62 % | 3 | sharp | 93.3 % | 48 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +0.27 % | 3 | sharp | 87.5 % | 96 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.62 % | 4.82 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +0.54 % | 6.43 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.62 % | 9.64 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +0.71 % | 1 | sharp | 93.3 % | 16 | +0.0 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -16.02 % | 1 | sharp | 77.7 % | 64 | -0.1 % | - | ok |

### `pwrinst2`: 320x240 frame, R0, 57.550 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| arcade_15 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| arcade_15ex | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| h9110 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| polo | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| k7131 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| k7000 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| ntsc | native | 320x240p | 262 | 15.704 | 59.940 | nearest in range +4.15 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 320x240p | 312 | 15.600 | 50.000 | nearest in range -13.12 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| arcade_15_25_31 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| arcade_15_31 | native | 320x240p | 272 | 15.653 | 57.551 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| m3129 | native | 320x240p | 265 | 15.250 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| d9200 | native | 320x240p | 265 | 15.250 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| d9400 | native | 320x240p | 265 | 15.250 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| d9800 | native | 320x240p | 265 | 15.250 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| ms929 | native | 320x240p | 269 | 15.480 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 240p | ok |
| ms2930 | native | 320x240p | 269 | 15.480 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| r666b | native | 320x240p | 269 | 15.480 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| pstar | native | 320x240p | 273 | 15.711 | 57.550 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| arcade_25 | native | 533x400p | 433 | 24.919 | 57.550 | exact | 1 | nearest | 60.0 % | 160 | +0.1 % | 1x 240p | ok |
| arcade_31 | 2x | 320x480p | 546 | 31.422 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| pc_31_120 | 2x | 320x480p | 546 | 31.422 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| pc_70_120 | 2x | 320x480p | 522 | 30.041 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 960p | ok |
| m2929 | 2x | 320x480p | 522 | 30.041 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| edid_31 | 2x | 320x480p | 539 | 31.019 | 57.550 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 480p | ok |
| edid_30_54 | 2x | 320x480p | 522 | 30.041 | 57.551 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 904p | ok |
| edid_30_70 | 2x | 320x480p | 522 | 30.041 | 57.551 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 960p | ok |
| edid_30_96 | 2x | 320x480p | 522 | 30.041 | 57.551 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 960p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +4.26 % | 3 | nearest | 100.0 % | 0 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +3.89 % | 3 | nearest | 93.7 % | 48 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +4.26 % | 4.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +4.17 % | 6 | nearest | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +4.26 % | 9 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +4.35 % | 1 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -12.98 % | 1 | sharp | 83.3 % | 48 | +0.1 % | - | ok |

### `dariusg`: 320x232 frame, R0, 58.940 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| arcade_15 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| arcade_15ex | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| h9110 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| polo | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| k7131 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| k7000 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| ntsc | native | 320x232p | 262 | 15.704 | 59.940 | nearest in range +1.70 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 320x232p | 312 | 15.600 | 50.000 | nearest in range -15.17 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| arcade_15_25_31 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| arcade_15_31 | native | 320x232p | 266 | 15.677 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| m3129 | native | 320x232p | 259 | 15.265 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| d9200 | native | 320x232p | 259 | 15.265 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| d9400 | native | 320x232p | 259 | 15.265 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| d9800 | native | 320x232p | 259 | 15.265 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| ms929 | native | 320x232p | 263 | 15.501 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 232p | ok |
| ms2930 | native | 320x232p | 263 | 15.501 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| r666b | native | 320x232p | 263 | 15.501 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| pstar | native | 320x232p | 267 | 15.736 | 58.940 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| arcade_25 | native | 539x391p | 423 | 24.931 | 58.940 | exact | 1 | nearest | 59.3 % | 159 | +0.1 % | 1x 232p | ok |
| arcade_31 | 2x | 320x464p | 533 | 31.414 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| pc_31_120 | 2x | 320x464p | 533 | 31.415 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| pc_70_120 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 928p | ok |
| m2929 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| edid_31 | 2x | 320x464p | 526 | 31.002 | 58.940 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 464p | ok |
| edid_30_54 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 880p | ok |
| edid_30_70 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 928p | ok |
| edid_30_96 | 2x | 320x464p | 509 | 30.000 | 58.939 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 928p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +1.80 % | 3 | sharp | 96.6 % | 24 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +1.44 % | 3 | sharp | 90.6 % | 72 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +1.80 % | 4.66 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +1.72 % | 6.21 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +1.80 % | 9.31 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +1.89 % | 1 | sharp | 96.6 % | 8 | -0.1 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -15.03 % | 1 | sharp | 80.5 % | 56 | +0.0 % | - | ok |

### `rtype`: 384x256 frame, R0, 55.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15ex | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| h9110 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| polo | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| k7131 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| k7000 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| ntsc | native | 384x240p | 262 | 15.704 | 59.940 | nearest in range +8.98 % | 1 | nearest | 100.0 % | 0 | -6.2 % | none | ok: overscan crop |
| pal | native | 384x256p | 312 | 15.600 | 50.000 | nearest in range -9.09 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15_25_31 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| arcade_15_31 | native | 384x256p | 285 | 15.674 | 54.999 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| m3129 | native | 384x256p | 278 | 15.290 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9200 | native | 384x256p | 278 | 15.290 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9400 | native | 384x256p | 278 | 15.290 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9800 | native | 384x256p | 278 | 15.290 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| ms929 | native | 384x256p | 281 | 15.455 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| ms2930 | native | 384x256p | 281 | 15.455 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| r666b | native | 384x256p | 281 | 15.455 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pstar | native | 384x256p | 286 | 15.730 | 55.000 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| arcade_25 | native | 600x400p | 453 | 24.914 | 55.000 | exact | 1 | nearest | 64.0 % | 144 | +0.0 % | 1x 256p | ok |
| arcade_31 | 2x | 384x512p | 571 | 31.404 | 55.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pc_31_120 | 2x | 384x512p | 571 | 31.404 | 55.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pc_70_120 | 2x | 384x512p | 546 | 30.030 | 55.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| m2929 | 2x | 384x512p | 546 | 30.030 | 55.000 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| edid_31 | 2x | 384x512p | 564 | 31.020 | 55.001 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| edid_30_54 | 2x | 384x512p | 546 | 30.030 | 55.001 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 768p | ok |
| edid_30_70 | 2x | 384x512p | 546 | 30.030 | 55.001 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| edid_30_96 | 2x | 384x512p | 546 | 30.030 | 55.001 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +9.09 % | 2 | nearest | 71.1 % | 208 | -0.1 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +8.71 % | 3 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +9.09 % | 4.22 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +9.00 % | 5.62 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +9.09 % | 8.44 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +9.19 % | 1 | sharp | 100.0 % | 0 | -6.2 % | - | ok: overscan crop |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -8.95 % | 1 | sharp | 88.8 % | 32 | +0.1 % | - | ok |

### `mk`: 400x256 frame, R0, 54.700 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15ex | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| h9110 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| polo | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| k7131 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| k7000 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| ntsc | native | 400x240p | 262 | 15.704 | 59.940 | nearest in range +9.58 % | 1 | nearest | 100.0 % | 0 | -6.2 % | none | ok: overscan crop |
| pal | native | 400x256p | 312 | 15.600 | 50.000 | nearest in range -8.59 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| arcade_15_25_31 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| arcade_15_31 | native | 400x256p | 286 | 15.644 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| m3129 | native | 400x256p | 279 | 15.261 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9200 | native | 400x256p | 279 | 15.261 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9400 | native | 400x256p | 279 | 15.261 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| d9800 | native | 400x256p | 279 | 15.261 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| ms929 | native | 400x256p | 283 | 15.480 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 256p | ok |
| ms2930 | native | 400x256p | 283 | 15.480 | 54.701 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| r666b | native | 400x256p | 283 | 15.480 | 54.701 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pstar | native | 400x256p | 288 | 15.753 | 54.700 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| arcade_25 | native | 625x400p | 456 | 24.943 | 54.700 | exact | 1 | nearest | 64.0 % | 144 | +0.0 % | 1x 256p | ok |
| arcade_31 | 2x | 400x512p | 575 | 31.452 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pc_31_120 | 2x | 400x512p | 575 | 31.452 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| pc_70_120 | 2x | 400x512p | 549 | 30.029 | 54.699 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| m2929 | 2x | 400x512p | 549 | 30.030 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| edid_31 | 2x | 400x512p | 567 | 31.014 | 54.699 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 512p | ok |
| edid_30_54 | 2x | 400x512p | 549 | 30.030 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 768p | ok |
| edid_30_70 | 2x | 400x512p | 549 | 30.030 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| edid_30_96 | 2x | 400x512p | 549 | 30.030 | 54.700 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1024p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +9.69 % | 2 | nearest | 71.1 % | 208 | -0.1 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +9.31 % | 3 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 56.250 | 50.000 | -8.59 % | 4.22 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +9.60 % | 5.62 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +9.69 % | 8.44 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +9.79 % | 1 | sharp | 100.0 % | 0 | -6.2 % | - | ok: overscan crop |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -8.45 % | 1 | sharp | 88.8 % | 32 | +0.1 % | - | ok |

### `sfiii3ws`: 496x224 frame, R0, 59.590 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15ex | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| h9110 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| polo | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7131 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| k7000 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ntsc | native | 496x224p | 262 | 15.704 | 59.940 | nearest in range +0.59 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| pal | native | 496x224p | 312 | 15.600 | 50.000 | nearest in range -16.09 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| arcade_15_25_31 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_15_31 | native | 496x224p | 263 | 15.672 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| m3129 | native | 496x224p | 256 | 15.255 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9200 | native | 496x224p | 256 | 15.255 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9400 | native | 496x224p | 256 | 15.255 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| d9800 | native | 496x224p | 256 | 15.255 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| ms929 | native | 496x224p | 260 | 15.493 | 59.589 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 224p | ok |
| ms2930 | native | 496x224p | 260 | 15.493 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| r666b | native | 496x224p | 260 | 15.493 | 59.591 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pstar | native | 496x224p | 264 | 15.731 | 59.590 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| arcade_25 | native | 855x386p | 418 | 24.908 | 59.590 | exact | 1 | nearest | 58.0 % | 162 | +0.0 % | 1x 224p | ok |
| arcade_31 | 2x | 496x448p | 527 | 31.404 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_31_120 | 2x | 496x448p | 527 | 31.403 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| pc_70_120 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| m2929 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 636p | ok |
| edid_31 | 2x | 496x448p | 521 | 31.046 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 448p | ok |
| edid_30_54 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 872p | ok |
| edid_30_70 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| edid_30_96 | 2x | 496x448p | 504 | 30.033 | 59.590 | exact | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 896p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.69 % | 3 | nearest | 93.3 % | 48 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +0.34 % | 3 | nearest | 87.5 % | 96 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.69 % | 4.82 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +0.61 % | 6.43 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.69 % | 9.64 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | native | 720x240p | 262 | 15.734 | 60.054 | +0.78 % | 1 | sharp | 93.3 % | 16 | +0.0 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -15.96 % | 1 | sharp | 77.7 % | 64 | -0.1 % | - | ok |

### `bnzabros`: 496x384 frame, R0, 57.524 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| arcade_15 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| arcade_15ex | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| h9110 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| polo | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| k7131 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| k7000 | interlaced 1:1 | 579x448i | 545 | 15.675 | 57.524 | exact | 1 | nearest | 85.7 % | 64 | +0.0 % | 1x 384i | ok |
| ntsc | interlaced 1:1 | 579x448i | 525 | 15.734 | 59.940 | nearest in range +4.20 % | 1 | nearest | 85.7 % | 64 | +0.0 % | none | ok |
| pal | interlaced 1:1 | 579x448i | 625 | 15.625 | 50.000 | nearest in range -13.08 % | 1 | nearest | 85.7 % | 64 | +0.0 % | none | ok |
| arcade_15_25 | native | 496x384p | 433 | 24.907 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| arcade_15_25_31 | native | 496x384p | 433 | 24.907 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| arcade_15_31 | native | 647x501p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 76.6 % | 117 | +0.0 % | 1x 384p | ok |
| m3129 | native | 638x494p | 539 | 31.005 | 57.523 | exact | 1 | nearest | 77.7 % | 110 | +0.0 % | 1x 384p | ok |
| d9200 | native | 638x494p | 539 | 31.005 | 57.523 | exact | 1 | nearest | 77.7 % | 110 | +0.0 % | 1x 384p | ok |
| d9400 | native | 496x384p | 425 | 24.447 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| d9800 | native | 496x384p | 425 | 24.447 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| ms929 | native | 496x384p | 425 | 24.447 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| ms2930 | native | 496x384p | 427 | 24.562 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| r666b | native | 496x384p | 427 | 24.562 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| pstar | native | 496x384p | 440 | 25.310 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| arcade_25 | native | 496x384p | 433 | 24.907 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| arcade_31 | native | 647x501p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 76.6 % | 117 | +0.0 % | 1x 384p | ok |
| pc_31_120 | native | 661x512p | 546 | 31.408 | 57.524 | exact | 1 | nearest | 75.0 % | 128 | +0.1 % | 1x 384p | ok |
| pc_70_120 | native | 646x500p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 76.8 % | 116 | +0.0 % | 2x 768p | ok |
| m2929 | native | 496x384p | 522 | 30.028 | 57.525 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 1x 384p | ok |
| edid_31 | native | 672x520p | 539 | 31.005 | 57.525 | exact | 1 | nearest | 73.8 % | 136 | +0.0 % | 1x 384p | ok |
| edid_30_54 | native | 496x384p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 768p | ok |
| edid_30_70 | native | 496x384p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 3x 1152p | ok |
| edid_30_96 | native | 496x384p | 522 | 30.027 | 57.524 | exact | 1 | nearest | 100.0 % | 0 | +0.0 % | 4x 1536p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +4.30 % | 1 | nearest | 53.3 % | 336 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | +3.94 % | 2 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +4.30 % | 2 | sharp | 71.1 % | 312 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | +4.22 % | 3 | sharp | 80.0 % | 288 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +4.30 % | 5.62 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | +4.20 % | 1 | sharp | 80.0 % | 96 | +0.0 % | - | ok |
| tv_pal | interlaced 1:1 | 720x576i | 625 | 15.625 | 50.000 | -13.08 % | 1 | sharp | 66.6 % | 192 | +0.0 % | - | ok |

### `littlerb`: 338x288 frame, R0, 30.000 Hz

| Display | Rung | Mode | Total | kHz | Hz | Rate | Scale | Filter | Height | Border | Shape | Best whole | Rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| generic_15 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| arcade_15 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| arcade_15ex | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| h9110 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| polo | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| k7131 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| k7000 | interlaced 1:1 | 526x448i | 523 | 15.690 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| ntsc | interlaced 1:1 | 526x448i | 525 | 15.734 | 59.940 | nearest in range -0.10 % | 1 | nearest | 64.2 % | 160 | +0.0 % | none | ok |
| pal | native | 338x288p | 312 | 15.600 | 50.000 | nearest in range -16.67 % | 1 | nearest | 100.0 % | 0 | +0.0 % | none | ok |
| arcade_15_25 | native | 451x384p | 416 | 24.959 | 59.999 | exact x2 | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_15_25_31 | native | 451x384p | 416 | 24.959 | 59.999 | exact x2 | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_15_31 | native | 562x479p | 524 | 31.440 | 60.000 | exact x2 | 1 | nearest | 60.1 % | 191 | +0.1 % | 1x 288p | ok |
| m3129 | native | 554x472p | 517 | 31.020 | 60.000 | exact x2 | 1 | nearest | 61.0 % | 184 | +0.0 % | 1x 288p | ok |
| d9200 | 2x | 338x576p | 617 | 37.020 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 576p | ok |
| d9400 | native | 338x288p | 312 | 18.719 | 60.000 | exact x2 | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 576p | ok |
| d9800 | native | 338x288p | 312 | 18.719 | 60.000 | exact x2 | 1 | nearest | 100.0 % | 0 | +0.0 % | 2x 576p | ok |
| ms929 | interlaced 1:1 | 526x448i | 517 | 15.510 | 60.000 | exact x2 | 1 | nearest | 64.2 % | 160 | +0.0 % | 1x 288i | ok |
| ms2930 | native | 563x480p | 528 | 31.679 | 60.000 | exact x2 | 1 | nearest | 60.0 % | 192 | +0.1 % | 1x 288p | ok |
| r666b | native | 552x470p | 517 | 31.019 | 59.999 | exact x2 | 1 | nearest | 61.2 % | 182 | +0.0 % | 1x 288p | ok |
| pstar | native | 470x400p | 422 | 25.320 | 60.000 | exact x2 | 1 | nearest | 72.0 % | 112 | -0.1 % | 1x 288p | ok |
| arcade_25 | native | 451x384p | 416 | 24.959 | 59.999 | exact x2 | 1 | nearest | 75.0 % | 96 | +0.0 % | 1x 288p | ok |
| arcade_31 | native | 562x479p | 524 | 31.440 | 60.000 | exact x2 | 1 | nearest | 60.1 % | 191 | +0.1 % | 1x 288p | ok |
| pc_31_120 | native | 594x506p | 524 | 31.440 | 60.000 | exact x2 | 1 | nearest | 56.9 % | 218 | +0.0 % | 1x 288p | ok |
| pc_70_120 | 2x | 338x576p | 602 | 36.120 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 864p | ok |
| m2929 | 2x | 338x576p | 597 | 35.820 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 2x 576p | ok |
| edid_31 | native | 585x498p | 517 | 31.020 | 60.000 | exact x2 | 1 | nearest | 57.8 % | 210 | -0.1 % | 1x 288p | ok |
| edid_30_54 | 2x | 338x576p | 599 | 35.939 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 3x 864p | ok |
| edid_30_70 | 2x | 338x576p | 599 | 35.939 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1120p | ok |
| edid_30_96 | 2x | 338x576p | 599 | 35.939 | 60.000 | exact x2 | 2 | nearest | 100.0 % | 0 | +0.0 % | 4x 1152p | ok |
| panel_720 | panel | 1280x720p | 750 | 45.000 | 60.000 | +0.00 % | 2 | sharp | 80.0 % | 144 | +0.0 % | - | ok |
| panel_768 | panel | 1366x768p | 798 | 47.712 | 59.790 | -0.35 % | 2 | sharp | 75.0 % | 192 | +0.0 % | - | ok |
| panel_1080 | panel | 1920x1080p | 1125 | 67.500 | 60.000 | +0.00 % | 3 | sharp | 80.0 % | 216 | +0.0 % | - | ok |
| panel_1440 | panel | 2560x1440p | 1481 | 88.786 | 59.951 | -0.08 % | 5 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| panel_2160 | panel | 3840x2160p | 2250 | 135.000 | 60.000 | +0.00 % | 7.50 | sharp | 100.0 % | 0 | +0.0 % | - | ok |
| tv_ntsc | interlaced 1:1 | 720x480i | 525 | 15.734 | 59.940 | -0.10 % | 1 | sharp | 60.0 % | 192 | -0.1 % | - | ok |
| tv_pal | native | 720x288p | 312 | 15.625 | 50.080 | -16.53 % | 1 | sharp | 100.0 % | 0 | +0.0 % | - | ok |

