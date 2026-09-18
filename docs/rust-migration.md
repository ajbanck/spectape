# Migrating SpecTape to a native UI, in stages

Goal: a desktop app that runs with no external dependencies — no WebView2 on Windows, no system
WebKit on macOS, no WebKitGTK on Linux — at native startup and list-scrolling speed.

Target stack: **Rust core + Slint UI**, one static binary per platform (15–25 MB, ~100 ms cold
start, measured baselines in "Where we start" below). egui is the fallback if Slint's widgets
disappoint; the core work is identical either way, which is why the UI choice is deferred to
stage 3 rather than decided up front.

The plan is staged so that **the app stays shippable after every stage**, and so that the
expensive decision (rewriting 2,223 lines of UI) is taken only after the cheap half has proven
the conversion rate.

## Where we start

Measured on 2026-09-17, warm launch, release build:

| | today |
|---|---|
| Cold start (window drawn) | 355 ms, of which ~150 ms is WKWebView creation |
| Cursor move, 200-block tape | at the frame floor, nothing to fix |
| Cursor move, 3000-block tape | 76–90 ms, ~90% our own render |
| Desktop download | 3.6 MB macOS, 1.3 MB Windows, 76 MB Linux AppImage |
| Runtime dependency | system webview on all three platforms |

Line counts to port:

| Area | Lines | Stage |
|---|---|---|
| `src/tzx` — parser, writer, audio, compare, consistency, content, programs | 2,545 | 1–2 |
| `src/spectrum` — BASIC lister, screen renderer, Z80 disassembler | 585 | 2 |
| `test/` — round trips, flow, audio, disassembler, BASIC, detection | 633 | 1–2 |
| `src/state` — signals, undo, selection | 1,038 | 4 |
| `src/ui` — panes, block list, editor, data window, dialogs | 2,223 | 4 |
| `src/platform` | 218 | 5 |
| `src-tauri/src` — emulator launch survives, the rest retires | 635 | 5 |

## Stage 0 — Spike (an hour, throwaway allowed) — **done, gate passed**

Port `src/tzx/types.ts` and `parser.ts` to a new `core/` crate. Feed it the same bytes as the
parser tests in `test/tzx.test.ts` and assert the same block structures.

Done on 2026-09-18 on the `rust-migration` branch, and kept rather than thrown away:

| | |
|---|---|
| Ported | `bytes.ts` `Reader`, the block model of `types.ts`, all of `parser.ts` |
| TypeScript in | ~745 lines (parser 279, model 416 of which the UI tables were skipped, reader ~50) |
| Rust out | 580 lines of crate (of which 175 are the dump harness below) plus 256 lines of tests |
| Result | `cargo test`: 11 tests green, no clippy warnings |

The tests are the TypeScript ones ported by hand (unknown blocks preserved, deprecated blocks kept
raw, truncated block warning, TAP parsing and its two warnings, generalized blocks, uids) **plus a
differential harness**, which is the part worth keeping:

- `core/src/dump.rs` prints a parsed tape as one canonical line per block.
- `scripts/dump-blocks.mjs` prints the same format from the TypeScript parser
  (`npm run core:fixtures`) into `core/tests/fixtures/*.dump`.
- `npm run core:test` fails with a line-level diff if the two parsers disagree anywhere.

The three sample tapes cover all 25 creatable block types, 36 blocks in total, and both parsers
agree on every field, byte for byte. Checked by deliberately shifting a pause by one: the harness
names the file, the line and the field.

**Gate answers.** It took a single session with no reworking, and the tests passed as soon as they
compiled. About 0.9 lines of Rust per line of TypeScript, and the conversion was mechanical — the
data layer has no DOM, no async and no cleverness, so nothing above needed redesigning.
Multiplied across 3,130 lines that is credible as the 1–2 days stage 2 assumes. **Continue.**

Three things learned that change the later stages:

1. `uid` moved off the block: `Block { uid, body }` with the ID implied by the `Body` variant.
   The `UnknownBlock.id` narrowing trap from CLAUDE.md cannot happen in Rust, and every `match`
   over block types is exhaustive — the compiler will list what a new block type still needs.
2. One real bug surfaced: a CSW block whose declared length is under its 10-byte header made
   `parser.ts` read `r.bytes(len - 10)` with a negative count, which walked the read position
   *backwards* and parsed the same bytes again. Both implementations now take the normal warning
   path, with the same message, and keep the remaining bytes verbatim — fixed in `parser.ts`
   first, so the two stay identical.
3. TZX text is Latin-1 and `parser.ts` keeps it as a JS string of byte values. Rust keeps a
   `String` with a one-byte-one-char conversion in `bytes.rs`, which is lossless, but every
   string that reaches a UI or a file has to go back through `string_to_latin1`. Worth watching in
   stage 2, where `describe.ts` and the BASIC lister live.

