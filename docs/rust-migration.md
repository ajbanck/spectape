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

## Where we start — and where it ended

Warm launch, release build, same machine: the Tauri app on 2026-09-17, the native one on
2026-09-18 (arm64, `SpecTape.app`).

| | Tauri | native |
|---|---|---|
| Cold start (window drawn) | 355 ms, of which ~150 ms is WKWebView creation | **161 ms** to the first frame (0.26 s for the whole process, `--exit-on-draw`) |
| Cursor move, 200-block tape | at the frame floor, nothing to fix | same |
| Cursor move, 3000-block tape | 76–90 ms, ~90% our own render | **0.27 ms median** (min 0.08, max 1.5) of our own frame build |
| Desktop download | 3.6 MB macOS, 1.3 MB Windows, 76 MB Linux AppImage | **7.1 MB** macOS dmg (universal), **5.6 MB** Linux AppImage, **7.3 MB** Windows exe (2.9 MB msi) |
| Runtime dependency | system webview on all three platforms | none |

Two things the numbers say that the plan guessed at:

- **Cold start is process start, window and GL context**, exactly as stage 4 predicted when it
  found the app's own first frame costs 4–6 ms headless. 161 ms is what is left after the web view
  goes; the remaining ~155 ms is dyld, `NSApplication` and the GL context, none of it ours. The
  target in the goal above was ~100 ms, so this lands near it without anything left in the repo to
  cut.
