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

## Stage 1 — Rust core behind the existing app (half a day) — **done, gate passed**

Compile `core/` to WebAssembly and have the current TypeScript call it. `src/tzx/parser.ts`
becomes a thin wrapper over the wasm export, with the same signature, so nothing above it
changes. Both the web build and the Tauri build get it.

Done on 2026-09-18. `src/tzx/parser.ts` is now 40 lines: `isTzx` and `bitsPerSymbol` (still
needed by the writer, the audio renderer and the consistency check, and moving with them in
stage 2) plus three calls into the core. The old implementation is frozen as
`test/reference/parser.ts`, the way the audio tests keep the original sample sink, and
`test/core.test.ts` runs the two against each other: the sample tapes, every creatable block
type, Latin-1 text and entries, select/call blocks, TAP edge cases and eight damaged TZX files.
61 TypeScript tests and 13 Rust tests pass, and `npm run smoke` drives the real UI on tapes
parsed by the core.

**No wasm-bindgen.** The interface is a byte buffer in and a byte buffer out, so the module loads
with a plain `WebAssembly.instantiate` and needs no generated glue and no extra build tool:

- `core/src/wasm.rs` exports `core_alloc`, `core_free`, `core_parse_tape`, `core_parse_tzx`,
  `core_parse_tap` and `core_wire_version` over the C ABI. JavaScript owns both buffers.
- `core/src/wire.rs` encodes a parsed tape as a flat little-endian byte format and
  `src/tzx/wire.ts` decodes it into the same block objects as before, key order included.
  Both ends are about 150 lines and version-checked against each other at startup.
- `scripts/build-wasm.mjs` builds the crate and writes `src/tzx/core.wasm.ts`, the module
  base64-inlined. `predev`, `prebuild`, `pretest` and `pretypecheck` run it, and the file is
  generated, not tracked.

Measured on 2026-09-18:

| | before | after |
|---|---|---|
| Main JS bundle | 187.56 kB (65.12 kB gzip) | 253.20 kB (92.71 kB gzip) |
| wasm module | — | 49,438 bytes (19.1 kB gzipped on its own) |
| Instantiate at startup | — | 0.2 ms |
| Parse of the 7.6 kB demo tape | — | 0.02 ms |

**Gate answers.** The bundle grew by 66 kB raw and 28 kB gzip — a sixth of the 200–400 kB the
plan budgeted, because there is no bindgen glue and the release profile is built for size
(`opt-level = "z"`, LTO, one codegen unit, `panic = "abort"`). Startup gains one 0.2 ms
instantiation against a 355 ms baseline, so the 355 ms stands; the desktop cold start is worth
one confirming run on the real app. **Continue.**

Four things worth knowing before stage 2:

1. Compiling wasm is asynchronous, but the app parses synchronously. `initCore()` runs once
   before the first render (`src/main.tsx`) and in `test/setup.ts`; the parse functions throw a
   clear error if something calls them earlier. Top-level `await` would have been tidier but
   Safari 14.1 does not have it, so it is a callback.
2. Inlining the module as base64 costs about 8.5 kB gzip over serving a separate `.wasm` asset.
   It buys one code path for the browser, the Tauri webview and vitest under node, and no second
   request at startup. Revisit if the core grows by an order of magnitude.
3. The Rust differential fixtures are now dumped from `test/reference/parser.ts`, not from
   `src/tzx/parser.ts` — otherwise the core would be checking itself.
4. The toolchain note in CLAUDE.md was wrong: rustup *is* installed, at `~/.cargo/bin`, with the
   wasm32 target; it is only missing from `PATH`, where Homebrew's cargo (host target only) wins.
   `scripts/build-wasm.mjs` prefers the rustup shim, and both CI workflows now install the target.

## Stage 2 — The rest of the logic (1–2 days) — **in progress**

Same pattern, module by module, each with its tests ported and running against both
implementations before the TypeScript goes away:

1. ~~`writer.ts`~~ — **done** 2026-09-18. `core/src/writer.rs` writes TZX and TAP, decides the
   required version and serializes single blocks; `src/tzx/writer.ts` is 45 lines of signature
   over it, and the old implementation is frozen as `test/reference/writer.ts`. The wire format
   now works both ways (`encode_blocks`/`decode_blocks` in Rust, `encodeBlocks` in TypeScript),
   which is what every later module needs to receive a tape. Round trips, the version rules, the
   TAP export and the cases the writer papers over (over-long text, short idents, odd glue,
   symbol pulses shorter than `npp`) are checked in both suites: 66 TypeScript tests, 19 Rust.
2. ~~`describe.ts`, `content.ts`, `consistency.ts`, `programs.ts`~~ — **done** 2026-09-18.
   `core/src/{describe,content,consistency,programs}.rs` hold the list descriptions and ROM
   headers, the content detection, the consistency checks and the program/group structure; the
   four TypeScript modules are signatures over them, with the old implementations frozen under
   `test/reference/`. 75 TypeScript tests and 27 Rust ones, the differential ones comparing every
   block of every sample tape, the awkward content cases, twenty consistency tapes and ten
   program layouts
