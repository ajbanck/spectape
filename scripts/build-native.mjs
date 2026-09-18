// Builds the stage 3 native shell (native/) and, on macOS, wraps it in a minimal
// .app bundle, because an unbundled binary gets the executable's name in the menu
// bar and starts behind the terminal — neither of which the stage 3 gate is about.
//
//   node scripts/build-native.mjs [--debug]
//
// Prints where the binary and the bundle are and how to run them. It never opens
// a window: running the app is the person's job, since the numbers this stage
// wants can only be judged on screen.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, copyFileSync, writeFileSync, statSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const native = join(root, 'native');
const release = !process.argv.includes('--debug');
const profile = release ? 'release' : 'debug';

// Homebrew's cargo is on PATH and is enough for a host build; scripts/build-wasm.mjs
// explains why the wasm build picks the rustup shim instead.
const cargo = process.env.CARGO || 'cargo';

console.log(`Building spectape-native (${profile})…`);
execFileSync(cargo, ['build', ...(release ? ['--release'] : [])], { cwd: native, stdio: 'inherit' });

const bin = join(native, 'target', profile, 'spectape-native');
const mb = (p) => (statSync(p).size / 1024 / 1024).toFixed(1);
console.log(`\nBinary: ${bin} (${mb(bin)} MB)`);

if (process.platform !== 'darwin') {
  console.log(`\nRun it with:\n  "${bin}" "public/samples/SpecTape demo.tzx"`);
  process.exit(0);
}

const app = join(native, 'dist', 'SpecTape Native.app');
rmSync(app, { recursive: true, force: true });
mkdirSync(join(app, 'Contents', 'MacOS'), { recursive: true });
mkdirSync(join(app, 'Contents', 'Resources'), { recursive: true });
copyFileSync(bin, join(app, 'Contents', 'MacOS', 'spectape-native'));
const icon = join(root, 'src-tauri', 'icons', 'icon.icns');
if (existsSync(icon)) copyFileSync(icon, join(app, 'Contents', 'Resources', 'icon.icns'));
writeFileSync(
  join(app, 'Contents', 'Info.plist'),
  `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>SpecTape Native</string>
  <key>CFBundleDisplayName</key><string>SpecTape Native</string>
  <key>CFBundleIdentifier</key><string>dev.spectape.native</string>
  <key>CFBundleExecutable</key><string>spectape-native</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.2.2</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
`,
);

console.log(`Bundle: ${app}`);
console.log(`
Run it — the bundle, so the menu bar says SpecTape Native:
  "${app}/Contents/MacOS/spectape-native" "public/samples/SpecTape demo.tzx"

The three numbers the stage 3 gate asks for:
  cold start   time "${app}/Contents/MacOS/spectape-native" --exit-on-draw
  cursor move  "${app}/Contents/MacOS/spectape-native" --rows 3000 --bench 200
  binary size  ${mb(bin)} MB (above)`);