- **Linux is the only download that shrank**, and it is the one the exercise was about: 76 MB to
  5.6 MB, because the AppImage no longer carries WebKitGTK. macOS doubled (3.6 → 7.1 MB, and it is
  universal now) and Windows went from 1.3 MB to 7.3 MB, which is the trade: the UI is in the
  binary instead of borrowed from the system, and on Windows that is what removes the WebView2
  dependency altogether. Sizes from the first full packaging run, 2026-09-18 — each about 0.8 MB
  low now, because the release profile keeps the symbol table for the crash log (see "A panic, and
  where panics go now"). Re-measure on the next packaging run.
- **The list is no longer the frame.** What used to be 76–90 ms of our own rendering is 0.27 ms,
  because the rows are built once per version of the tape and the list is virtualised. During the
  bench the *whole* frame sits at ~12 ms, which is the display's pace with a repaint requested
  every frame, not work — with one outlier per run (62–155 ms) in the first frames, where the
  window and the font atlas are still arriving.

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

## Stage 2 — The rest of the logic (1–2 days) — **done**

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
5. ~~`audio.ts`~~ — **done** 2026-09-18. `core/src/audio.rs` holds the playback flow, the pulse
   emission, the sample rendering and the WAV encoding. The tests that already compared the
   renderer against the previous algorithm now compare it against the core: every pulse source at
   two sample rates in both modes, sample for sample, plus the pulse streams, the timelines and
   the WAV bytes including how JavaScript rounds a half. Two things stayed in TypeScript on
   purpose: inflating Z-RLE CSW blocks, because that needs zlib and the crate has no dependencies,
   so the app hands those blocks over already inflated; and `positionAt`, a binary search over an
   array the caller already holds, which the progress indicator runs every animation frame.
   `renderWav` was added so the WAV export renders and encodes in one call — the samples of a long
   tape are tens of megabytes and crossing twice with them would be silly

**Ships after each module.** At the end, all 3,130 lines of logic are Rust, proven in production
through the existing app, and the browser version still works.

### Where stage 2 landed

All five module groups are done. What is left in TypeScript under `src/tzx` and `src/spectrum` is
600 lines of signatures over the core, plus the plumbing: `core.ts` (the loader and one call per
entry point), `wire.ts` (the byte format), `cache.ts`, `bytes.ts` and the block model in
`types.ts`. The core is 6,600 lines of Rust with 45 tests of its own, and `test/reference/` holds
2,548 lines of frozen TypeScript that `test/core.test.ts` checks it against in 95 tests.

The differential tests earned their keep: they caught six real differences that would otherwise
have shipped — the CSW length bug, Latin-1 wire strings mangling the block graphics, JavaScript's
tie-breaking in `toPrecision`, a panic on `Infinity` (a trap, in wasm), POKE numbers truncating
early, and a truncated POKEs block failing differently.

**What stays in TypeScript until stage 4:** predicates and slices over the block model that the
UI asks for per row and that carry no logic — `isMetadata`, `blockBody`, `payload`, `totalBits`,
and the `isDataBlock`/`hasData` pair that was always in `types.ts`. Crossing into wasm to drop two bytes
costs more than it saves. The core has its own copy of each, and both sides assert the same table
(`core/tests/logic.rs` and `test/core.test.ts`), so the copies cannot drift apart.

### The bundle, and what to do about it

| after | bundle | gzipped |
|---|---|---|
| before stage 1 | 187.56 kB | 65.12 kB |
| the parser | 253.20 kB | 92.71 kB |
| the writer and module 2 | 335.10 kB | 122.76 kB |
| module 3 | 372.00 kB | 135.93 kB |
| the Spectrum side | 480.21 kB | 182.03 kB |
| **audio, end of stage 2** | **492.70 kB** | **186.10 kB** |

That is 305 kB of growth against the 200–400 kB the plan budgeted, so it landed inside the range
but near the top. Where it goes is measurable rather than mysterious: the wasm is 249 kB, of which
stubbing out the `toPrecision(8)` reimplementation alone takes 32 kB — that one earns its size,
because it is what makes the BASIC listing show the same numbers as before.

Three things to try, and what is known about each:

1. **`wasm-opt -Oz`** (binaryen). Usually 10–20%, one line in `scripts/build-wasm.mjs`. Not
   installed here; it would need `brew install binaryen` and a CI step.
2. **Serve the module as its own asset** instead of base64 inside the bundle. The wasm gzips to
   98 kB on its own, so this is the biggest single win available: the JS would fall to about
   160 kB and the total transfer to roughly 145 kB gzipped, against 186 kB now. The cost is the
   fetch-and-instantiate path that base64 was chosen to avoid — worth revisiting in stage 5,
   where the desktop app stops being a web bundle at all.
3. **A `panic_immediate_abort` build of the standard library.** Tried here: the nightly toolchain
   installed on this machine cannot build `-Z build-std` (its `rust-src` does not match the
   compiler). Worth another look on a machine where it does, but it would tie the wasm build to
   nightly.

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

## Stage 3 — Native shell skeleton (half a day) — **done, gate answered: egui**

A second binary, `spectape-native`, linking `core/` directly (no wasm). Window, native menu built
from the same command table, and a read-only block list showing a loaded tape.

Not shipped. Developed alongside the real app.

**Gate — this is the decision point.** Does the Slint list handle 3,000 rows smoothly? Does the
menu feel right on all three platforms? If not, swap to egui or Qt here, having lost an afternoon. Measure cold start now: it should be near 100 ms, or the premise is wrong.

### What stage 3 built

Done on 2026-09-18. `native/` is a **Slint 1.18** binary crate that links `spectape-core` as an
rlib: no wasm, no wire format, no web view. `npm run native` builds it and wraps it in a minimal
`SpecTape Native.app`, and prints how to run it and the three measuring commands. It is 1,066 lines
of Rust and Slint in total:

| | |
|---|---|
| `native/src/menutable.rs` | the command table: id, label, shortcut, when it is enabled |
| `native/build.rs` | includes that table and writes the `MenuBar { … }` markup into `ui/app.slint` |
| `native/ui/app.slint` | the window: tape header, block list, status bar, key handling |
| `native/src/main.rs` | loads a tape, builds the rows from the core, wires the menu, measures |
| `native/tests/menu.rs` | the menu against `src/state/commands.ts`: 4 tests, `npm run native:test` |

The list shows what the web one shows, out of the same core calls the app's TypeScript reaches
through `src/tzx/core.ts`: `parser::parse_tape`, `describe::describe_block` and `block_length`,
`content::content_labels`, the group/loop indent, and `consistency` and `programs` in the status
line. Three menu commands are live, the three that only read the tape — Tape Info, Programs and
Check Consistency report into the status bar; every other item reports its id there instead.

Four decisions worth keeping:

1. **`native/` is its own crate, not a workspace member with `core/`.** Cargo takes profiles from
   the workspace root only, and `core/Cargo.toml` owns the size-first profile (`opt-level = "z"`,
   LTO, `panic = "abort"`) that keeps the wasm module at 249 kB. A workspace would have silently
   ignored it. A path dependency costs nothing and keeps the two builds independent.
2. **The menu is generated, not written twice.** Slint's `MenuBar` has to be static markup inside
   the `Window` — it cannot be a component, and the dynamic menu interface (`MenuVTable`) is
   internal to `i-slint-core` — so `build.rs` writes the markup from `menutable.rs` and addresses
   each item by its flat index. Labels, shortcuts and enabled rules therefore still live in one
   Rust table, `enabled` is bound to a model the Rust side updates, and the tests fail if an id
   drifts from `commands.ts`.
3. **`@keys(Control + …)` is exactly SpecTape's `Mod`**: Slint maps `Control` to ⌘ on macOS and to
   Ctrl elsewhere, so one table gives both platforms the right accelerator. On macOS the menu is
   the real menu bar (Slint uses muda), and it adds the About/Services/Hide/Quit app menu itself —
   which is why the app needs a bundle: unbundled, that menu is named after the executable.
4. **The perf harness is in the binary.** `--measure` prints what the core costs in process and
   opens no window; `--exit-on-draw` quits on the first frame, so `time …` is the cold start;
   `--bench N` moves the cursor N times as fast as frames arrive and reports the distribution;
   `--rows N` repeats the sample tape's blocks to N rows.

### The three numbers

**The boundary is gone.** The same calls as the stage 2 table, on a 3,000-block, 1.2 MB tape (the
demo tape's blocks repeated), release build, in process:

| | TypeScript | wasm core, cold | native, in process |
|---|---|---|---|
| `describeBlock` + `blockLength` × 3000 | 1.9 ms | 14.7 ms | **0.9 ms** |
| content labels for the list | 0.8 ms | 7.4 ms | **0.2 ms** |
| `checkConsistency` | 1.9 ms | 9.5 ms | **0.2 ms** |
| `detectPrograms` | 0.6 ms | 8.3 ms | **0.1 ms** |
| `groupRanges` | 0.1 ms | 1.5 ms | **0.01 ms** |
| `requiredVersion` | 0.1 ms | 1.6 ms | **0.03 ms** |
| `serializeTzx` | 4.7 ms | 7.4 ms | **0.6 ms** |

So the ~40 ms the web app pays on the first look at a changed tape becomes about 1.5 ms, with no
caching needed at all. That is the part of the gate a terminal can answer, and it passed.

**Binary size: 11.2 MB**, stripped, arm64 only, against the 15–25 MB the plan assumed and today's
3.6 MB app plus a system webview. Nothing has been trimmed yet (no `wasm-opt` equivalent, femtovg
rather than the software renderer, no `panic = "abort"`), so this is an upper bound.

**Cold start and the 3,000-row cursor move need the app on screen**, which is the person's part of
this loop:

| | baseline | native |
|---|---|---|
| Cold start to a drawn window | 355 ms | _to be measured: `time … --exit-on-draw`_ |
| Cursor move, 3,000-block list | 76–90 ms | _to be measured: `… --rows 3000 --bench 200`_ |

Both commands print their number and quit; `--bench` also prints min, median and max, and the
status bar shows the last move's key-to-frame time while the window is open.

### The gate answer: egui, not Slint

The gate asked two things — does the list handle 3,000 rows, and does the menu feel right — and
the honest outcome is that it was settled on neither. The Slint skeleton worked, and the numbers
above show the boundary cost disappearing, which is the part that mattered. The toolkit went the
other way for three reasons, in order of weight:

1. **Licence.** Slint is `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR commercial`.
   SpecTape is GPL-2.0-**or-later**, so the GPLv3 path is legal but ends the "or later" freedom
   for anyone downstream, and the royalty-free path carries a condition: the `AboutSlint` widget
   in an About dialog reachable from the top-level menu, or a "Made with Slint" badge where the
   binaries are downloaded. egui and eframe are `MIT OR Apache-2.0`, with no condition at all.
2. **The app is forms-shaped.** `BlockEditor.tsx` is 556 lines over 25 per-type forms,
   `DataWindow.tsx` 496, `Dialogs.tsx` 284 — about 1,300 of the 2,223 UI lines are panels over
   typed data. Slint's structs hold only Slint types, so none of `Body`'s variants can cross as
   itself: every field needs a property, a setter and a callback, or a generic field-model
   indirection. In egui the field *is* the Rust data, and the existing Commit/Revert semantics
   are an edit to a draft copy, which is what the app already does.
3. **Two of Slint's three advantages did not survive contact.** The macOS menu bar is not a Slint
   feature: Slint calls **muda**, the same crate Tauri's `menu.rs` already uses here, and muda
   works from any winit app — eframe exposes the event loop through `event_loop_builder` and
   delivers clicks on `MenuEvent::receiver()`. And "platform-styled widgets" matters little for an
   app that has its own design language: neither toolkit draws native controls, and the Slint
   skeleton painted `src/style.css`'s tokens rather than the cupertino style anyway. What is left
   is text-input polish, a broader widget set and a more mature AccessKit story — real, and small
   against the two points above.

**What that leaves unanswered.** Cold start and the 3,000-row cursor move were never read off a
screen, because the choice stopped depending on them. They are still the premise of this whole
plan — "near 100 ms, or the premise is wrong" — so they are the **first checkpoint of stage 4**,
taken on the egui skeleton with the same `--exit-on-draw` and `--bench` flags. If cold start comes
back anywhere near 355 ms, stop and reconsider before porting the editor.

The Slint skeleton is kept in the branch history rather than in the tree: it is what proved the
core links natively and produced the boundary table above.

### Starting stage 3

**What the core already gives you.** `spectape-core` is an rlib as well as a cdylib, so the native
binary links it directly: no wasm, no wire format, no `core.ts`. Call the Rust functions the app's
TypeScript reaches through `src/tzx/core.ts` — `parser::parse_tape`, `describe::describe_block`
and `block_length`, `content::content_labels`, `programs::group_ranges` and `detect_programs`,
`consistency::check_consistency`, `audio::{playback_order, render_tape}`, and the rest. Stage 2
left nothing in TypeScript that the native app will need, with two exceptions worth knowing about
before the block list is built: inflating Z-RLE CSW blocks needs a zlib crate on this side (the
core has no dependencies on purpose, and the web app does it with pako), and the hex dump's
per-byte character table is `spectrum::charset::char_table`.

**Keep the dependency line where it is.** Slint goes in a *new* binary crate that depends on
`core`, not in `core` itself. The whole reason the core is dependency-free is that it compiles to
a 249 kB wasm module for the web build; a UI toolkit in there would end that. A workspace with
`core/` and `native/` is the natural shape.

**Build the list against a real tape.** `public/samples/SpecTape demo.tzx` has 19 blocks covering
every type; for the 3,000-row question, repeat a sample tape's blocks until the list is long
enough, which is what the stage 2 benchmarks did. Rendering 3,000 rows means calling
`describe_block` and `content_labels` for all of them — in-process now, so the numbers should be
the ones in "What the boundary costs" *minus* the encoding, which was most of them.

**Three numbers to bring back.** Cold start to a drawn window (the baseline is 355 ms, of which
~150 ms is WKWebView creation); a cursor move on a 3,000-block list (the baseline is 76–90 ms);
and the binary's size (the plan assumes 15–25 MB against today's 3.6 MB app plus a system
webview). None can be measured from a terminal session: the app has to be run and watched, so
this stage is a build-and-report loop with the person at the keyboard.

**What "the menu feels right" means here.** `src/state/commands.ts` is the complete command table
— ids, labels, shortcuts, enabled rules — and `src-tauri/src/menu.rs` already builds a native menu
from the same ids. Stage 3 only needs enough of it to judge the feel: the real port is stage 4.

## Stage 4 — Feature parity, area by area — **done**

Port `src/state` into Rust as you go; each area is done when it matches the current app:

1. Block list: selection semantics, collapsed groups, drag & drop, cursor
2. Block editor: per-type forms, commit/revert
3. Data window: hex, screen, BASIC, vars, text, disassembly
4. Dialogs, status bar, options
5. Playback: audio out via `cpal`, progress indicator
6. Files: open, save, save-as, associations, "open with", the emulator launch that already exists
   in `src-tauri/src/emulator.rs`

### Starting stage 4

**The toolkit is egui + eframe, with muda for the menu.** Stage 3's reasoning is above; what it
means in practice is that the UI is ordinary Rust in the same crate as the state, and the only
thing that is not is the menu, which muda hands to the platform.

**The skeleton is already converted**, so stage 4 starts on something that runs:

| | |
|---|---|
| `native/src/main.rs` | tape loading, the row builder, the measuring flags, `run_command` |
| `native/src/app.rs` | the window: header, virtualized block list, status bar, keys, perf |
| `native/src/menu.rs` | muda on macOS, an egui bar drawn from the same table elsewhere |
| `native/src/menutable.rs` | the command table, now with muda accelerators (`CmdOrCtrl+S`) |
| `native/tests/menu.rs` | the same four tests against `src/state/commands.ts` |

`build.rs` and `ui/app.slint` are gone; the table is read at runtime instead of generated into
markup. The binary is **5.2 MB** against Slint's 11.2 MB, both stripped and arm64 only.

Two things the skeleton leaves for stage 4 to do properly: muda only sets the menu on macOS here,
because Windows needs `init_for_hwnd` with the window handle out of eframe (elsewhere the egui bar
draws it, without accelerators), and the list is read-only — no selection, no collapsing, no
dragging.

**First checkpoint, before any porting.** Take the two numbers the stage 3 gate never got — cold
start (`time … --exit-on-draw`, baseline 355 ms) and a cursor move on 3,000 rows
(`--rows 3000 --bench 200`, baseline 76–90 ms) — on the egui skeleton. They are the premise of the
plan, not a detail.

**Where the state lives.** `src/state/store.ts` is 443 lines of Preact signals: two `tapes[side]`,
`active`, `hex`, `locked`, compare modes, clipboard, `dialog`, `dataWindow`, `theme`, with
`commit()` snapshotting for undo and `saved` clearing `dirty`. In Rust it becomes a plain struct
the frame reads and the commands mutate; blocks stay immutable, which is what makes the undo
snapshot cheap. Port it with the first area rather than up front, so it is shaped by a caller.

**One area per session.** The areas below are the order to do them in, and this file is the handoff
between them: each session starts by reading it and ends by writing what it did and what the next
one starts from, the way stages 0–3 did.

### The parity checklist

Every id in `COMMANDS` (`src/state/commands.ts`), which is the complete list of what the app does.
An area is done when its ids work the way the web app works them, including the enabled rules.

All six are done. The checklist is no longer a list to keep by hand either: `tests/menu.rs` reads
`commands.ts` and fails if an id or an enabled rule drifts, and `commands.rs`'s own test runs every
id that does not reach outside the process.

**1. Block list — done.** Selection semantics, collapsed groups as one unit (`unitIndices`),
drag & drop, cursor, the `.playing` marker:
`select-all` · `move-up` · `move-down` · `group` · `toggle-collapse` · `collapse-all` ·
`expand-all` · `select-program` · `extract` · `switch-pane` · `toggle-lock` · `delete` ·
`duplicate` · `cut` · `copy` · `paste` · `insert` · `undo` · `redo`

**2. Block editor — done.** Per-type forms, Commit/Revert, the footer that spells out
"Block length N bytes: flag + M data + checksum":
`view-data` · `view-as-one` · `set-timings` · `insert-file`

**3. Data window — done.** Hex, screen, BASIC, vars, text, disassembly, plus the bit and byte
edits, the search, the last-byte mask and Append/Replace/Save file; the views are virtualised lists:
(no command ids of its own; driven by `view-data`)

**4. Dialogs, status bar, options — done** (the status bar also carries the theme switch the web
menu bar has):
`programs` · `tape-info` · `consistency` · `compare` · `clear-compare` · `find-match` ·
`toggle-hex` · `opt-hex-bytes` · `opt-zero-based` · `emu-settings` · `shortcuts` · `about`

**5. Playback — done.** `cpal` output at the device's own rate, the progress indicator read off one
`Progress` instead of four signals; `positionAt` stays a binary search on the caller's array:
`play` · `play-cursor` · `play-selection` · `stop` · `export-wav`

**6. Files — done, except one path.** `rfd` dialogs, tapes named on the command line, files dropped
on a pane, and the emulator launch copied from `src-tauri/src/emulator.rs`. The bundle declares the
TZX/TAP document types, but the Apple Event macOS sends to an already-running app is not wired:
`new` · `open` · `open-other` · `save` · `save-as` · `save-tap` · `emu-tape` · `emu-cursor` ·
`emu-selection`

**Two things the native side must bring with it that the core does not have:** inflating Z-RLE CSW
blocks needs a zlib crate here (the core is dependency-free on purpose; the web app uses pako), and
the "hex bytes" and "number blocks from 0" options change formatting everywhere, so they belong in
the state struct that the row builders read, not in the call sites.

Keep a parity checklist against `src/state/commands.ts` — it is the complete list of what the app
does, which makes "are we done" answerable rather than a feeling.

**Not shipped until the checklist is complete.** This is the only long stretch without a release;
it is unavoidable, because a half-ported UI is worse than either side.

### What stage 4 built

Done on 2026-09-18. `native/` is the whole app now: two tape panes with their editors, the block
list with its selection semantics, the data window, every dialog, the status bar, playback through
`cpal` and files through `rfd`. 7,400 lines of Rust in 21 modules plus 700 of tests, against the
3,300 lines of TypeScript in `src/state/` and `src/ui/` they replace — the difference is mostly
the forms, which say in Rust what JSX says in markup.

| | |
|---|---|
| `src/state.rs` | the store: two tapes, cursor, selection, collapse, clipboard, undo, compare |
| `src/commands.rs` | every command id, dispatched; the context menu; the key table |
| `src/actions.rs` | the higher-level actions, and `Then`, a named follow-up where the web has a closure |
| `src/list.rs` | the block list: rows, collapsing, drag & drop, the consistency marks |
| `src/editor.rs` | the 25 per-type forms over a draft `Body`, with Commit/Revert |
| `src/datawin.rs` | dump, screen, BASIC, variables, text, disassembly, and the bit/byte edits |
| `src/dialogs.rs` | insert, tape info, consistency, WAV export, programs, emulator, about, confirm |
| `src/statusbar.rs` · `src/menu.rs` · `src/menutable.rs` | the bars, and the one command table |
| `src/player.rs` · `src/tape.rs` | cpal output; zlib for Z-RLE CSW and the `positionAt` search |
| `src/files.rs` · `src/settings.rs` · `src/emulator.rs` | rfd, the options file, the emulator launch |
| `src/widgets.rs` · `src/theme.rs` · `src/fmt.rs` · `src/tables.rs` | fields, tokens, formats, labels |

Four things worth knowing:

1. **`dirty` is a generation number, not object identity.** The web store compares `snap.blocks !==
   t.saved`; every version of the blocks array here carries a counter instead, so undoing back to
   the saved version clears the dot exactly as it does on the web, and redoing sets it again.
2. **A dialog's "OK" is a value, not a closure.** `confirmDiscard(side, () => …)` would have to
   capture the store it is about to mutate. `Then` names the follow-up instead — `Then::ExtractGo`,
   `Then::EmulatorGo` — which the frame runs once the dialog is gone. It is also the only reason a
   test can check what a dialog would do without pressing its button.
3. **Edit commands go to a focused text field first.** On macOS the platform menu owns ⌘X/C/V/A/Z,
   so a field would never see them: `commands::text_field_edit` turns the menu click back into the
   egui input event the field is waiting for, with `arboard` supplying the clipboard text for
   Paste. This is the rule `handleNativeMenu` follows in `src/ui/App.tsx`, and it is why the native
   Edit menu can act on both the tape and a text field.
4. **The rows are built once per version of the tape**, keyed on the same generation number plus a
   counter for the display options. A cursor move rebuilds nothing; changing the Dec/Hex switch
   rebuilds both panes. This is the in-process version of the two caching rules stage 2 measured.

**The icons are geometry, not glyphs.** The toolbar and status bar first went in as emoji (📂 💾 🔒
☀), which is a font dependency wearing a different hat: the glyph differs per platform and per font
set. `src/icons.rs` is the port of `src/ui/icons.tsx` — the same 24×24 stroke paths as polylines,
circles and arcs, painted by egui, with a test that paints every one at every size the app uses.

The reason for doing it first was a guess that trimming egui's font list would be the cold-start
lever. **It is not**, and `--measure` now says so out loud: the app's whole first frame is 4–6 ms
headless, and dropping both emoji fonts saves about 1 ms of it. egui rasterises glyphs on demand,
so fonts it never draws from cost almost nothing. Whatever cold start turns out to be, it is
process start, window creation and the GL context — not egui, not the fonts, and not the core,
which parses the demo tape in 0.7 ms. The trim is kept as `theme::latin_only_fonts()` for the
measurement and for the platforms that spell shortcuts "Ctrl+S"; macOS needs those fonts for the
⌘⇧⌥⌃⌫ in the shortcut list.

**What the tests cover.** `npm run native:test` is 57 tests:

- `tests/menu.rs` reads `src/state/commands.ts` and checks the table against it — every id present
  in both, no duplicates, and now **the enabled rule of every command**, parsed from the web's own
  `enabled:` expressions. A predicate this test cannot read is a failure, not a skip.
- `src/state.rs` tests the semantics nobody wrote down: dirty across undo/redo/save, collapsed
  groups as one unit, what moves when a selected block is grabbed, move-up past a collapsed group,
  paste getting fresh uids, the four click modes, compare and find-match marks.
- `src/commands.rs` runs **every command id** that does not reach outside the process and fails if
  one has no arm; plus the option toggles, a disabled command doing nothing, and the Edit commands
  going to a focused field.
- `src/icons.rs` paints every icon at every size the app uses, through egui's real tessellator.
- `src/list.rs` tests the list itself: what a collapsed group hides, where a drop lands next to
  one, the drop the web ignores, and that the rows carry the indent, the range and the block
  numbering the options ask for.
- `src/app.rs` draws real frames headlessly, through `egui::Context::run_ui` with no window and no
  event loop: the cursor on all 25 block types, an empty tape, all nine dialogs, all six data
  window views, a collapsed group with its context menu open, and both themes. This is the native
  answer to `npm run smoke`, and it catches a layout panic or a bad index before the window does.

**What is left, honestly.** One is the person's; the other two are platform plumbing, and both are
scheduled into stage 5 below rather than left floating here — they share one seam (the event loop
and window handle that eframe hands out) and one verification loop (install the packaged app on the
platform and try it), so doing them in stage 4 would mean building the packaging twice:

- **The two numbers.** Cold start and a 3,000-row cursor move still have to be read off a running
  window; `npm run native` prints both commands. The boundary numbers this stage could take from a
  terminal are unchanged from stage 3 (0.67 ms to describe 3,000 blocks, 0.17 ms for the content
  labels, 0.63 ms to serialize), and the first frame costs 4–6 ms of them. Binary size is **6.1 MB**
  stripped, arm64 only, up from the skeleton's 5.2 MB with rfd, cpal, zlib, png and arboard added.
  So if cold start comes back high, none of the candidates are in this repo: look at dyld, at
  NSApplication and window creation, and at whether an unsigned bundle is paying a Gatekeeper toll
  on its first run.
- **File associations.** The bundle now declares TZX and TAP as document types, and a tape named on
  the command line opens (a second one goes into the right pane, the way `?open=&right=` does on
  the web). What is not wired is the Apple Event macOS sends when you double-click a document while
  the app is already running: eframe does not surface it, so that path needs a winit hook. Windows
  needs its association written by an installer, which is stage 5's packaging work anyway.
- **muda on Windows.** Still `init_for_hwnd`, still waiting for the window handle out of eframe.
  Until then Windows gets the egui menu bar, with the accelerators handled by `app.rs` from the
  same table — so nothing is missing, it just is not the platform's own bar.

The difference in urgency between the two is worth keeping: the Apple Event is a *broken* path once
associations are registered (double-click a tape while the app is running and nothing happens at
all), while the Windows menu bar is a *different-looking* path that works. If stage 5 runs long,
the first must ship and the second can slip.

### Starting stage 5

The native binary becomes the desktop app. `src-tauri/` retires except for `emulator.rs`, which
`native/src/emulator.rs` already holds a copy of — delete the Tauri one and its two
`#[tauri::command]` attributes are the whole difference. `scripts/build-native.mjs` is the start of
the packaging: it already writes the Info.plist with the document types. The release workflow
(`.github/workflows/release.yml`) is what has to change next, and the Linux download should fall
from 76 MB to under 25 MB once WebKitGTK is gone.

**What retires is the Tauri shell, not the web app.** Tauri is a desktop wrapper around the
TypeScript UI, which is the role `native/` takes over; `src/` stays as the browser build. So the
wasm boundary (`core/wire.rs`, `core/wasm.rs`, `scripts/build-wasm.mjs`, the size-first profile in
`core/Cargo.toml`) keeps earning its keep, and so does `test/core.test.ts`, which runs the frozen
TypeScript in `test/reference/` against the core through it. `native/` stays out of the cargo
workspace for the same reason it always did.

**The two platform items stage 4 left.** Both belong here, because both need the packaged app
installed on the platform before they can be judged:

1. **The macOS Apple Event.** Once the `.app` is registered with LaunchServices, double-clicking a
   tape while SpecTape is already running sends `kAEOpenDocuments` — no argv — and today nothing
   happens. eframe does not surface it, so it needs a winit hook at the same seam that
   `event_loop_builder` already hands to muda. Ship this one: it is a dead path, not a cosmetic gap.
2. **muda on Windows.** `init_for_hwnd` with the window handle out of eframe, at that same seam.
   Optional in a way the first is not — Windows works today with the egui-drawn bar and the
   accelerators `app.rs` handles from the same table — so it can slip to stage 6 if the packaging
   round trips eat the time.

Neither can be verified from a macOS terminal: the first needs an installed bundle, the second a
Windows machine or a CI run.

## Stage 5 — Switch (a day, plus CI round trips) — **done**

The native binary becomes the desktop app. `src-tauri/` retires except for the emulator code.
Packaging changes to plain binaries plus a `.app`, `.msi` and an AppImage that no longer carries
WebKitGTK — the Linux download should fall from 76 MB to under 25 MB.

The web app keeps the existing TypeScript UI on the wasm core, and is maintained as its own thing.
**Decided, 2026-09-18:** it stays. A light local native executable next to the web application is
the goal of the whole exercise, so the "delete `src/ui` and save a week" branch this paragraph used
to offer is closed — do not reopen it. What that leaves is a working practice rather than a gate:
when a feature is added, it goes into the native app, and into the browser one only if it earns its
place there. The core protects the expensive half either way — parsing, descriptions, consistency,
audio, BASIC and the disassembler are shared — so what can diverge is UI only, and the two already
have (`?open=&right=` is argv on the native side, and the macOS build has a platform menu bar the
browser cannot). Where they diverged by accident rather than by platform, though, the web build is
the reference: it is the app in README.md's screenshot, and the native one was measured against it
after stage 5 — the window's menu bar, its Left/Right grouping, the theme switch at the right end
of it, the L/R tag, the version pill and the toolbar rule all came back that way. Parity is pinned where it matters, against `commands.ts`, by
`native/tests/menu.rs`.

### What stage 5 did

Done on 2026-09-18.

**The shell is gone.** `src-tauri/` is deleted — 638 lines of Rust, the config, the capabilities
and the generated schemas — and with it the four `@tauri-apps/*` packages. The icons moved to
`assets/icons/`, which is now where both the bundles and the window icon come from.
`emulator.rs` needed no work: `native/src/emulator.rs` was already the same file without the two
`#[tauri::command]` attributes.

**The front end lost its desktop half**, which is the part of stage 6 that could not wait: with the
Tauri API gone, `src/platform/tauri.ts` would not have compiled. `isDesktop` and every branch it
guarded are gone, and so are the `Platform` members only the shell implemented (`onOpenWith`,
`onMenu`, `setMenuChecked`, `ready`, `rememberTheme`, `detectEmulator`, `pickProgram`,
`openInEmulator`), the native-menu dispatcher `handleNativeMenu`, and `TapeState.path` — a browser
has nowhere to write back to, so Save is Save as there. `src/` is 180 lines lighter and says one
thing: this is the browser build. The command *ids* did not change, which is what
`native/tests/menu.rs` checks.

**Packaging is one script.** `scripts/build-native.mjs` builds the app and, with `--package`,
writes what a release carries: `.dmg` + `.zip` on macOS (`--universal` builds both architectures
and lipos them), an `.AppImage` from an AppDir plus a `.tar.gz` on Linux, and a portable `.exe`
plus a WiX `.msi` on Windows. CI runs the same command, so a release is not a second way of
building the app. `npm run desktop` is the build, `desktop:test` the tests, `desktop:package` the
artifacts; `npm run native*` is gone.

| | Tauri | now |
|---|---|---|
| macOS download | 3.6 MB | 3.4 MB dmg (arm64; 6.2 MB binary) |
| Linux download | 76 MB AppImage | the 6.2 MB binary, in a tarball or an AppImage |
| Windows download | 1.3 MB + WebView2 runtime | 6.2 MB exe, nothing else |
| Runtime dependency | system webview | none |

The macOS bundle grew 6.2 MB of binary but the download shrank, because a compressed native binary
packs better than a bundle whose code lived in the system. The Linux number is the whole point: no
WebKitGTK to carry.

**The two platform items are both in.**

1. **The macOS Apple Event.** `native/src/macos.rs`. Launch Services sends `kAEOpenDocuments`, not
   argv; AppKit turns it into `application:openURLs:` on the application delegate; winit registers
   that delegate (`WinitApplicationDelegate`) and implements only the two lifecycle methods. So the
   module adds the method to whatever class the delegate is, from an observer of
   `NSApplicationWillFinishLaunchingNotification` — the last moment before AppKit delivers the
   launch event and the first at which the delegate exists — and sets the delegate again, because
   `NSApplication` caches which selectors it answers. Paths land in a queue that `App::frame`
   drains into `files::open_with`, the way the shell had the front end drain `take_pending_files`.
   **This is the one thing here that no test can reach**: it needs an installed bundle and a
   double-click. The test covers the queue and the URL-to-path step.
2. **muda on Windows — tried, and taken out again.** `Menu::new` pulled the HWND out of eframe's
   `CreationContext` and called `init_for_hwnd`. On a real Windows machine the window came back
   with a black strip where the menu should be and every click landing a menu-height from what it
   hit: a Win32 menu shrinks the client area, and nothing tells egui. So Windows draws the egui bar
   again, as it did in stage 4.

   Worth separating out, because the report that came with it was three symptoms and only two of
   them were Windows'. The third — "the menu with Left and Right is gone" — was this change
   dropping the *in-window* bar on Windows (`draws_in_window()` was true only for the egui variant),
   which is the same hole macOS had had since stage 3 and is fixed for every platform by putting
   that bar back everywhere. It never needed muda taken out.

   The geometry is Windows' own in kind — only Windows had a menu attached to its window, and the
   same build showed neither symptom on macOS — but it is an inference: a post-revert Windows build
   has not been run. If the strip and the offset survive it, muda was innocent and the next suspect
   is a display scale factor other than 100%, which would be stage 4's bug rather than this one's.
   Either way the revert costs nothing: muda's accelerators on Windows want a `TranslateAccelerator`
   in the message loop, which winit does not have, so the platform bar there was always a menu whose
   keys someone else handled.

**CI builds and tests the desktop app on all three platforms** now (`cargo fmt --check`, `clippy
-D warnings`, `cargo test`), next to the web job. The release workflow no longer uses
`tauri-action`: each platform packages itself and a final job drafts the release with `gh`. A
`workflow_dispatch` run stops before the release and leaves the packages on the run, which is how
to try the packaging without cutting a release.

**Verified on the installed bundle** (2026-09-18, `/Applications/SpecTape.app`). Both Apple Event
paths work: a tape opens in SpecTape whether the app was running or not. Two things that took
finding, neither visible from a terminal:

- Double-clicking a `.tzx` opens *Fuse*, because Launch Services keeps the user's own default and
  an emulator claimed the extension first. That is macOS working as intended — Finder's Get Info →
  Open with → Change All is the answer, and no `LSHandlerRank` in our plist should try to win it.
- `open -a SpecTape` with no document opened **the demo tape**: `DEFAULT_TAPE` was a stage-3
  convenience, and the fallback path it searches includes `CARGO_MANIFEST_DIR`, which is compiled
  into the binary — so a shipped app opened whatever tape sat next to the repo it was built in.
  Started with nothing to open, the app now opens nothing; only `--measure`, `--rows` and `--bench`
  still fall back to the demo tape, because they have to measure something.

**Two extras that came with the move.** The window and taskbar now carry the app icon (decoded
from the same PNG with the `png` crate the screen view already uses), and the in-window menu bar is
covered by a headless frame test on every platform — it used to be compiled out on macOS, where
it is developed.

## Stage 6 — Cleanup (an hour)

Finish what only a real machine can answer. The dead TypeScript and CI were done in stage 5,
because neither could wait for it, and "Where we start" was re-measured on 2026-09-18.

### After stage 5: what using the app turned up

The stage is done and the app shipped to one desktop — and then a day of using it found things no
test had, all of them older than stage 5 and none visible from a terminal. Worth listing, because
the pattern is the lesson:

- **The window's menu bar** was missing on macOS (since stage 3) and on Windows (since stage 5's
  muda attempt). It is the app's own bar, grouped Left/Right the way `MenuBar.tsx` groups it, and
  it is back on every platform. The **theme switch** had drifted from that bar into the status bar;
  the **L/R tag**, the **version pill** and the **toolbar rule** had been flattened into plain text.
  Every one was a silent divergence from `src/ui/`, which is the app people know.
