// Platform adapter: the only place that knows how files get in and out of the app.
// The web build uses <input type=file> and blob downloads; the Tauri desktop build uses
// native dialogs and writes files in place.

export interface OpenedFile {
  name: string;
  /** Absolute path when known (desktop only). Enables "save in place". */
  path?: string;
  bytes: Uint8Array;
}

export interface FileFilter {
  name: string;
  extensions: string[]; // without dots
}

export interface SaveRequest {
  suggestedName: string;
  filters: FileFilter[];
  bytes: Uint8Array;
  /** Write here without asking (desktop). Ignored on the web. */
  path?: string;
  mime?: string;
}

export interface SaveResult {
  name: string;
  path?: string;
}

export interface Platform {
  openFiles(opts: { filters: FileFilter[]; multiple: boolean }): Promise<OpenedFile[]>;
  /** Resolves to null when the user cancelled. */
  saveFile(req: SaveRequest): Promise<SaveResult | null>;
  /** Files the app was launched with or asked to open by the OS (desktop). */
  onOpenWith(cb: (files: OpenedFile[]) => void): void;
  /** Native application menu commands (desktop). */
  onMenu(cb: (id: string) => void): void;
  /** Emulator that auto-detect would start, or null (desktop; always null on the web). */
  detectEmulator(): Promise<string | null>;
  /** Write a TZX to a temp file and open it in the emulator (desktop). Rejects with a message;
   *  'NOT_FOUND' when auto-detect found nothing. Resolves to the program that was started. */
  openInEmulator(req: { bytes: Uint8Array; name: string; program: string; args: string }): Promise<string>;
  /** Let the user pick a program (or macOS app); resolves to its path, null when cancelled. */
  pickProgram(): Promise<string | null>;
  /** Show the state of a native check menu item (desktop). */
  setMenuChecked(id: string, checked: boolean): void;
  /** System clipboard text access for text fields when the native menu is used. */
  readClipboard(): Promise<string>;
  writeClipboard(text: string): Promise<void>;
  /** The UI has painted; the desktop window may be shown (no-op on the web). */
  ready(): void;
  /** Remember the resolved theme so the next window opens in that colour (desktop). */
  rememberTheme(dark: boolean): void;
}

export const isDesktop: boolean = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
/** macOS (or iOS): shortcuts use ⌘ there and Ctrl everywhere else. */
export const isMac: boolean = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.userAgent);

let impl: Promise<Platform> | null = null;

/** Lazily load the right implementation so the web bundle never touches the Tauri API. */
export function platform(): Promise<Platform> {
  if (!impl) {
    impl = isDesktop ? import('./tauri').then((m) => m.tauriPlatform) : import('./web').then((m) => m.webPlatform);
  }
  return impl;
}

export const TAPE_FILTERS: FileFilter[] = [
  { name: 'Tape images', extensions: ['tzx', 'tap'] },
  { name: 'TZX tape image', extensions: ['tzx'] },
  { name: 'TAP tape image', extensions: ['tap'] },
];

export function filtersForName(name: string): FileFilter[] {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  const known: Record<string, string> = { tzx: 'TZX tape image', tap: 'TAP tape image', wav: 'WAV audio', scr: 'Spectrum screen', png: 'PNG image', bin: 'Binary data' };
  if (known[ext]) return [{ name: known[ext], extensions: [ext] }, { name: 'All files', extensions: ['*'] }];
  return [{ name: 'All files', extensions: ['*'] }];
}

/** Wrap a browser File (drag & drop, file input) as an OpenedFile; paths are unknown. */
export async function fileToOpened(f: File): Promise<OpenedFile> {
  return { name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) };
}

export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}