## Stage 1 — Rust core behind the existing app (half a day)

Compile `core/` to WebAssembly and have the current TypeScript call it. `src/tzx/parser.ts`
becomes a thin wrapper over the wasm export, with the same signature, so nothing above it
changes. Both the web build and the Tauri build get it.

Run old and new implementations side by side in the test suite (differential testing: same input,
assert identical output) until they agree on every fixture, then delete the TypeScript body.

**Ships.** The app is unchanged for users; the parser is now Rust.

**Gate:** wasm adds 200–400 KB to the web bundle. Acceptable? Startup must not regress past the
355 ms baseline.

## Stage 2 — The rest of the logic (1–2 days)

Same pattern, module by module, each with its tests ported and running against both
implementations before the TypeScript goes away:

1. `writer.ts` — round-trip tests already exist and are the strongest safety net here
2. `describe.ts`, `content.ts`, `consistency.ts`, `programs.ts` — pure, well covered
3. `compare.ts`, `convert.ts`, `pokes.ts`, `bits.ts`
4. `spectrum/basic.ts`, `screen.ts`, `disasm.ts`, `charset.ts`
5. `audio.ts` last — the preallocated-buffer behaviour and `playbackTimeline` are subtle, and the
   existing tests compare against the previous algorithm bit-for-bit

**Ships after each module.** At the end, all 3,130 lines of logic are Rust, proven in production
through the existing app, and the browser version still works.

## Stage 3 — Native shell skeleton (half a day)

A second binary, `spectape-native`, linking `core/` directly (no wasm). Window, native menu built
from the same command table, and a read-only block list showing a loaded tape.

Not shipped. Developed alongside the real app.

**Gate — this is the decision point.** Does the Slint list handle 3,000 rows smoothly? Does the
menu feel right on all three platforms? If not, swap to egui or Qt here, having lost an afternoon. Measure cold start now: it should be near 100 ms, or the premise is wrong.

## Stage 4 — Feature parity, area by area (2–4 days of writing, plus your testing)

Port `src/state` into Rust as you go; each area is done when it matches the current app:

1. Block list: selection semantics, collapsed groups, drag & drop, cursor
2. Block editor: per-type forms, commit/revert
3. Data window: hex, screen, BASIC, vars, text, disassembly
4. Dialogs, status bar, options
5. Playback: audio out via `cpal`, progress indicator
6. Files: open, save, save-as, associations, "open with", the emulator launch that already exists
   in `src-tauri/src/emulator.rs`

Keep a parity checklist against `src/state/commands.ts` — it is the complete list of what the app
does, which makes "are we done" answerable rather than a feeling.

**Not shipped until the checklist is complete.** This is the only long stretch without a release;
it is unavoidable, because a half-ported UI is worse than either side.

## Stage 5 — Switch (a day, plus CI round trips)

The native binary becomes the desktop app. `src-tauri/` retires except for the emulator code.
Packaging changes to plain binaries plus a `.app`, `.msi` and an AppImage that no longer carries
WebKitGTK — the Linux download should fall from 76 MB to under 25 MB.

The web app keeps the existing TypeScript UI on the wasm core, and is maintained as its own thing.
If the browser version does not matter, delete `src/ui` here instead and the estimate shortens by
roughly a week of parallel-maintenance work.

## Stage 6 — Cleanup (an hour)

Delete the dead TypeScript, update CI to build and test the Rust binaries on all three platforms,
rewrite CLAUDE.md's architecture map, and re-measure the numbers in "Where we start".

## Total

**Roughly a week of writing**, with shippable states throughout except during stage 4. The two
gates (stage 0 and stage 3) cost an afternoon between them and can each cancel the project before
the expensive part starts.

## What actually costs time

Not the typing. This application was written in a day, and a port has no design left to invent:
the reference implementation and its tests are right here. What does not speed up:

- **Verification loops.** Only you can run the desktop app and say whether it feels right. Every
  round of "build, look, report" is a turnaround, and this session spent most of its length on
  exactly that for two small startup bugs.
- **Other platforms.** Windows and Linux builds are verified in CI or on a real machine, minutes
  per cycle, and some things (file associations, native menus, the emulator launch) can only be
  judged by running them.
- **Details nobody wrote down.** Selection semantics, collapsed groups, the editor footer, the
  flash-free start. They live in the current UI's behaviour and get rediscovered one at a time.
- **Audio and packaging.** `cpal` output correctness and code-signing are where estimates usually
  go wrong, because neither fails loudly in a test.

So: days of writing, and a calendar that depends on how quickly the two of us can round-trip.

## What can be done today instead, for a fraction of this

Virtualizing the block list — the one measured performance problem — is an afternoon in the
current codebase (`src/ui/TapePane.tsx`, following the pattern already in `DataWindow.tsx:198`).
It does not address the dependency requirement, which is the actual reason for this plan.