- **The editor clipped its screen thumbnail** in a narrow pane — egui clips overflow and shows no
  scrollbar, so the preview simply vanished. The row wraps now, as the web's flex-wrap does.
- **A dialog's ✕ did nothing**, in every dialog, because the body's outcome was assigned over it.
- **`DEFAULT_TAPE`**, a stage 3 convenience, made the installed app open a tape out of the repo it
  was built in.

What all of these have in common is that the test layer could draw the UI but never *look* at it,
and never clicked anything. Both holes are now closed: `native/src/shot.rs` rasterises egui's own
triangles and font atlas into a PNG with no window and no GPU (`--screenshot out.png,WxH
--theme light --cursor N`), and a test can click a widget by id over two frames. The two
regression tests that came out of this — a preview that must draw red pixels at three pane widths,
a ✕ that must close the dialog — both fail against the code they were written for.

### Starting stage 6

What is left is small and, unusually for this plan, mostly *not* code:

- **The two numbers are in** (2026-09-18, read off the bundle): 161 ms cold start against 355 ms,
  and 0.27 ms against 76–90 ms for a cursor move on 3,000 rows. Both are in the table at the top of
  this file, with what they mean. Nothing is owed on the measuring side any more.
- **A real Windows and a real Linux run.** CI builds, tests and *packages* both — the run of
  2026-09-18 produced every artifact, including the first `.msi` WiX has managed — but nobody has
  installed that `.msi` and double-clicked a `.tzx`, or run the AppImage. The Windows exe from that
  run is also the one that answers the open question above: it is the first Windows build since
  muda was taken out, so if the black strip and the offset clicks are gone, the revert rested on
  the right cause; if they survive, the suspect is a display scale other than 100%, which would be
  stage 4's bug.
- **macOS is done**: both Apple Event paths were confirmed on the installed bundle (above).
- **Naming.** The crate directory is still `native/` and the package still `spectape-native`,
  though the binary it builds is `spectape` and it is the only desktop app there is. Renaming the
  directory is a rename of paths in three scripts, two workflows and this file — worth doing
  once the platforms above have been tried, not before.
- **The UI parity sweep is done** (2026-09-18). See below for what one pass turned up.
- **The dead TypeScript is already gone**, and so are the measurements and CLAUDE.md's map, all
  done during stage 5. Stage 6 is the platforms and the rename.

### The parity sweep

One `--screenshot` of the two sample tapes against `scratch/01-main.png`, the smoke test's own
picture. `docs/screenshot-main.png` was a stage-4 copy of it and had gone stale — it predated the
Programs button, so the reference showed five toolbar icons where the app has six; it has been
refreshed from a smoke run, and the way to keep it current is `npm run smoke` and a copy.
Twenty-odd divergences from `src/ui/`, every one of them invisible to a test that only draws, and
one of them a real bug:

- **The editor's row buttons and its footer were off the pane.** A `TextEdit` asking for
  `desired_width(INFINITY)` with buttons after it does not clip in egui: it *widens the enclosing
  `Ui`*, and `set_max_width` will not shrink one back (the placer unions the new max rect with what
  has been laid out). So Archive info's − and ↑ were drawn past the pane edge, and Commit and
  Revert with them — under the other pane, whose background then painted over them. On the right
  pane they were half visible; on the left, gone. Fixed by reserving the buttons' width and by
  laying the footer out in a child `Ui` pinned to the rect the pane handed over, which is also
  what `.editor .body { flex: 1 }` does: Commit sits at the bottom whatever the form above it is.
  `app::tests::the_editor_footer_stays_inside_its_pane` fails against the code it was written for.
