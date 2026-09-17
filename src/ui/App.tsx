import { useEffect } from 'preact/hooks';
import { effect } from '@preact/signals';
import { MenuBar } from './MenuBar';
import { Panes } from './TapePane';
import { StatusBar } from './StatusBar';
import { Dialogs } from './Dialogs';
import { DataWindow } from './DataWindow';
import { active, tapes, dialog, dataWindow, setCursor, undo, redo, hex, hexBytes, zeroBased, Side } from '../state/store';
import { openWithFiles } from '../state/files';
import { runCommand, isCommand, KEY_COMMANDS } from '../state/commands';
import { playTape } from '../state/actions';
import { platform } from '../platform';
import { playing, stopPlayback } from '../state/player';

type EditableEl = HTMLInputElement | HTMLTextAreaElement;

function focusedField(): EditableEl | null {
  const el = document.activeElement as HTMLElement | null;
  if (!el) return null;
  if (el.tagName === 'INPUT' && !/^(checkbox|radio|button|file)$/.test((el as HTMLInputElement).type)) return el as HTMLInputElement;
  if (el.tagName === 'TEXTAREA') return el as HTMLTextAreaElement;
  return null;
}

function replaceSelection(el: EditableEl, text: string) {
  const s = el.selectionStart ?? el.value.length;
  const e = el.selectionEnd ?? s;
  el.setRangeText(text, s, e, 'end');
  el.dispatchEvent(new Event('input', { bubbles: true }));
}

/** Commands from the native (desktop) menu. Edit commands go to a focused text field first. */
async function handleNativeMenu(id: string, p: Awaited<ReturnType<typeof platform>>) {
  const field = focusedField();
  if (field) {
    switch (id) {
      case 'undo': document.execCommand('undo'); return;
      case 'redo': document.execCommand('redo'); return;
      case 'select-all': field.select(); return;
      case 'copy':
      case 'cut': {
        const sel = field.value.slice(field.selectionStart ?? 0, field.selectionEnd ?? 0);
        if (sel) {
          await p.writeClipboard(sel);
          if (id === 'cut') replaceSelection(field, '');
        }
        return;
      }
      case 'paste': {
        const text = await p.readClipboard().catch(() => '');
        if (text) replaceSelection(field, text);
        return;
      }
    }
  }
  if (dialog.value || dataWindow.value) {
    if (id === 'undo' || id === 'redo' || id === 'cut' || id === 'copy' || id === 'paste' || id === 'delete' || id === 'select-all') return;
  }
  if (isCommand(id)) runCommand(id, active.value);
}

export function App() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const inField = /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName) || target.isContentEditable;
      const mod = e.metaKey || e.ctrlKey;
      if (e.key === 'Escape') {
        if (dialog.value) dialog.value = null;
        else if (dataWindow.value) dataWindow.value = null;
        return;
      }
      if (dataWindow.value || dialog.value) return; // modal handles its own keys
      const side: Side = active.value;
      const t = tapes[side].value;
      // Global shortcuts that work even inside fields; Shift picks the right-hand tape
      if (mod && !e.altKey) {
        const k = e.key.toLowerCase();
        if (k === 'o') { e.preventDefault(); runCommand('open', e.shiftKey ? 1 : 0); return; }
        if (k === 's') { e.preventDefault(); runCommand('save', e.shiftKey ? 1 : 0); return; }
        if (k === 'z' && !inField) { e.preventDefault(); if (e.shiftKey) redo(side); else undo(side); return; }
        if (k === 'y' && !inField) { e.preventDefault(); redo(side); return; }
      }
      if (inField) return;
      const key = mod ? e.key.toLowerCase() : e.key;
      const bound = KEY_COMMANDS.find((kc) => kc.mod === mod && (kc.shift === undefined || kc.shift === e.shiftKey) && (kc.mod ? kc.key.toLowerCase() : kc.key) === key);
      if (bound) {
        e.preventDefault();
        runCommand(bound.id, side);
        return;
      }
      if (mod) return;
      // Cursor movement and play/stop are not menu commands: they need the modifier state.
      switch (e.key) {
        case 'ArrowUp':
          e.preventDefault();
          if (t.cursor > 0) setCursor(side, t.cursor - 1, e.shiftKey ? 'range' : 'single');
          break;
        case 'ArrowDown':
          e.preventDefault();
          if (t.cursor < t.blocks.length - 1) setCursor(side, t.cursor + 1, e.shiftKey ? 'range' : 'single');
          break;
        case 'Home':
          e.preventDefault();
          setCursor(side, 0);
          break;
        case 'End':
          e.preventDefault();
          setCursor(side, t.blocks.length - 1);
          break;
        case ' ':
          e.preventDefault();
          if (playing.value) stopPlayback();
          else playTape(side, true);
          break;
      }
    };
    window.addEventListener('keydown', onKey);
    const onBeforeUnload = (e: BeforeUnloadEvent) => {
      if (tapes[0].value.dirty || tapes[1].value.dirty) {
        e.preventDefault();
        e.returnValue = '';
      }
    };
    window.addEventListener('beforeunload', onBeforeUnload);
    // Files opened from the OS (desktop file associations / command line)
    let disposeChecks = () => {};
    platform().then((p) => {
      p.onOpenWith(openWithFiles);
      p.onMenu((id) => handleNativeMenu(id, p));
      // Native Options check marks follow the signals (also after in-app or status-bar toggles)
      disposeChecks = effect(() => {
        p.setMenuChecked('toggle-hex', hex.value);
        p.setMenuChecked('opt-hex-bytes', hexBytes.value);
        p.setMenuChecked('opt-zero-based', zeroBased.value);
      });
    });
    // Window title mirrors the active tape
    const disposeTitle = effect(() => {
      const t = tapes[active.value].value;
      document.title = `${t.name}${t.dirty ? ' *' : ''} — SpecTape`;
    });
    return () => {
      disposeTitle();
      disposeChecks();
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('beforeunload', onBeforeUnload);
    };
  }, []);

  return (
    <div class="app" onDragOver={(e) => e.preventDefault()} onDrop={(e) => e.preventDefault()}>
      <MenuBar />
      <Panes />
      <StatusBar />
      <DataWindow />
      <Dialogs />
    </div>
  );
}
