// Builds SpecTape, the desktop app (desktop/), and packages it for the platform it
// is run on. Stage 5 of docs/rust-migration.md: this replaced `tauri build`, so it
// is what CI calls too — one script, so a release is the same steps a person runs.
//
//   node scripts/build-desktop.mjs [--debug] [--package] [--universal] [--no-build]
//
//   (nothing)    release build, plus SpecTape.app on macOS
//   --debug      debug build, for a quick run
//   --package    also write the artifacts a release carries:
//                  macOS    SpecTape_<version>_<arch>.dmg and .zip
//                  Linux    SpecTape-<version>-<arch>.AppImage (if appimagetool is
//                           there) and a .tar.gz, always
//                  Windows  SpecTape_<version>_x64_portable.exe and, with the WiX
//                           `wix` command on PATH, an .msi that registers .tzx/.tap
//   --universal  macOS: build both architectures and lipo them into one binary
//   --no-build   package what is already in desktop/target
//
// It never opens a window: running the app is the person's job. On macOS the
// binary is wrapped in an .app even for a plain build, because an unbundled
// binary gets the executable's name in the menu bar, starts behind the terminal,
// and cannot declare the document types that make "open with" work at all.
import { execFileSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  copyFileSync,
  writeFileSync,
  statSync,
  rmSync,
  symlinkSync,
  chmodSync,
  readFileSync,
} from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const crate = join(root, 'desktop');
const dist = join(crate, 'dist');
const icons = join(root, 'assets', 'icons');

const args = process.argv.slice(2);
const has = (flag) => args.includes(flag);
const release = !has('--debug');
const profile = release ? 'release' : 'debug';
const packaging = has('--package');
const universal = has('--universal');

const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
const exe = process.platform === 'win32' ? 'spectape.exe' : 'spectape';

// Homebrew's cargo is on PATH and is enough for a host build; scripts/build-wasm.mjs
// explains why the wasm build picks the rustup shim instead. A cross build (the
// universal one) needs whichever cargo has the other target installed.
const cargo = process.env.CARGO || 'cargo';
const run = (cmd, argv, opts = {}) => execFileSync(cmd, argv, { stdio: 'inherit', ...opts });
const mb = (p) => (statSync(p).size / 1024 / 1024).toFixed(1);
const out = (name) => join(dist, name);

mkdirSync(dist, { recursive: true });

// ---- build ----------------------------------------------------------------

/** The binary to package, built unless --no-build said it is already there. */
function build() {
  const flags = release ? ['--release'] : [];
  if (!universal) {
    if (!has('--no-build')) {
      console.log(`Building SpecTape (${profile})…`);
      run(cargo, ['build', ...flags], { cwd: crate });
    }
    return join(crate, 'target', profile, exe);
  }

  // A universal macOS binary is two builds and a lipo; Rust has no fat target.
  const targets = ['aarch64-apple-darwin', 'x86_64-apple-darwin'];
  const built = targets.map((t) => join(crate, 'target', t, profile, exe));
  if (!has('--no-build')) {
    for (const target of targets) {
      console.log(`Building SpecTape (${profile}, ${target})…`);
      run(cargo, ['build', ...flags, '--target', target], { cwd: crate });
    }
  }
  const fat = join(crate, 'target', `universal-${profile}`, exe);
  mkdirSync(dirname(fat), { recursive: true });
  run('lipo', ['-create', '-output', fat, ...built]);
  return fat;
}

const bin = build();
console.log(`\nBinary: ${bin} (${mb(bin)} MB)`);

// ---- macOS ----------------------------------------------------------------

/** `SpecTape.app`: the document types live in its Info.plist, and nothing else can
 *  declare them — this is what makes a tape open SpecTape when it is double-clicked. */
function macApp() {
  const app = out('SpecTape.app');
  rmSync(app, { recursive: true, force: true });
  mkdirSync(join(app, 'Contents', 'MacOS'), { recursive: true });
  mkdirSync(join(app, 'Contents', 'Resources'), { recursive: true });
  copyFileSync(bin, join(app, 'Contents', 'MacOS', 'spectape'));
  chmodSync(join(app, 'Contents', 'MacOS', 'spectape'), 0o755);
  const icon = join(icons, 'icon.icns');
  if (existsSync(icon)) copyFileSync(icon, join(app, 'Contents', 'Resources', 'icon.icns'));
  writeFileSync(
    join(app, 'Contents', 'Info.plist'),
    `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>SpecTape</string>
  <key>CFBundleDisplayName</key><string>SpecTape</string>
  <key>CFBundleIdentifier</key><string>com.zxtoolkit.spectape</string>
  <key>CFBundleExecutable</key><string>spectape</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>ZX Spectrum tape image</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Owner</string>
      <key>LSItemContentTypes</key>
      <array><string>com.zxtoolkit.spectape.tzx</string><string>com.zxtoolkit.spectape.tap</string></array>
    </dict>
  </array>
  <key>UTExportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeIdentifier</key><string>com.zxtoolkit.spectape.tzx</string>
      <key>UTTypeDescription</key><string>TZX tape image</string>
      <key>UTTypeConformsTo</key><array><string>public.data</string></array>
      <key>UTTypeTagSpecification</key>
      <dict><key>public.filename-extension</key><array><string>tzx</string></array></dict>
    </dict>
    <dict>
      <key>UTTypeIdentifier</key><string>com.zxtoolkit.spectape.tap</string>
      <key>UTTypeDescription</key><string>TAP tape image</string>
      <key>UTTypeConformsTo</key><array><string>public.data</string></array>
      <key>UTTypeTagSpecification</key>
      <dict><key>public.filename-extension</key><array><string>tap</string></array></dict>
    </dict>
  </array>
</dict>
</plist>
`,
  );
  return app;
}

