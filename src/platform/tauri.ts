import { open, save } from '@tauri-apps/plugin-dialog';
import { readFile, writeFile } from '@tauri-apps/plugin-fs';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager';
import { Platform, OpenedFile, baseName } from './index';

async function read(path: string): Promise<OpenedFile> {
  return { name: baseName(path), path, bytes: await readFile(path) };
}

export const tauriPlatform: Platform = {
  async openFiles({ filters, multiple }) {
    const sel = await open({ multiple, directory: false, filters: filters.filter((f) => !f.extensions.includes('*')) });
    if (!sel) return [];
    const paths = Array.isArray(sel) ? sel : [sel];
    return Promise.all(paths.map(read));
  },

  async saveFile({ suggestedName, filters, bytes, path }) {
    let target = path;
    if (!target) {
      target = (await save({ defaultPath: suggestedName, filters: filters.filter((f) => !f.extensions.includes('*')) })) ?? undefined;
      if (!target) return null;
    }
    await writeFile(target, bytes);
    return { name: baseName(target), path: target };
  },

  onOpenWith(cb) {
    // The Rust side queues paths from argv (Windows/Linux) and from macOS "Opened" events,
    // and emits `open-files` whenever new ones arrive. Draining through a command avoids
    // losing files that arrive before this listener exists.
    const drain = async () => {
      const paths = await invoke<string[]>('take_pending_files');
      if (paths.length) cb(await Promise.all(paths.map(read)));
    };
    listen('open-files', drain);
    drain();
  },

  onMenu(cb) {
    listen<string>('menu', (e) => cb(e.payload));
  },
  setMenuChecked(id, checked) {
    invoke('set_menu_checked', { id, checked });
  },
  async pickProgram() {
    const mac = navigator.userAgent.includes('Mac');
    const win = navigator.userAgent.includes('Windows');
    const sel = await open({
      multiple: false,
      directory: false,
      title: 'Choose the emulator',
      defaultPath: mac ? '/Applications' : undefined,
      filters: mac ? [{ name: 'Applications', extensions: ['app'] }] : win ? [{ name: 'Programs', extensions: ['exe'] }] : [],
    });
    return typeof sel === 'string' ? sel : null;
  },
  detectEmulator: () => invoke<string | null>('detect_emulator'),
  openInEmulator: ({ bytes, name, program, args }) => invoke<string>('open_in_emulator', { bytes: Array.from(bytes), name, program, args }),
  readClipboard: () => readText(),
  writeClipboard: (t) => writeText(t),
};
