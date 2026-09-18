// Differential test for stage 1 of docs/rust-migration.md: the wasm core and the
// TypeScript parser it replaced must return the same thing for the same bytes.
// test/reference/parser.ts is that former implementation, frozen.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import * as core from '../src/tzx/parser';
import * as ref from './reference/parser';
import * as coreWriter from '../src/tzx/writer';
import * as refWriter from './reference/writer';
import * as coreDescribe from '../src/tzx/describe';
import * as refDescribe from './reference/describe';
import * as coreContent from '../src/tzx/content';
import * as refContent from './reference/content';
import { checkConsistency as coreCheck } from '../src/tzx/consistency';
import { checkConsistency as refCheck } from './reference/consistency';
import * as corePrograms from '../src/tzx/programs';
import * as refPrograms from './reference/programs';
import { serializeTzx, serializeTap } from '../src/tzx/writer';
import { Block, CREATABLE_IDS, ParsedTape, createBlock, isDataBlock } from '../src/tzx/types';
import { encodeHeader } from '../src/tzx/describe';

/** Blocks compare by content; uids are handed out per parse and always differ. */
function comparable(t: ParsedTape) {
  return {
    major: t.major,
    minor: t.minor,
    warnings: t.warnings,
    blocks: t.blocks.map(({ uid: _uid, ...rest }) => rest),
  };
}

function sameTape(bytes: Uint8Array, parse: 'parseTape' | 'parseTzx' | 'parseTap' = 'parseTape') {
  const got = comparable(core[parse](bytes));
  const want = comparable(ref[parse](bytes));
  // Deep equality covers key order poorly, so check the shapes too.
  expect(got).toEqual(want);
  expect(JSON.stringify(got, replacer)).toEqual(JSON.stringify(want, replacer));
  return got;
}

/** Uint8Array does not survive JSON.stringify on its own. */
function replacer(_k: string, v: unknown) {
  return v instanceof Uint8Array ? Array.from(v) : v;
}

const sample = (name: string) => new Uint8Array(fs.readFileSync(path.resolve('public/samples', name)));

