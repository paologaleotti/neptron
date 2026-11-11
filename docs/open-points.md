# Open points

A question that the documents do not answer becomes an open point (run rule 8). The task takes a safe default, records it in its task report, and continues. The orchestrator copies it here.

A `TODO` in the code names the id of its open point, for example `TODO(OP-003)`.

## Format

Each open point is one section:

```markdown
### OP-NNN: <the question, in one line>

- Raised by: <task id>
- Status: open | decided | closed
- Default: <the safe default that the code uses now>
- Where: <files or modules that hold the default>
- How to change: <what to edit when the decision changes>
- Decision: <empty while open; the decision and a link to its ADR>
```

## List

### OP-001: Which dependency kinds do the modeline and safety dependency rules restrict?

- Raised by: T01
- Status: open
- Default: normal dependencies only. Build dependencies (`cc`) and test dependencies stay allowed.
- Where: `xtask/src/rules.rs`
- How to change: add the other dependency kinds to the rule.
- Decision:

### OP-002: Which licences does `cargo-deny` allow, and do advisories run in T0?

- Raised by: T01
- Status: open
- Default: permissive licences only. The advisory check is not in T0, because it needs the network.
- Where: `deny.toml`, `xtask/src/verify.rs`
- How to change: edit the allow-list; add an advisory step to a tier that can use the network.
- Decision:

### OP-003: Does the macOS CI job run under the machine-load limits?

- Raised by: T01
- Status: open
- Default: yes. `xtask` cannot tell a CI runner from a local machine.
- Where: `.github/workflows/ci.yml`, `xtask/src/load.rs`
- How to change: set `NEPTRON_BUILD_SLOTS` and `NEPTRON_BUILD_JOBS` in the CI job.
- Decision:

### OP-004: Are the tier time budgets enforced?

- Raised by: T01
- Status: open
- Default: no. `verify` reports the time against the budget and does not fail on it.
- Where: `xtask/src/verify.rs`
- How to change: make a step over budget a failure.
- Decision:

### OP-005: The CI workflow was never run.

- Raised by: T01
- Status: open
- Default: the workflow is checked by inspection only. The first push of `main` runs it.
- Where: `.github/workflows/ci.yml`
- How to change: repair the workflow after the first run.
- Decision:

### OP-006: The forbid check can be fooled by a char literal inside an attribute.

- Raised by: T01 review
- Status: open
- Default: accepted. Only a file made on purpose can do this.
- Where: `xtask/src/rules.rs`, `bracket_body`
- How to change: handle char literals and comments inside attributes.
- Decision:

### OP-007: Can preset data derived from Switchres stay in the permissively licensed tree?

- Raised by: T02
- Status: open, needs the project owner
- Default: the 25 files in `data/monitors` stay, and `data/monitors/README.md` states the origin (Switchres, GPL-2.0-or-later, commit 09272ac).
- Where: `data/monitors`
- How to change: move the presets to a directory with its own GPL licence file, or write the values again from the monitor specifications.
- Decision:

### OP-008: The four `vesa_*` presets of Switchres are not converted.

- Raised by: T02
- Status: open
- Default: absent. Switchres computes them at run time with floating-point GTF maths, so there is no table to convert.
- Where: `data/monitors/README.md`
- How to change: T05 can produce them through Switchres.
- Decision:

### OP-009: `InputEvent` has no hotplug event and no device that is not a RetroPad.

- Raised by: T02
- Status: open
- Default: absent. A later addition does not change the existing wire format.
- Where: `crates/neptron-types/src/event.rs`
- How to change: add variants.
- Decision:
