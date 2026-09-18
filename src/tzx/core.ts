// Loader for the Rust tape core. The wasm module is inlined as base64 by
// scripts/build-wasm.mjs, so the same code path works in the browser, in the
// Tauri webview and in vitest under node.
//
// Compiling wasm is asynchronous (browsers refuse a synchronous compile of
// anything but a tiny module on the main thread), but the app parses tapes
// synchronously, so `initCore` runs once at startup — see src/main.tsx — and
// the parse functions are sync from then on.
import { CORE_WASM_BASE64 } from './core.wasm';
import { Block, ContentInfo, HeaderInfo, Issue, ParsedTape, Program } from './types';
import {
  decodeBytes, decodeContent, decodeDescribed, decodeF64, decodeHeaderInfo, decodeIssues,
  decodeOptString, decodePrograms, decodeRanges, decodeStrings, decodeTap, decodeTape, decodeU8,
  decodeVersion, encodeBlocks, encodeHeaderInfo, WIRE_VERSION,
} from './wire';

interface CoreExports {
  memory: WebAssembly.Memory;
  core_wire_version(): number;
  core_alloc(len: number): number;
  core_free(ptr: number, len: number): void;
  core_parse_tape(ptr: number, len: number): number;
  core_parse_tzx(ptr: number, len: number): number;
  core_parse_tap(ptr: number, len: number): number;
  core_serialize_tzx(ptr: number, len: number, major: number, minor: number): number;
  core_serialize_tap(ptr: number, len: number): number;
  core_serialize_blocks(ptr: number, len: number): number;
  core_required_version(ptr: number, len: number): number;
  core_save_version(ptr: number, len: number, major: number, minor: number): number;
  core_describe_block(ptr: number, len: number, hex: number): number;
  core_content_labels(ptr: number, len: number): number;
  core_detect_content(ptr: number, len: number, index: number): number;
  core_check_consistency(ptr: number, len: number, base: number): number;
  core_detect_programs(ptr: number, len: number): number;
  core_group_ranges(ptr: number, len: number): number;
  core_tape_title(ptr: number, len: number): number;
  core_decode_header(ptr: number, len: number): number;
  core_encode_header(ptr: number, len: number): number;
  core_checksum(ptr: number, len: number): number;
  core_basic_score(ptr: number, len: number): number;
}

/** Everything but the bookkeeping exports takes a buffer and returns one. */
type Entry = Exclude<keyof CoreExports, 'memory' | 'core_wire_version' | 'core_alloc' | 'core_free'>;

/** What `core_serialize_tzx` and `core_save_version` take for "no version". */
const NO_VERSION = 0xffff;

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

function requireCore(): CoreExports {
  if (core) return core;
  throw new Error(
    failure
      ? `The tape core failed to load: ${failure.message}`
      : 'The tape core is still loading; await initCore() before parsing',
  );
}

/** Hand a buffer to one of the core's entry points and copy the answer out. */
function call(entry: Entry, buf: Uint8Array, ...extra: number[]): Uint8Array {
  const c = requireCore();
  const inPtr = c.core_alloc(buf.length);
  try {
    if (buf.length > 0) new Uint8Array(c.memory.buffer, inPtr, buf.length).set(buf);
    const out = (c[entry] as (...a: number[]) => number)(inPtr, buf.length, ...extra);
    if (out === 0) throw new Error('The tape core could not allocate a result');
    // memory.buffer is replaced when wasm memory grows, so read it again here.
    const len = new DataView(c.memory.buffer).getUint32(out, true);
    const payload = new Uint8Array(c.memory.buffer, out + 4, len).slice();
    c.core_free(out, 4 + len);
    return payload;
  } finally {
    c.core_free(inPtr, buf.length);
  }
}

export function parseTapeCore(buf: Uint8Array): ParsedTape {
  return decodeTape(call('core_parse_tape', buf));
}

export function parseTzxCore(buf: Uint8Array): ParsedTape {
  return decodeTape(call('core_parse_tzx', buf));
}

export function parseTapCore(buf: Uint8Array): ParsedTape {
  return decodeTape(call('core_parse_tap', buf));
}

export function serializeTzxCore(blocks: Block[], version?: { major: number; minor: number }): Uint8Array {
  const v = version ?? { major: NO_VERSION, minor: NO_VERSION };
  return decodeBytes(call('core_serialize_tzx', encodeBlocks(blocks), v.major, v.minor));
}

export function serializeTapCore(blocks: Block[]): { bytes: Uint8Array; skipped: number[] } {
  return decodeTap(call('core_serialize_tap', encodeBlocks(blocks)));
}

/** The blocks' own bytes, ID and body, with no file header. */
export function serializeBlocksCore(blocks: Block[]): Uint8Array {
  return decodeBytes(call('core_serialize_blocks', encodeBlocks(blocks)));
}

export function requiredVersionCore(blocks: Block[]): { major: number; minor: number } {
  return decodeVersion(call('core_required_version', encodeBlocks(blocks, { withData: false })));
}

export function saveVersionCore(
  blocks: Block[],
  loaded: { major: number; minor: number } | null,
): { major: number; minor: number } {
  const l = loaded ?? { major: NO_VERSION, minor: NO_VERSION };
  const payload = encodeBlocks(blocks, { withData: false });
  return decodeVersion(call('core_save_version', payload, l.major, l.minor));
}

/** Description and length column for one block, as the list shows them. */
export function describeBlockCore(block: Block, hex: boolean): { description: string; length: number } {
  return decodeDescribed(call('core_describe_block', encodeBlocks([block]), hex ? 1 : 0));
}

/** Content labels for a whole tape, one call rather than one per row. */
export function contentLabelsCore(blocks: Block[]): string[] {
  return decodeStrings(call('core_content_labels', encodeBlocks(blocks)));
}

/**
 * What a block contains. Only the block and the one before it matter, so that
 * is all that goes over the wire.
 */
export function detectContentCore(blocks: Block[], index: number): ContentInfo {
  const block = blocks[index];
  // Out of range: the core answers with the "nothing detected" default.
  if (!block) return decodeContent(call('core_detect_content', encodeBlocks([]), 0));
  const prev = index > 0 ? blocks[index - 1] : null;
  const window = prev ? [prev, block] : [block];
  return decodeContent(call('core_detect_content', encodeBlocks(window), prev ? 1 : 0));
}

export function checkConsistencyCore(blocks: Block[], base: number): Issue[] {
  return decodeIssues(call('core_check_consistency', encodeBlocks(blocks), base));
}

export function detectProgramsCore(blocks: Block[]): Program[] {
  return decodePrograms(call('core_detect_programs', encodeBlocks(blocks)));
}

export function groupRangesCore(blocks: Block[]): Map<number, number> {
  return decodeRanges(call('core_group_ranges', encodeBlocks(blocks, { withData: false })));
}

export function tapeTitleCore(blocks: Block[]): string | null {
  return decodeOptString(call('core_tape_title', encodeBlocks(blocks, { withData: false })));
}

export function decodeHeaderCore(data: Uint8Array): HeaderInfo | null {
  return decodeHeaderInfo(call('core_decode_header', data));
}

export function encodeHeaderCore(h: HeaderInfo): Uint8Array {
  return decodeBytes(call('core_encode_header', encodeHeaderInfo(h)));
}

export function checksumCore(data: Uint8Array): number {
  return decodeU8(call('core_checksum', data));
}

export function basicScoreCore(data: Uint8Array): number {
  return decodeF64(call('core_basic_score', data));
}