describe('the Rust core against the TypeScript parser it replaced', () => {
  it('agrees on the sample tapes', () => {
    for (const name of fs.readdirSync(path.resolve('public/samples'))) {
      const tape = sameTape(sample(name));
      expect(tape.blocks.length).toBeGreaterThan(0);
      expect(tape.warnings).toEqual([]);
    }
  });

  it('agrees on every creatable block type', () => {
    const blocks: Block[] = CREATABLE_IDS.map((id) => createBlock(id));
    for (const b of blocks) if ('data' in b) (b as any).data = new Uint8Array([0x00, 3, 65, 66, 67, 0x55]);
    const g = blocks.find((b) => b.id === 0x19)!;
    (g as any).totp = 2;
    (g as any).pilotSymbols = [{ flags: 0, pulses: [2168] }, { flags: 1, pulses: [667, 735] }];
    (g as any).pilotStream = [{ symbol: 0, reps: 8063 }, { symbol: 1, reps: 1 }];
    (g as any).totd = 48;
    const bytes = serializeTzx(blocks);
    const tape = sameTape(bytes, 'parseTzx');
    expect(tape.blocks.length).toBe(blocks.length);
    // And the core's blocks still write back to the same file.
    expect(Array.from(serializeTzx(core.parseTzx(bytes).blocks))).toEqual(Array.from(bytes));
  });

  it('agrees on text, entries and high-bit characters', () => {
    const blocks: Block[] = [createBlock(0x21), createBlock(0x30), createBlock(0x31), createBlock(0x32), createBlock(0x33), createBlock(0x35)];
    (blocks[0] as any).name = 'Grüße';           // Latin-1 above 0x7f
    (blocks[1] as any).text = 'Line one\rLine two';
    (blocks[2] as any).text = 'Press \x7fENTER';
    (blocks[3] as any).entries = [{ type: 0, text: 'Full title' }, { type: 0xff, text: 'Comment' }];
    (blocks[4] as any).entries = [{ type: 0, id: 1, info: 0 }, { type: 4, id: 0, info: 3 }];
    (blocks[5] as any).ident = 'POKEs           ';
    sameTape(serializeTzx(blocks), 'parseTzx');
  });

  it('agrees on a select block and a call sequence', () => {
    const blocks: Block[] = [createBlock(0x28), createBlock(0x26), createBlock(0x23)];
    (blocks[0] as any).entries = [{ offset: 3, text: 'Side A' }, { offset: -2, text: 'Side B' }];
    (blocks[1] as any).offsets = [1, -1, 300];
    (blocks[2] as any).offset = -5;
    sameTape(serializeTzx(blocks), 'parseTzx');
  });

  it('agrees on TAP files, including the broken ones', () => {
    const h = encodeHeader({ type: 0, typeName: '', name: 'TEST      ', length: 10, param1: 10, param2: 10 });
    sameTape(new Uint8Array([19, 0, ...h, 3, 0, 0xff, 1, 0xfe]), 'parseTap');
    sameTape(new Uint8Array([4, 0, 1, 2]), 'parseTap');          // truncated block
    sameTape(new Uint8Array([2, 0, 1, 2, 9]), 'parseTap');       // trailing byte
    sameTape(new Uint8Array(0), 'parseTap');                     // empty file
    // Auto-detection sends all of these to the TAP parser too.
    sameTape(new Uint8Array([19, 0, ...h, 3, 0, 0xff, 1, 0xfe]));
  });

  it('agrees on damaged TZX files', () => {
    const sig = Array.from('ZXTape!\x1a').map((c) => c.charCodeAt(0));
    const cases: number[][] = [
      [0x5b, 3, 0, 0, 0, 1, 2, 3, 0x20, 0xe8, 0x03], // unknown block, then a pause
      [0x10, 0xe8, 0x03, 10, 0, 1, 2],               // data block that claims more than it has
      [0x18, 4, 0, 0, 0, 1, 2, 3, 4],                // CSW length below its own header
      [0x34, 1, 2, 3, 4, 5, 6, 7, 8],                // deprecated emulation info
      [0x40, 0, 2, 0, 0, 0xaa, 0xbb],                // deprecated snapshot
      [0x16, 2, 0, 0, 0, 1, 2],                      // deprecated C64 block
      [0x21, 5],                                     // group start with a truncated name
      [0x19, 8, 0, 0, 0, 0xe8, 0x03, 0, 0],          // generalized block cut short
    ];
    for (const body of cases) {
      const file = new Uint8Array([...sig, 1, 20, ...body]);
      const tape = sameTape(file, 'parseTzx');
      expect(tape.blocks.length).toBeGreaterThan(0);
    }
  });

  it('agrees on an empty tape and on a file that is only a header', () => {
    const sig = Array.from('ZXTape!\x1a').map((c) => c.charCodeAt(0));
    sameTape(new Uint8Array([...sig, 1, 20]), 'parseTzx');
    expect(core.isTzx(new Uint8Array([...sig, 1, 20]))).toBe(ref.isTzx(new Uint8Array([...sig, 1, 20])));
  });

  it('refuses a TZX parse of something without the signature', () => {
    const bytes = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    expect(() => core.parseTzx(bytes)).toThrow('Not a TZX file (missing ZXTape! signature)');
    expect(() => ref.parseTzx(bytes)).toThrow('Not a TZX file (missing ZXTape! signature)');
  });

  it('round-trips a TAP export through the core', () => {
    const tape = core.parseTape(sample('SpecTape demo.tap'));
    expect(Array.from(serializeTap(tape.blocks).bytes)).toEqual(Array.from(sample('SpecTape demo.tap')));
  });
});

