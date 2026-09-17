# SpecTape

SpecTape is an editor for ZX Spectrum **TZX** and **TAP** tape images, for the desktop and the
browser. Everything runs locally; no files are uploaded anywhere.

![SpecTape with two tapes open](docs/screenshot-main.png)

## Download

Desktop builds for macOS, Linux and Windows are attached to each
[GitHub release](../../releases).

| Platform | File | Requirements |
|---|---|---|
| macOS 11.3+ (Intel and Apple Silicon) | `.dmg` | The bundle is unsigned: right-click the app and choose **Open** the first time. |
| Linux | `.deb`, `.rpm` or `.AppImage` | `webkit2gtk-4.1` (Ubuntu 22.04, Debian 12, Fedora 37 or newer). |
| Windows 10+ | `.msi` or `.exe` installer | WebView2 runtime; the installer fetches it if missing. |

The browser version is the same app without native file dialogs: Open reads a file you pick,
Save downloads a copy. It works in Safari 14.1, Chrome 90, Firefox 90 and newer. To run it
yourself, see [Building from source](#building-from-source).

## Features

**Two tape windows** (Left / Right), each with a numbered block list, a per-block editor and
Commit / Revert buttons. Drag the bar between the tapes to resize them
(double-click for equal widths), and the bar above an editor to resize the editor.

**File formats**: reads and writes TZX 1.20 (blocks 10–19, 20–28, 2A, 2B, 30–33, 35, 5A); unknown
and deprecated blocks are preserved byte for byte. The saved TZX version is the lowest one that can
hold the content, but never lower than the version the file was loaded with, so an unaltered tape
round-trips byte for byte. TAP files are read and written (only data blocks survive TAP export).

**Block operations**: insert any block type, cut / copy / paste / duplicate / delete, move up and
down, drag & drop within and between tapes (hold Alt or Ctrl to copy), group the selection,
collapse and expand groups and loops (a collapsed group acts as one block), multi-select with
Shift-click and Ctrl-click (⌘-click on macOS), undo / redo per tape, lock switch to prevent
accidental edits.

**Collection tapes**: Programs… (Ctrl+J, or the list button in a tape's toolbar) lists the games on
the tape and selects the one you pick. Programs start at BASIC Program headers, at groups that
contain one, and at Select block entries; everything else (headerless custom loaders, stop blocks
between the parts of a multi-load game) stays with the program before it. Select program
(Ctrl+Shift+A) selects the program at the cursor; Extract to other pane (Ctrl+Shift+E) copies the
selection into the other pane as a new tape named after the program, ready for Save As. Group
blocks manually to fix a wrong split.

**Block editors** for every block type: standard / turbo / pure data timings with a ROM-timings
preset, ROM header fields (type, name, length, autostart line, start address), pure tone, pulse
sequence, direct recording (T-states or sample rate), CSW, generalized data (symbol tables and
pilot stream in a text syntax), pause, group, jump, loop, call sequence, return, select,
signal level, text, message, archive info, hardware type (full hardware list), custom info
(with a text editor for `POKEs` blocks) and glue.

**Data window** (Enter, double-click or View data):

- Hex dump with in-place hex and ASCII editing, search pattern with `?` wildcards.
- View as Screen (with FLASH animation, hide attributes, Save to SCR / PNG).
- BASIC listing with hidden-number detection, "Show numbers", Speccy 32-column formatting and 128k
  tokens; variables area listing.
- Text view with Spectrum character set and tokens.
- Z80 disassembly (all prefixes, undocumented forms) with ROM routine labels.
- Base address, flip bytes (RR), reverse order (DEC IX), hide flag / checksum byte modifiers.
- Bit and byte level Drop / Add / Shift left / Shift right and last-byte mask.
- Append file, replace from file, save block data to file. "View selected as one" joins the
  selected blocks bit-exactly.

**Content labels** in the block list come from the preceding ROM header when there is one. If
the data length differs from the header, the label says so (`SCREEN (short)`, `CODE 32768 (long)`);
blocks without a usable header are guessed from their size and bytes (`SCREEN?`, `BASIC?`).

**Tape tools**: tape info (size, TZX version, estimated playing time, block counts), consistency
check (nesting, useless loops, bad jumps, calls without return, infinite loops, checksums),
compare tapes and find match with block-compare and tape-compare modes (magenta = differs,
grey = ignored, green = match), set selection timings to current block.
Blocks with consistency problems are marked in the block list: a red `!` for errors (invalid
structure, bad jump targets, broken generalized data), an amber `!` for warnings such as a
checksum that does not match or data that is shorter or longer than its header says; hover
for the details. A collapsed group shows the marks of the blocks inside it.

**Audio**: play the tape, play from cursor or play the selection through Web Audio, and export
WAV (8/16-bit, several sample rates) with either a square wave or MIC-emulation waveform. Loops,
jumps, calls and selects are followed as an emulator would. While playing, the status bar shows
elapsed/total time and a progress bar, and the block being played is marked in the list (click
either the status bar or the "Playing" pill to stop).

**Open in emulator** (Tape menu: whole tape; Block menu and context menu: from the cursor, or the
selection): the desktop app writes those blocks to a temporary TZX and starts an emulator with it.
By default it looks for [Fuse](https://fuse-emulator.sourceforge.net/): the Fuse app on macOS, and
`fuse`, `fuse-gtk` or `fuse-sdl` on the PATH or in the usual install folders on Windows and Linux.
Options → Emulator… picks another program (ZEsarUX, Spectaculator, a Flatpak via `flatpak` with
`run <app-id>` as arguments, …) and extra arguments; the tape file is always the last argument.
If the exported part would break jumps, loops or calls, you are asked first. The browser build
downloads the TZX instead.

**Dec / Hex** switch in the status bar affects every number in the UI (BASIC numbers included),
except the sample-rate field.

**Options** menu (remembered between sessions): *Flag and checksum bytes in hex* shows those byte
values as `0xXX` while other numbers stay decimal, *Number blocks from 0* switches block numbers
in the list, data window, jump/call targets and consistency report to 0-based. The button at the
right end of the menu bar cycles the theme between light, dark and follow-the-system.

### Files on the desktop

Open and Save use native dialogs, **Save (Ctrl+S) writes back to the file you opened**, and `.tzx`
/ `.tap` files are associated with the app so they open with a double click (or by dropping them on
the app icon in the macOS Dock). Tapes can also be opened from the Left / Right menus or dropped
onto a tape pane. In the browser, Save downloads a copy, and tapes can be given in the URL:
`?open=samples/SpecTape%20demo.tzx&right=other.tzx`.

### Keyboard and mouse

Help → Keyboard shortcuts… shows the full list inside the app. The essentials are listed with
Windows and Linux keys. **On macOS, use ⌘ (Command) in place of Ctrl and Option in place of Alt**;
Delete is the Backspace (⌫) key there.

| Keys | Action |
|---|---|
| Click, Shift+click, Ctrl+click | Current block, select range, toggle selection |
| Double click | View data, or collapse / expand a group or loop |
| Drag & drop | Move blocks within or between tapes; hold Alt or Ctrl to copy |
| Drop a file on a tape | Open it; hold Shift to insert it at the cursor |
| ↑ ↓, Ctrl+↑ Ctrl+↓ | Move cursor, move block |
| Enter, Escape | View data, close window |
| Delete or Backspace | Delete current block or selection |
| Insert (desktop: also Ctrl+Shift+N) | Insert block |
| Ctrl+X, Ctrl+C, Ctrl+V, Ctrl+D | Cut, copy, paste, duplicate |
| Ctrl+Z, Ctrl+Shift+Z, Ctrl+A | Undo, redo, select all |
| Ctrl+O, Ctrl+S | Open / save left tape |
| Ctrl+Shift+O, Ctrl+Shift+S | Open / save right tape |
| Ctrl+G, Ctrl+F | Group selection, find match |
| Ctrl+J, Ctrl+Shift+A, Ctrl+Shift+E | Pick a program, select the program at the cursor, extract selection to the other pane |
| Tab, Space | Switch active tape, play / stop from the cursor |
| Ctrl+R, Ctrl+Shift+R | Open the tape / from the cursor in the emulator (desktop) |

Mac keyboards have no Insert key: use ⌘⇧N in the desktop app, or the + button in
the tape toolbar.

### Not implemented

- Loading CSW files into a CSW block (existing CSW blocks are preserved and played; Z-RLE blocks
  are inflated with [pako](https://github.com/nodeca/pako)).

## Building from source

SpecTape is a Vite + Preact + TypeScript front end with a [Tauri 2](https://tauri.app) shell for
the desktop. Node 22 or newer is needed for the web app.

```sh
npm install
npm run dev        # web app at http://localhost:5173
npm run build      # static site in dist/
```

### Desktop app

The desktop build needs a Rust toolchain (`rustup`), plus Xcode command line tools on macOS,
WebView2 on Windows or `webkit2gtk-4.1` development packages on Linux (see the
[Tauri prerequisites](https://tauri.app/start/prerequisites/)).

```sh
npm run desktop         # run the desktop app against the dev server (hot reload)
npx tauri build          # build the installers for the current system
```

`npx tauri build` produces `.msi` and `.exe` installers on Windows; `.deb`, `.rpm` and
`.AppImage` on Linux; and `SpecTape.app` plus a `.dmg` on macOS. All of them are written to
`src-tauri/target/release/bundle/`. Two macOS-only shortcuts are also available:
`npm run desktop:build` builds only `SpecTape.app`, and `npm run desktop:dmg` also packages the
`.dmg` (which opens a Finder window while it lays out the image).

### Supported systems

| Platform | Minimum | Notes |
|---|---|---|
| macOS | 10.13, tested on Big Sur and later | Needs the WebKit that ships with Safari 14.1+. Intel and Apple Silicon via the universal build from CI. |
| Linux | Ubuntu 22.04, Debian 12, Fedora 37 | Requires `webkit2gtk-4.1`; `.deb`, `.rpm` and AppImage from CI. |
| Windows | 10 | WebView2 runtime (bundled installer fetches it). |
| Browser | Safari 14.1, Chrome 90, Firefox 90 | The build targets these engines explicitly. |

The app deliberately avoids `structuredClone`, CSS `:has()`, `color-mix()` and
`DecompressionStream` so it runs on web views that never received updates.

## Development

```sh
npm run typecheck  # tsc --noEmit
npm test           # vitest: parser/writer round trips, flow, audio, disassembler, BASIC, content detection
npm run smoke      # headless-Chrome UI smoke test; screenshots land in scratch/ (dev server must be running)
npm run samples    # regenerate the synthetic sample tapes in public/samples/
```

- `.github/workflows/ci.yml` typechecks, tests and builds the web app on every push.
- `.github/workflows/release.yml` builds the desktop bundles (macOS universal, Linux, Windows)
  on every `v*` tag, or manually from the Actions tab, and attaches them to a **draft** GitHub
  release. Push a tag such as `v0.1.0`, then publish the draft from the Releases page.

### Layout

```
src/tzx/       TZX/TAP model, parser, writer, audio rendering, consistency check, compare
src/spectrum/  character set, BASIC lister, screen renderer, Z80 disassembler
src/state/     application state (Preact signals), undo/redo, file I/O, the command table, playback
src/ui/        Preact components: menus, tape panes, block editor, data window, dialogs
src/platform/  the only file I/O boundary: web (input + download) and Tauri (native dialogs, fs)
src-tauri/     Tauri desktop shell (Rust): window, native menu, file associations, emulator launch
test/          Vitest unit tests
scripts/       headless UI smoke test, sample tape generator
public/samples synthetic demo tapes, generated by scripts/make-samples.mjs
docs/          screenshots for this README
.github/       CI and release workflows
```

The rest of the app does not know which environment it runs in: `src/platform/` hides the web
and Tauri file handling behind one interface, and `src-tauri/` forwards "open with" files and
native menu events to the front end.

## Credits

TZX is the tape format defined by the [TZX specification](https://worldofspectrum.net/TZXformat.html).
The tapes in `public/samples/` are generated by `scripts/make-samples.mjs` and contain no
third-party software.

## Licence

SpecTape is free software, released under the GNU General Public License version 2 or (at your
option) any later version. See [LICENSE](LICENSE) for the full text.
