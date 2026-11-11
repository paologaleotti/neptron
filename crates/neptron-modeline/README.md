# neptron-modeline

Safe Switchres wrapper, monitor profiles and the ODREN policy: from source geometry, profile and GPU capabilities to a list of candidate modelines. No I/O.

## Public API sketch

- `fn modelines(&SourceGeometry, &MonitorProfile, &GpuCaps) -> Vec<Candidate>`
- `policy` module: the decision procedure, each candidate with its reason

## Must never depend on

- Any NEPTRON crate except `neptron-types` and `neptron-switchres-sys` (normal dependencies)
- Any hardware crate
- I/O of any kind
- `unsafe`: the raw calls live in `neptron-switchres-sys`

## Rules

- `unsafe` is forbidden: the crate root carries `#![forbid(unsafe_code)]`.
- No features.

## Status

Stub.