3. ~~`compare.ts`, `convert.ts`, `pokes.ts`, `bits.ts`~~ — **done** 2026-09-18.
   `core/src/{compare,convert,pokes,bits}.rs`, with `create_body` (the port of `createBlock`)
   added to `types.rs` because the type conversion needs it. The TypeScript's duck-typed field
   copying became an explicit carry-over table, and the POKEs line grammar, one regular
   expression there, is read by hand here — the crate still has no dependencies. 82 TypeScript
   tests and 32 Rust ones; the differential test converts between every pair of block types,
   compares every tape pair in all nine mode combinations, and parses a corpus of POKEs text
   including the lines that must fail
4. ~~`spectrum/basic.ts`, `screen.ts`, `z80dis.ts`, `charset.ts`~~ — **done** 2026-09-18.
   `core/src/spectrum/` holds all four. The differential test walks every opcode under every
   prefix (about 3,000 instructions, both number bases, labels on and off), every one of the 256
   characters, seven screens against every combination of the view options, and a BASIC program
   with hidden numbers and control codes under five option sets. It found three real differences
   on the way: wire strings were Latin-1, which mangled the block graphics; Rust rounds a tie to
   the even digit where JavaScript rounds it up, which `toPrecision(8)` needs; and `Infinity`
   panicked the core, which in wasm means a trap, not an exception
5. `audio.ts` last — the preallocated-buffer behaviour and `playbackTimeline` are subtle, and the
   existing tests compare against the previous algorithm bit-for-bit

**Ships after each module.** At the end, all 3,130 lines of logic are Rust, proven in production
through the existing app, and the browser version still works.

**What stays in TypeScript until stage 4:** predicates and slices over the block model that the
UI asks for per row and that carry no logic — `isMetadata`, `blockBody`, `payload`, `totalBits`,
and the `isDataBlock`/`hasData` pair that was always in `types.ts`. Crossing into wasm to drop two bytes
costs more than it saves. The core has its own copy of each, and both sides assert the same table
(`core/tests/logic.rs` and `test/core.test.ts`), so the copies cannot drift apart.

**Bundle, and it needs attention:** 187.56 kB before stage 1, then 253.20 after the parser,
335.10 after the writer and module 2, 372.00 after module 3, and **480.21 kB after the Spectrum
side** (182.03 kB gzipped). The wasm itself is 236 kB. That is 293 kB of growth with only the
audio module left, against the 200–400 kB the plan budgeted for the whole of stages 1 and 2, so
this will land at the top of the range or just past it.

Where it goes is measurable rather than mysterious: stubbing out the `toPrecision(8)`
reimplementation alone takes 32 kB off, because it pulls in Rust's float-to-decimal machinery.
That one is worth its size — it is what makes the BASIC listing show the same numbers as before —
but the total deserves a proper look when the stage closes. Three things to try, cheapest first:
`wasm-opt -Oz` from binaryen (not installed here; usually 10–20%), a `panic_immediate_abort`
build of the standard library (needs nightly, which rustup here has, and would make the wasm
build depend on it), and serving the module as a separate compressed asset instead of base64
inside the bundle, which would give back the 33% the encoding costs.

**What the boundary costs, measured on a 3,000-block, 1.2 MB tape** (the size the plan measures
rendering with): the app encodes the blocks onto the wire for every call, so calls that the UI
makes per row are the ones to watch.

| | TypeScript | Rust core, cold | Rust core, asked again |
|---|---|---|---|
| `serializeTzx` of the whole tape | 4.7 ms | 7.4 ms | — |
| `requiredVersion` (pane header) | 0.1 ms | 1.6 ms | — |
| `describeBlock` + `blockLength` × 3000 | 1.9 ms | 14.7 ms | 0.3 ms |
| content labels for the list | 0.8 ms | 7.4 ms | 0.0 ms |
| `checkConsistency` | 1.9 ms | 9.5 ms | 0.0 ms |
| `detectPrograms` | 0.6 ms | 8.3 ms | 0.0 ms |
| `groupRanges` | 0.1 ms | 1.5 ms | 0.0 ms |

Three things keep that honest:

- **Ask once per list, not once per row.** `contentLabels(blocks)` replaced 3,000 calls to
  `detectContent` in `TapePane.tsx`.
- **Cache on identity.** Blocks and the arrays holding them are immutable, so `src/tzx/cache.ts`
  memoizes per-tape answers on a `WeakMap` keyed by the array, and descriptions on the block
  object. Re-renders — the measured 76–90 ms cursor move — now cost nothing at all where the
  TypeScript recomputed every time. Only the first look at a changed tape pays.
- **Leave out what the call does not read.** `requiredVersion` and `saveVersion` get a payload
  with the byte data stripped, which took them from 8.5 ms to 1.6 ms. The differential tests give
  the reference the real data, so a core that started reading what it is not sent would fail.

The remaining cost is the first call after a tape changes: on a 3,000-block, 1.2 MB tape that is
about 40 ms of encoding spread over the list, the checks and the program list. The real answer is
stage 4, where the tape stops crossing the boundary because it lives in Rust.

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