describe('the Rust writer against the TypeScript writer it replaced', () => {
  /** Every creatable block, with content in the ones that carry data. */
  function everyBlock(): Block[] {
    const blocks: Block[] = CREATABLE_IDS.map((id) => createBlock(id));
    for (const b of blocks) if ('data' in b) (b as any).data = new Uint8Array([0x00, 3, 65, 66, 67, 0x55]);
    const g = blocks.find((b) => b.id === 0x19)!;
    (g as any).totp = 2;
    (g as any).pilotSymbols = [{ flags: 0, pulses: [2168] }, { flags: 1, pulses: [667, 735] }];
    (g as any).pilotStream = [{ symbol: 0, reps: 8063 }, { symbol: 1, reps: 1 }];
    (g as any).totd = 48;
    return blocks;
  }

  /** Blocks the old writer had to paper over: over-long text, short idents, odd glue. */
  function awkwardBlocks(): Block[] {
    const blocks: Block[] = [
      createBlock(0x21), createBlock(0x30), createBlock(0x31), createBlock(0x28),
      createBlock(0x32), createBlock(0x33), createBlock(0x35), createBlock(0x5a),
      createBlock(0x19), createBlock(0x13), createBlock(0x26),
    ];
    (blocks[0] as any).name = 'x'.repeat(300);
    (blocks[1] as any).text = 'y'.repeat(300);
    (blocks[2] as any).text = 'Grüße\r\nfrom 1982';
    (blocks[3] as any).entries = [{ offset: 2, text: 'z'.repeat(300) }, { offset: -3, text: '' }];
    (blocks[4] as any).entries = [{ type: 4, text: 'en' }, { type: 0xff, text: 'two\nlines' }];
    (blocks[5] as any).entries = [{ type: 0, id: 0x1c, info: 1 }, { type: 0x10, id: 0, info: 0 }];
    (blocks[6] as any).ident = 'POKEs';
    (blocks[7] as any).raw = new Uint8Array([1, 2]);
    Object.assign(blocks[8], {
      pause: 0, totp: 0, npp: 2, pilotSymbols: [], pilotStream: [], totd: 16, npd: 3,
      dataSymbols: [{ flags: 0, pulses: [855] }, { flags: 0, pulses: [] }],
      data: new Uint8Array([0xaa]),
    });
    (blocks[9] as any).pulses = [667, 735, 1000];
    (blocks[10] as any).offsets = [1, -1, 300];
    return blocks;
  }

  function sameOutput(blocks: Block[], version?: { major: number; minor: number }) {
    expect(Array.from(coreWriter.serializeTzx(blocks, version))).toEqual(Array.from(refWriter.serializeTzx(blocks, version)));
    const coreTap = coreWriter.serializeTap(blocks);
    const refTap = refWriter.serializeTap(blocks);
    expect(Array.from(coreTap.bytes)).toEqual(Array.from(refTap.bytes));
    expect(coreTap.skipped).toEqual(refTap.skipped);
    expect(coreWriter.requiredVersion(blocks)).toEqual(refWriter.requiredVersion(blocks));
    for (const loaded of [null, { major: 1, minor: 0 }, { major: 1, minor: 20 }]) {
      expect(coreWriter.saveVersion(blocks, loaded)).toEqual(refWriter.saveVersion(blocks, loaded));
    }
    for (const b of blocks) {
      expect(Array.from(coreWriter.serializeBlock(b))).toEqual(Array.from(refWriter.serializeBlock(b)));
    }
  }

  it('writes every creatable block the same way', () => {
    sameOutput(everyBlock());
    sameOutput(everyBlock(), { major: 1, minor: 20 });
  });

  it('writes the awkward blocks the same way', () => {
    sameOutput(awkwardBlocks());
  });

  it('writes the sample tapes the same way', () => {
    for (const name of fs.readdirSync(path.resolve('public/samples'))) {
      sameOutput(core.parseTape(sample(name)).blocks);
    }
  });

  it('writes an unknown block and an empty tape the same way', () => {
    const unknown = core.parseTzx(new Uint8Array([
      ...Array.from('ZXTape!\x1a').map((c) => c.charCodeAt(0)), 1, 20, 0x5b, 3, 0, 0, 0, 1, 2, 3,
    ])).blocks;
    sameOutput(unknown);
    sameOutput([]);
  });

  it('round-trips the sample tapes byte for byte', () => {
    for (const name of fs.readdirSync(path.resolve('public/samples'))) {
      const bytes = sample(name);
      const tape = core.parseTape(bytes);
      const out = name.endsWith('.tap')
        ? coreWriter.serializeTap(tape.blocks).bytes
        : coreWriter.serializeTzx(tape.blocks, { major: tape.major, minor: tape.minor });
      expect(Array.from(out)).toEqual(Array.from(bytes));
    }
  });
});