- **The block list** was on 20px rows against the web's 26, had a zebra stripe the web has never
  had, drew `.kind` as bare text instead of a bordered pill, set the description in a proportional
  font where `.blocklist` is monospace throughout, dimmed the cursor row when the block was
  metadata (`.row.cursor .desc` takes the text colour back), and had every column about 20px right
  of where `style.css` puts it.
- **The window's wordmark** — `.brand`, the cassette in a rounded square — was missing, and the
  menu bar was 30px of window background rather than 44 of `--surface` under a rule. The status
  bar had the same problem, and its cells were plain text between separators instead of `.cell`
  pills.
- **`.pane-head`** had no tinted band and no rule, **`.editor`** no tinted panel, and the editor
  splitter no `::after` grab handle — just a hairline.
- **The active pane** was ringed in full-strength accent where `.pane.active` uses `--accent-soft-2`.
- **Play and Stop were filled**; every icon in `src/ui/icons.tsx` is `fill="none" stroke=…`.
- **Badge ids stayed white in the dark theme**, where the web inks them `#0b1220` because the
  category colours lighten.
- **`↑` was a hollow box**: egui's font set has no U+2191, which is the rule about glyphs in
  CLAUDE.md meeting the one place the port had ignored it. It and `−` are icon paths now.
- **The screenshot itself started from black**, so anything the app did not cover read as black
  rather than as the colour eframe clears the window to. `shot::capture` clears to `panel_fill`.
