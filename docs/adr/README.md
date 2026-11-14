# Architecture decision records

An ADR is a short, numbered file in this directory. It records a decision that changes the design (run rule 18):

- a change to a contract,
- a new crate,
- a change to a dependency rule,
- a licence decision,
- a change to a safety rule.

A routine choice goes in the commit body or in the crate README, not in an ADR.

To write one, copy `0000-template.md` to the next free number. A later change that amends a decision adds a line to the Status of the ADR; it does not rewrite the decision.

## Index

| Number | Decision | Status | Task |
| --- | --- | --- | --- |
| [0001](0001-contract-corrections-in-code.md) | Corrections to the contracts block when it became code | accepted | T02 |
| [0002](0002-switchres-link-licence.md) | Licence of the link with Switchres | accepted | T05 |
| [0003](0003-line-rate-tolerance-follows-switchres.md) | The lowest line rate of a range follows Switchres | accepted | T04 |
| [0004](0004-font-licence.md) | The UI font is Spleen under BSD-2-Clause | accepted | T07 |
| [0005](0005-ui-destination-in-scene.md) | The scene carries the UI destination rectangle | accepted, amended by 0012 | C01 |
| [0006](0006-scanlines-in-scene.md) | The scene carries the scanlines of the game picture | accepted | C04 |
| [0007](0007-bounded-flip-wait-and-blank.md) | The flip wait is bounded, and a display can turn its output off | accepted | C07 |
| 0008 | Not used | | |
| [0009](0009-pixel-clock-in-whole-khz.md) | A mode for a display has a pixel clock in whole kHz | accepted | C09 |
| [0010](0010-output-kind-picks-the-gpu-limits.md) | The output kind picks the GPU limits | accepted | C08 |
| [0011](0011-audio-outputs-trait-and-text-setting.md) | An audio outputs trait, and a text kind of setting | accepted | C12 |
| [0012](0012-tv-standard-is-a-monitor-kind.md) | A TV standard is a monitor kind with a fixed mode table | accepted, amended by P03, P05, P06 | P02 |