/** A .dmg and a .zip of the bundle. `hdiutil create` lays the image out itself, so
 *  unlike the DMG packager Tauri used it opens no Finder window. */
function macPackages(app) {
  const arch = universal ? 'universal' : process.arch === 'x64' ? 'x64' : 'aarch64';
  const dmg = out(`SpecTape_${version}_${arch}.dmg`);
  const zip = out(`SpecTape_${version}_${arch}.zip`);
  const staging = join(dist, 'dmg');
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(staging, { recursive: true });
  run('cp', ['-R', app, join(staging, 'SpecTape.app')]);
  symlinkSync('/Applications', join(staging, 'Applications'));
  rmSync(dmg, { force: true });
  run('hdiutil', [
    'create', '-volname', 'SpecTape', '-srcfolder', staging,
    '-fs', 'HFS+', '-format', 'UDZO', '-ov', '-quiet', dmg,
  ]);
  rmSync(staging, { recursive: true, force: true });
  rmSync(zip, { force: true });
  run('ditto', ['-c', '-k', '--keepParent', app, zip]);
  return [dmg, zip];
}

// ---- Linux ----------------------------------------------------------------

const DESKTOP_ENTRY = `[Desktop Entry]
Type=Application
Name=SpecTape
Comment=ZX Spectrum TZX/TAP tape editor
Exec=spectape %F
Icon=spectape
Categories=Utility;AudioVideo;Development;
MimeType=application/x-tzx;application/x-tap;
Terminal=false
`;

/** The AppDir an AppImage is made of, and the tarball for people who would rather
 *  have a binary. The .desktop file is where Linux learns about .tzx and .tap. */
function linuxPackages() {
  const appdir = join(dist, 'SpecTape.AppDir');
  rmSync(appdir, { recursive: true, force: true });
  mkdirSync(join(appdir, 'usr', 'bin'), { recursive: true });
  mkdirSync(join(appdir, 'usr', 'share', 'icons', 'hicolor', '128x128', 'apps'), { recursive: true });
  copyFileSync(bin, join(appdir, 'usr', 'bin', 'spectape'));
  chmodSync(join(appdir, 'usr', 'bin', 'spectape'), 0o755);
  writeFileSync(join(appdir, 'spectape.desktop'), DESKTOP_ENTRY);
  copyFileSync(join(icons, '128x128.png'), join(appdir, 'spectape.png'));
  copyFileSync(join(icons, '128x128.png'), join(appdir, '.DirIcon'));
  copyFileSync(
    join(icons, '128x128.png'),
    join(appdir, 'usr', 'share', 'icons', 'hicolor', '128x128', 'apps', 'spectape.png'),
  );
  writeFileSync(join(appdir, 'AppRun'), '#!/bin/sh\nexec "$(dirname "$0")/usr/bin/spectape" "$@"\n');
  chmodSync(join(appdir, 'AppRun'), 0o755);

  const made = [];
  const tar = out(`SpecTape_${version}_${process.arch === 'arm64' ? 'aarch64' : 'x86_64'}.tar.gz`);
  run('tar', ['-czf', tar, '-C', appdir, 'usr/bin/spectape', 'spectape.desktop', 'spectape.png']);
  made.push(tar);

  // appimagetool is not a build dependency: without it the tarball is the download.
  const tool = process.env.APPIMAGETOOL || 'appimagetool';
  const appimage = out(`SpecTape-${version}-${process.arch === 'arm64' ? 'aarch64' : 'x86_64'}.AppImage`);
  try {
    rmSync(appimage, { force: true });
    run(tool, ['--appimage-extract-and-run', appdir, appimage], {
      env: { ...process.env, ARCH: process.arch === 'arm64' ? 'aarch64' : 'x86_64' },
    });
    made.push(appimage);
  } catch (e) {
    console.log(`\nNo AppImage: ${tool} would not run (${e.message.split('\n')[0]}).`);
    console.log('Set APPIMAGETOOL to its path to get one; the tarball above is complete without it.');
  }
  return made;
}