- And the small change: 5px of padding round the panes against `.panes`' 10, an 8px gap between
  them against `.vsplitter`'s 10, and a playback bar filled in the accent where `.fill` is `--ok`.

None of this is behaviour, which is why stage 4's tests all still pass unchanged. It is the part
of a port that only a picture can check, and the picture is now a command away.

### A panic, and where panics go now

Opening a tape with a group in it and then a shorter one from the pane's folder button killed the
app: `index out of bounds: the len is 2 but the index is 5`. `App::frame` rebuilds the row caches
early and draws afterwards, and two things run a command in between — the in-window menu bar, and
the pane toolbar, whose Open button is a few lines above `list::show`. The load replaced the
blocks while the cache still described the tape that had been open, so `visible_rows` walked the
old rows and indexed the new blocks with them. It only reads `blocks[i]` for a row with a
`range_end`, which is why it takes a group or a loop to show it. `list::show` refreshes the cache
itself now, which is a generation comparison and costs nothing when nothing has moved.

The macOS platform menu and the keyboard shortcuts were never affected: `handle_menu` and
`handle_keys` both run before the refresh. That is also why the headless tests missed it — they
load tapes *between* frames, never inside one.

That hole is closed too. `Menu::fire_next_frame` is a test-only queue on the in-window bar that
`bar` drains where a real click would be read, so the value takes the production path from there:
a test can now run any command at the point in the frame the bar runs one. The sweep
`every_command_survives_being_run_in_the_middle_of_a_frame` puts the whole table through it with
groups in both panes, and checks afterwards that each pane's rows still describe the tape that is
open. Without the `list::show` fix it fails on `new` — `len is 0 but the index is 9`, the same
line the reported crash came from, reached by a different command. Twelve ids stay out of it,
listed in `NOT_HEADLESS`: six open a native file dialog and would block the run, six reach for an
audio device or another program. The pane toolbar cannot be driven instead, for that same reason —
its interesting buttons are the file ones.

What the episode actually cost was the hour before the line number. A Rust panic unwinds and exits
101: macOS files no crash report for an exit, `~/Library/Logs/DiagnosticReports` stayed empty, and
the unified log does not carry the stderr of an app launched from Finder. The report was "it
terminates", and it took running the bundle's binary from a terminal to get `list.rs:159:46` —
which a user on Windows could not have done at all, because `#![windows_subsystem = "windows"]`
means there is no console. So `native/src/crashlog.rs` now installs a panic hook that writes the
message, the location and a backtrace to `panic.log` beside the settings, and the About dialog
names that file once it exists. The location survives stripping either way; the release profile
keeps the symbol table (`strip = "debuginfo"`, 6.2 MB to 7.0 MB) so the backtrace names functions
rather than repeating `__mh_execute_header`, which is the difference between knowing the line and
knowing the call path that reached it.

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
