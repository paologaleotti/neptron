## Code standards

These rules are binding for every task. The reviewer checks them. They go into `CLAUDE.md`.

### Errors

- Each crate or module owns its error enum, made with `thiserror`. There is no global error type.
- An error carries its context: the value, the limit, the path. Use `#[from]` only when the conversion has one meaning; in other cases use `map_err` and add context.
- Library crates do not call `unwrap`, `expect` or `panic!`. The only exception is a proven invariant, with a comment that gives the proof.
- No `let _ =` on a `Result`, unless a comment says why the error does not matter.
- A constructor that can fail returns `Result`. `new()` is for creation that cannot fail.
- No panic path in an `extern "C"` callback.

### Structure and idiom

- Decide in pure code, execute in the backend. The policy, the validator and the UI make every decision and do no I/O. A backend executes what it gets and makes no decision.
- One code path for preview and for run. The display panel shows a mode from the same policy function that the engine uses.
- Types of external libraries stay in their backend crate, behind the traits: `drm`, `gbm`, `glow`, `sdl3`, `libloading`, `evdev`.
- In each file, public items come first. Every public item has a `///` comment.
- No `mod.rs`. A module root re-exports the public types of its private files, so callers import from one level.
- Split a large module by concern, as `impl` blocks in child files. Do not make a file for two methods.
- Signatures that only read take `&[T]` and `&str`.
- A function has one responsibility. A function that calls a sequence of such functions is correct.
- All external dependencies are in the root `[workspace.dependencies]`, one version each.
- A format on disk or on the wire is append-only: settings keys, control protocol messages, cache headers. Do not rename or remove a field; add a version.

### Comments

- The default is no comment. A comment gives a reason that the code cannot show: a constraint, an order dependency, an external problem.
- A comment describes the code as it is now. It does not tell history.
- No comment that only points to a document section or a task number. No banner comments.

### Concurrency

- No async runtime. The frame thread does no file I/O, takes no lock that a different thread can hold for a long time, and never blocks on a channel.
- Threads communicate through bounded, one-direction channels. A full channel does not stop the frame thread: the sender drops or merges the message.
- The audio callback is real-time code: no lock, no allocation, no I/O. It reads from a lock-free ring buffer.
- No lock guard stays held across a channel receive, a call into a core, or a DRM commit.
- No allocation in the steady-state frame loop. Buffers are made one time, at launch or at a mode change.
- Long work (ROM scan, cache build) checks for cancellation between work units.
- Each thread has an owner that stops it and joins it at shutdown. There are no detached threads.

### Unsafe

- `unsafe` is only in the crates of the allow-list, and there in one module (`ffi.rs` or equivalent) with a safe wrapper around it.
- Every `unsafe` block has a `// SAFETY:` comment that states the invariant. `unsafe_op_in_unsafe_fn` is denied.
- A pointer or a length from a core is checked (null, size, alignment) before use. A file descriptor has one owner (`OwnedFd`).

### User data

- NEPTRON never changes or deletes a ROM or a BIOS file.
- Settings, save RAM and save states are written as temp file, fsync, rename. A crash leaves the old file or the new file, never a broken file. Save RAM keeps one `.bak`.
- A cache can be deleted at any time with no loss of user data. A damaged cache is rebuilt, and it never stops the boot.

### Tests and reports

- Pure crates are tested with no mocks. Tests cover the error paths, not only the correct path.
- A task report says what was done, why, what to watch, and which tests exist.