describe('the Rust descriptions, detection and structure against the TypeScript they replaced', () => {
  const sig = Array.from('ZXTape!\x1a').map((c) => c.charCodeAt(0));
  const tzx = (...body: number[]) => new Uint8Array([...sig, 1, 20, ...body]);

  /** Every creatable block plus a few with realistic content. */
  function blocksForDescription(): Block[] {
    const blocks: Block[] = CREATABLE_IDS.map((id) => createBlock(id));
    for (const b of blocks) if ('data' in b) (b as any).data = new Uint8Array([0x00, 3, 65, 66, 67, 0x55]);
    const header = (type: number, name: string, length: number, p1: number, p2: number) =>
      new Uint8Array([0x00, ...encodeHeader({ type, typeName: '', name, length, param1: p1, param2: p2 }).subarray(1, 18), 0x55]);
    return [
      ...blocks,
      { ...createBlock(0x10), data: header(0, 'demo      ', 131, 131, 20) } as Block,
      { ...createBlock(0x10), data: header(3, 'demo.scr  ', 6912, 16384, 32768) } as Block,
      { ...createBlock(0x11), data: header(1, 'nums      ', 40, 0x4100, 0) } as Block,
      { ...createBlock(0x14), data: header(2, 'chars     ', 40, 0x0100, 0) } as Block,
      { ...createBlock(0x21), name: 'Machine code' } as Block,
      { ...createBlock(0x23), offset: -3 } as Block,
      { ...createBlock(0x30), text: 'Two\r\nlines' } as Block,
      { ...createBlock(0x31), time: 3, text: 'Grüße\nfrom 1982' } as Block,
      { ...createBlock(0x35), ident: '  POKEs  ' } as Block,
      { ...createBlock(0x20), pause: 0 } as Block,
      { ...createBlock(0x2b), level: 1 } as Block,
      ...core.parseTzx(tzx(0x5b, 3, 0, 0, 0, 1, 2, 3, 0x34, 1, 2, 3, 4, 5, 6, 7, 8)).blocks,
    ];
  }

  it('describes every block the same way, in both number bases', () => {
    for (const b of blocksForDescription()) {
      for (const hex of [false, true]) {
        expect(coreDescribe.describeBlock(b, hex)).toBe(refDescribe.describeBlock(b, hex));
      }
      expect(coreDescribe.blockLength(b)).toBe(refDescribe.blockLength(b));
      expect(coreDescribe.isMetadata(b)).toBe(refDescribe.isMetadata(b));
    }
  });

  it('describes the blocks of the sample tapes the same way', () => {
    for (const name of fs.readdirSync(path.resolve('public/samples'))) {
      for (const b of core.parseTape(sample(name)).blocks) {
        expect(coreDescribe.describeBlock(b, false)).toBe(refDescribe.describeBlock(b, false));
        expect(coreDescribe.describeBlock(b, true)).toBe(refDescribe.describeBlock(b, true));
        expect(coreDescribe.blockLength(b)).toBe(refDescribe.blockLength(b));
      }
    }
  });

  it('decodes and encodes ROM headers the same way', () => {
    for (const type of [0, 1, 2, 3, 4]) {
      const h = { type, typeName: '', name: 'name      ', length: 100, param1: 32768, param2: 10 };
      const bytes = refDescribe.encodeHeader(h);
      expect(Array.from(coreDescribe.encodeHeader(h))).toEqual(Array.from(bytes));
      expect(coreDescribe.decodeHeader(bytes)).toEqual(refDescribe.decodeHeader(bytes));
      expect(coreDescribe.checksum(bytes)).toBe(refDescribe.checksum(bytes));
      expect(coreDescribe.checksum(bytes, 1, 5)).toBe(refDescribe.checksum(bytes, 1, 5));
    }
    // Not a header: wrong length, wrong flag, unknown type.
    for (const bad of [new Uint8Array(0), new Uint8Array(19).fill(9), new Uint8Array(18)]) {
      expect(coreDescribe.decodeHeader(bad)).toEqual(refDescribe.decodeHeader(bad));
    }
  });

  it('detects the same content for every block of every sample tape', () => {
    for (const name of fs.readdirSync(path.resolve('public/samples'))) {
      const blocks = core.parseTape(sample(name)).blocks;
      const labels = coreContent.contentLabels(blocks);
      blocks.forEach((b, i) => {
        expect(coreContent.detectContent(blocks, i)).toEqual(refContent.detectContent(blocks, i));
        const refLabel = isDataBlock(b) ? refContent.detectContent(blocks, i).label : '';
        expect(labels[i]).toBe(refLabel);
      });
    }
  });

  it('detects the same content for the awkward cases', () => {
    const withFlag = (body: number[]) => new Uint8Array([0xff, ...body, 0]);
    const std = (data: Uint8Array) => ({ ...createBlock(0x10), data } as Block);
    const hdr = (type: number, length: number, p1: number) =>
      std(new Uint8Array([0x00, ...encodeHeader({ type, typeName: '', name: 'x         ', length, param1: p1, param2: 0 }).subarray(1, 18), 0x55]));
    const cases: Block[][] = [
      [hdr(3, 6912, 16384), std(withFlag(new Array(6912).fill(0)))],
      [hdr(3, 6912, 40000), std(withFlag(new Array(6912).fill(0)))],
      [hdr(3, 6912, 16384), std(withFlag(new Array(100).fill(0)))],       // short
      [hdr(3, 120, 32768), std(withFlag(new Array(130).fill(0)))],        // long
      [hdr(0, 40, 0), std(withFlag(new Array(40).fill(0)))],              // BASIC
      [hdr(1, 40, 0x4100), std(withFlag(new Array(40).fill(0)))],         // number array
      [hdr(2, 40, 0x0100), std(withFlag(new Array(40).fill(0)))],         // char array
      [std(withFlag([0x00, 0x0a, 0x05, 0x00, 1, 2, 3, 4, 0x0d]))],        // BASIC by heuristic
      [std(withFlag(new Array(6912).fill(1)))],                           // screen by heuristic
      [std(withFlag([0xc3, 0x00, 0x80, 0x21, 0x00, 0x40, 0x11]))],        // plain data
      [std(new Uint8Array(0))],                                           // empty
      [{ ...createBlock(0x14), data: new Uint8Array([1, 2, 3]) } as Block],
      [createBlock(0x20)],                                                // not a data block
    ];
    for (const blocks of cases) {
      blocks.forEach((_b, i) => {
        expect(coreContent.detectContent(blocks, i)).toEqual(refContent.detectContent(blocks, i));
      });
      expect(coreContent.contentLabels(blocks)).toEqual(
        blocks.map((b, i) => (isDataBlock(b) ? refContent.detectContent(blocks, i).label : '')),
      );
      expect(coreContent.basicScore(blocks[0].id === 0x10 ? (blocks[0] as any).data : new Uint8Array(0)))
        .toBe(refContent.basicScore(blocks[0].id === 0x10 ? (blocks[0] as any).data : new Uint8Array(0)));
    }
    // Out of range indices answer with the default.
    expect(coreContent.detectContent([], 0)).toEqual(refContent.detectContent([], 0));
    expect(coreContent.detectContent(cases[0], 9)).toEqual(refContent.detectContent(cases[0], 9));
  });

  it('finds the same consistency issues', () => {
    const b = (id: number, fields: Record<string, unknown> = {}) => Object.assign(createBlock(id), fields) as Block;
    const cases: Block[][] = [
      [],
      [b(0x21), b(0x24), b(0x22)],                                   // crossing
      [b(0x21), b(0x21), b(0x22), b(0x22)],                          // nested group
      [b(0x24, { count: 0 }), b(0x25), b(0x24, { count: 1 }), b(0x25)],
      [b(0x22), b(0x25)],                                            // ends without starts
      [b(0x23, { offset: 0 })],                                      // jump to itself
      [b(0x23, { offset: 99 })],                                     // outside the tape
      [b(0x26, { offsets: [] }), b(0x26, { offsets: [0, 99] })],
      [b(0x28, { entries: [{ offset: 99, text: 'x' }] })],
      [b(0x11, { usedBits: 0, data: new Uint8Array(0) })],
      [b(0x15, { usedBits: 9, tstates: 0 })],
      [b(0x10, { data: new Uint8Array([0x00, 1, 2, 3]) })],           // bad checksum
      [b(0x12, { count: 0 }), b(0x13, { pulses: [] })],
      [b(0x19, { totp: 2, pilotStream: [{ symbol: 5, reps: 1 }], pilotSymbols: [], totd: 8, dataSymbols: [], data: new Uint8Array(0) })],
      [b(0x32, { entries: [] }), b(0x33, { entries: [] })],
      [b(0x24, { count: 3 }), b(0x12), b(0x25), b(0x20)],             // a healthy loop
      [b(0x26, { offsets: [1] }), b(0x12), b(0x27)],                  // a call that returns
      [b(0x26, { offsets: [1] }), b(0x12)],                           // a call that never returns
      [b(0x21)],                                                      // never closed
      [b(0x23, { offset: 1 }), b(0x23, { offset: -1 })],              // infinite jump loop
    ];
    for (const blocks of cases) {
      for (const base of [0, 1]) expect(coreCheck(blocks, base)).toEqual(refCheck(blocks, base));
    }
    for (const name of fs.readdirSync(path.resolve('public/samples'))) {
      const blocks = core.parseTape(sample(name)).blocks;
      expect(coreCheck(blocks)).toEqual(refCheck(blocks));
    }
  });

  // The two predicates the TypeScript still implements itself. The same tables
  // are asserted in core/tests/logic.rs, so the copies cannot drift apart.
  it('classifies metadata blocks the way the core does', () => {
    const metadata = [0x21, 0x22, 0x30, 0x31, 0x32, 0x33, 0x35, 0x5a];
    for (const id of CREATABLE_IDS) {
      expect(coreDescribe.isMetadata(createBlock(id))).toBe(metadata.includes(id));
    }
    const unknown = (id: number) => core.parseTzx(tzx(id, 0, 0, 0, 0)).blocks[0];
    expect(coreDescribe.isMetadata(unknown(0x5b))).toBe(true);
    expect(coreDescribe.isMetadata(unknown(0x16))).toBe(false);
    expect(coreDescribe.isMetadata(unknown(0x17))).toBe(false);
  });

  it('strips flag and checksum bytes the way the core does', () => {
    const d = new Uint8Array([0xff, 1, 2, 3, 0x55]);
    expect(Array.from(coreContent.blockBody(d, true, true))).toEqual([1, 2, 3]);
    expect(Array.from(coreContent.blockBody(d, true, false))).toEqual([1, 2, 3, 0x55]);
    expect(Array.from(coreContent.blockBody(d, false, true))).toEqual([0xff, 1, 2, 3]);
    expect(Array.from(coreContent.blockBody(d, false, false))).toEqual([0xff, 1, 2, 3, 0x55]);
    expect(Array.from(coreContent.blockBody(new Uint8Array(0), true, true))).toEqual([]);
    expect(Array.from(coreContent.blockBody(new Uint8Array([7]), true, true))).toEqual([]);
  });

  it('finds the same groups, programs and titles', () => {
    const b = (id: number, fields: Record<string, unknown> = {}) => Object.assign(createBlock(id), fields) as Block;
    const prog = (name: string) =>
      b(0x10, { data: new Uint8Array([0x00, ...encodeHeader({ type: 0, typeName: '', name, length: 10, param1: 0, param2: 10 }).subarray(1, 18), 0x55]) });
    const cases: Block[][] = [
      [],
      [prog('A         '), b(0x10), prog('B         '), b(0x10)],
      [b(0x21, { name: 'Game' }), prog('A         '), b(0x22), b(0x21, { name: 'Empty' }), b(0x22)],
      [b(0x21), b(0x24), b(0x25), b(0x22)],
      [b(0x24), b(0x21), b(0x22), b(0x25), b(0x21), b(0x22)],
      [b(0x28, { entries: [{ offset: 2, text: 'Side B' }, { offset: 99, text: 'Nope' }] }), b(0x10), prog('C         ')],
      [b(0x32, { entries: [{ type: 0, text: 'The Tape' }] }), b(0x10)],
      [b(0x32, { entries: [{ type: 1, text: 'No title' }] }), b(0x10)],
      [b(0x30, { text: 'lead in' }), prog('A         '), b(0x20), prog('B         ')],
      [b(0x21, { name: '' }), prog('          '), b(0x22)],
    ];
    for (const blocks of cases) {
      expect([...corePrograms.groupRanges(blocks)]).toEqual([...refPrograms.groupRanges(blocks)]);
      expect(corePrograms.detectPrograms(blocks)).toEqual(refPrograms.detectPrograms(blocks));
      expect(corePrograms.tapeTitle(blocks)).toEqual(refPrograms.tapeTitle(blocks));
    }
    for (const name of fs.readdirSync(path.resolve('public/samples'))) {
      const blocks = core.parseTape(sample(name)).blocks;
      expect([...corePrograms.groupRanges(blocks)]).toEqual([...refPrograms.groupRanges(blocks)]);
      expect(corePrograms.detectPrograms(blocks)).toEqual(refPrograms.detectPrograms(blocks));
      expect(corePrograms.tapeTitle(blocks)).toEqual(refPrograms.tapeTitle(blocks));
    }
  });
});