// ---- Windows --------------------------------------------------------------

/** WiX v4+ source: one component with the exe, a Start menu shortcut, and the two
 *  file associations. Windows has no bundle to declare them in, so they are
 *  registry entries an installer writes — which is why there is an .msi at all. */
function wxs() {
  return `<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="SpecTape" Manufacturer="SpecTape" Version="${version}" Language="1033"
           UpgradeCode="7d6f1f6e-5f1a-4a1e-9a2b-5f2d1d5a4e10" Scope="perMachine">
    <MajorUpgrade DowngradeErrorMessage="A newer version of SpecTape is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Icon Id="SpecTapeIcon" SourceFile="icon.ico" />
    <Property Id="ARPPRODUCTICON" Value="SpecTapeIcon" />
    <StandardDirectory Id="ProgramFiles6432Folder">
      <Directory Id="INSTALLFOLDER" Name="SpecTape" />
    </StandardDirectory>
    <StandardDirectory Id="ProgramMenuFolder" />
    <ComponentGroup Id="SpecTapeFiles" Directory="INSTALLFOLDER">
      <Component Id="SpecTapeExe" Guid="2f1c7d4a-9b3e-4c55-8a71-3c0f6b9d2a41">
        <File Id="SpecTapeExeFile" Source="spectape.exe" KeyPath="yes">
          <Shortcut Id="StartMenuShortcut" Directory="ProgramMenuFolder" Name="SpecTape"
                    Icon="SpecTapeIcon" Advertise="yes" />
        </File>
        <ProgId Id="SpecTape.tzx" Description="TZX tape image" Icon="SpecTapeExeFile">
          <Extension Id="tzx" ContentType="application/x-tzx">
            <!-- TargetFile is what a non-advertised verb runs; without it WiX0045. -->
            <Verb Id="open" Command="Open" TargetFile="SpecTapeExeFile" Argument="&quot;%1&quot;" />
          </Extension>
        </ProgId>
        <ProgId Id="SpecTape.tap" Description="TAP tape image" Icon="SpecTapeExeFile">
          <Extension Id="tap" ContentType="application/x-tap">
            <Verb Id="open" Command="Open" TargetFile="SpecTapeExeFile" Argument="&quot;%1&quot;" />
          </Extension>
        </ProgId>
      </Component>
    </ComponentGroup>
    <Feature Id="Main">
      <ComponentGroupRef Id="SpecTapeFiles" />
    </Feature>
  </Package>
</Wix>
`;
}

function windowsPackages() {
  const made = [];
  const portable = out(`SpecTape_${version}_x64_portable.exe`);
  copyFileSync(bin, portable);
  made.push(portable);

  // The staging directory is what the .wxs paths are relative to.
  const staging = join(dist, 'msi');
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(staging, { recursive: true });
  copyFileSync(bin, join(staging, 'spectape.exe'));
  copyFileSync(join(icons, 'icon.ico'), join(staging, 'icon.ico'));
  writeFileSync(join(staging, 'spectape.wxs'), wxs());
  const msi = out(`SpecTape_${version}_x64.msi`);
  try {
    run('wix', ['build', 'spectape.wxs', '-arch', 'x64', '-o', msi], { cwd: staging, shell: true });
    made.push(msi);
  } catch (e) {
    console.log(`\nNo .msi: the WiX \`wix\` command would not run (${e.message.split('\n')[0]}).`);
    console.log('Install it with `dotnet tool install --global wix`; the portable exe above needs no installer.');
  }
  return made;
}

// ---- what to say afterwards -----------------------------------------------

const made = [];
if (process.platform === 'darwin') {
  const app = macApp();
  console.log(`Bundle: ${app}`);
  if (packaging) made.push(...macPackages(app));
  console.log(`
Run it — the bundle, so the menu bar says SpecTape:
  "${app}/Contents/MacOS/spectape" "public/samples/SpecTape demo.tzx"

A second tape opens in the right pane:
  "${app}/Contents/MacOS/spectape" tape-a.tzx tape-b.tzx`);
} else if (process.platform === 'win32') {
  if (packaging) made.push(...windowsPackages());
  console.log(`\nRun it with:\n  "${bin}" "public\\samples\\SpecTape demo.tzx"`);
} else {
  if (packaging) made.push(...linuxPackages());
  console.log(`\nRun it with:\n  "${bin}" "public/samples/SpecTape demo.tzx"`);
}

if (made.length) {
  console.log('\nPackaged:');
  for (const f of made) console.log(`  ${f} (${mb(f)} MB)`);
}

// One line, and the binary's own --help carries the flags. Everything that used to
// be printed here was scaffolding for a question that has since been answered: the
// measuring flags for numbers that are now in docs/rust-migration.md, and a symlink
// into /Applications for an "open with" path confirmed on 2026-09-18.
console.log(`
Screenshots, measuring and the rest:
  "${bin}" --help`);
