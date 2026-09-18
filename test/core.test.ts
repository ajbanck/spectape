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
import { serializeTzx, serializeTap } from '../src/tzx/writer';
import { Block, CREATABLE_IDS, ParsedTape, createBlock } from '../src/tzx/types';
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
