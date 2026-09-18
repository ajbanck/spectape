// Loader for the Rust tape core. The wasm module is inlined as base64 by
// scripts/build-wasm.mjs, so the same code path works in the browser, in the
// Tauri webview and in vitest under node.
//
// Compiling wasm is asynchronous (browsers refuse a synchronous compile of
// anything but a tiny module on the main thread), but the app parses tapes
// synchronously, so `initCore` runs once at startup — see src/main.tsx — and
// the parse functions are sync from then on.
import { CORE_WASM_BASE64 } from './core.wasm';
import { ParsedTape } from './types';
import { decodeTape, WIRE_VERSION } from './wire';

interface CoreExports {
  memory: WebAssembly.Memory;
  core_wire_version(): number;
  core_alloc(len: number): number;
  core_free(ptr: number, len: number): void;
  core_parse_tape(ptr: number, len: number): number;
  core_parse_tzx(ptr: number, len: number): number;
  core_parse_tap(ptr: number, len: number): number;
}

type Entry = 'core_parse_tape' | 'core_parse_tzx' | 'core_parse_tap';

let core: CoreExports | null = null;
let loading: Promise<void> | null = null;
let failure: Error | null = null;

// Typed as backed by an ArrayBuffer so it satisfies BufferSource.
function wasmBytes(): Uint8Array<ArrayBuffer> {
  const binary = atob(CORE_WASM_BASE64);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
  return out;
}

/** Compile and instantiate the core. Safe to call repeatedly; only the first call works. */
export function initCore(): Promise<void> {
  if (core) return Promise.resolve();
  if (!loading) {
    loading = WebAssembly.instantiate(wasmBytes()).then(
      ({ instance }) => {
        const exports = instance.exports as unknown as CoreExports;
        const version = exports.core_wire_version();
        if (version !== WIRE_VERSION) {
          throw new Error(`Tape core speaks wire format ${version}, this build expects ${WIRE_VERSION}`);
        }
        core = exports;
      },
      (e: unknown) => {
        // Remember why, so a later parse can say something better than "still loading".
        failure = e instanceof Error ? e : new Error(String(e));
        throw failure;
      },
    );
  }
  return loading;
}

export function coreReady(): boolean {
  return core !== null;
}

/** Hand the bytes to the core and decode what comes back. */
function run(entry: Entry, buf: Uint8Array): ParsedTape {
  const c = core;
  if (!c) {
    throw new Error(
      failure
        ? `The tape core failed to load: ${failure.message}`
        : 'The tape core is still loading; await initCore() before parsing',
    );
  }
  const inPtr = c.core_alloc(buf.length);
  try {
    if (buf.length > 0) new Uint8Array(c.memory.buffer, inPtr, buf.length).set(buf);
    const out = c[entry](inPtr, buf.length);
    if (out === 0) throw new Error('The tape core could not allocate a result');
    // memory.buffer is replaced when wasm memory grows, so read it again here.
    const len = new DataView(c.memory.buffer).getUint32(out, true);
    const payload = new Uint8Array(c.memory.buffer, out + 4, len).slice();
    c.core_free(out, 4 + len);
    return decodeTape(payload);
  } finally {
    c.core_free(inPtr, buf.length);
  }
}

export function parseTapeCore(buf: Uint8Array): ParsedTape {
  return run('core_parse_tape', buf);
}

export function parseTzxCore(buf: Uint8Array): ParsedTape {
  return run('core_parse_tzx', buf);
}

export function parseTapCore(buf: Uint8Array): ParsedTape {
  return run('core_parse_tap', buf);
}
