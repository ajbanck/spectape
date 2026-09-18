// TZX block model. Every block carries a `uid` used by the UI for selection,
// drag & drop and keys. Numbers are plain JS numbers, byte data is Uint8Array.

export interface SymDef {
  flags: number; // b0-b1 polarity
  pulses: number[]; // pulse lengths, shorter symbols padded with 0 when written
}

export interface PilotRun {
  symbol: number;
  reps: number;
}

export interface SelectEntry {
  offset: number; // relative signed
  text: string;
}

export interface ArchiveEntry {
  type: number;
  text: string;
}

export interface HardwareEntry {
  type: number;
  id: number;
  info: number;
}

interface Base {
  uid: number;
}

export interface StandardBlock extends Base {
  id: 0x10;
  pause: number;
  data: Uint8Array;
}
export interface TurboBlock extends Base {
  id: 0x11;
  pilot: number;
  sync1: number;
  sync2: number;
  zero: number;
  one: number;
  pilotLen: number;
  usedBits: number;
  pause: number;
  data: Uint8Array;
}
export interface PureToneBlock extends Base {
  id: 0x12;
  pulseLen: number;
  count: number;
}
export interface PulseSeqBlock extends Base {
  id: 0x13;
  pulses: number[];
}
export interface PureDataBlock extends Base {
  id: 0x14;
  zero: number;
  one: number;
  usedBits: number;
  pause: number;
  data: Uint8Array;
}
export interface DirectBlock extends Base {
  id: 0x15;
  tstates: number;
  pause: number;
  usedBits: number;
  data: Uint8Array;
}
export interface CswBlock extends Base {
  id: 0x18;
  pause: number;
  sampleRate: number;
  compression: number; // 1 RLE, 2 Z-RLE
  pulseCount: number;
  data: Uint8Array;
}
export interface GeneralizedBlock extends Base {
  id: 0x19;
  pause: number;
  totp: number;
  npp: number;
  pilotSymbols: SymDef[]; // length = asp (0 -> 256)
  pilotStream: PilotRun[]; // length = totp
  totd: number;
  npd: number;
  dataSymbols: SymDef[]; // length = asd
  data: Uint8Array;
}
export interface PauseBlock extends Base {
  id: 0x20;
  pause: number;
}
export interface GroupStartBlock extends Base {
  id: 0x21;
  name: string;
}
export interface GroupEndBlock extends Base {
  id: 0x22;
}
export interface JumpBlock extends Base {
  id: 0x23;
  offset: number;
}
export interface LoopStartBlock extends Base {
  id: 0x24;
  count: number;
}
export interface LoopEndBlock extends Base {
  id: 0x25;
}
export interface CallBlock extends Base {
  id: 0x26;
  offsets: number[];
}
export interface ReturnBlock extends Base {
  id: 0x27;
}
export interface SelectBlock extends Base {
  id: 0x28;
  entries: SelectEntry[];
}
export interface Stop48Block extends Base {
  id: 0x2a;
}
export interface SignalLevelBlock extends Base {
  id: 0x2b;
  level: number;
}
export interface TextBlock extends Base {
  id: 0x30;
  text: string;
}
export interface MessageBlock extends Base {
  id: 0x31;
  time: number;
  text: string;
}
export interface ArchiveBlock extends Base {
  id: 0x32;
  entries: ArchiveEntry[];
}
export interface HardwareBlock extends Base {
  id: 0x33;
  entries: HardwareEntry[];
}
export interface CustomBlock extends Base {
  id: 0x35;
  ident: string; // 16 chars, space padded
  data: Uint8Array;
}
export interface GlueBlock extends Base {
  id: 0x5a;
  raw: Uint8Array; // 9 bytes
}
export interface UnknownBlock extends Base {
  id: number;
  unknown: true;
  raw: Uint8Array; // body bytes after the ID byte (including any length prefix)
}

export type Block =
  | StandardBlock
  | TurboBlock
  | PureToneBlock
  | PulseSeqBlock
  | PureDataBlock
  | DirectBlock
  | CswBlock
  | GeneralizedBlock
  | PauseBlock
  | GroupStartBlock
  | GroupEndBlock
  | JumpBlock
  | LoopStartBlock
  | LoopEndBlock
  | CallBlock
  | ReturnBlock
  | SelectBlock
  | Stop48Block
  | SignalLevelBlock
  | TextBlock
  | MessageBlock
  | ArchiveBlock
  | HardwareBlock
  | CustomBlock
  | GlueBlock
  | UnknownBlock;

export type DataBlock = StandardBlock | TurboBlock | PureDataBlock | GeneralizedBlock | DirectBlock;

/** What a parse returns: the blocks, the file's TZX version and any complaints. */
export interface ParsedTape {
  blocks: Block[];
  major: number;
  minor: number;
  warnings: string[];
}

let nextUid = 1;
export function newUid(): number {
  return nextUid++;
}

export function isDataBlock(b: Block): b is DataBlock {
  return b.id === 0x10 || b.id === 0x11 || b.id === 0x14 || b.id === 0x19 || b.id === 0x15;
}

/** Blocks that carry a byte data payload the data window can view/edit. */
export function hasData(b: Block): b is DataBlock | CustomBlock | CswBlock {
  return isDataBlock(b) || b.id === 0x35 || b.id === 0x18;
}

export function isUnknown(b: Block): b is UnknownBlock {
  return (b as UnknownBlock).unknown === true;
}

export const BLOCK_NAMES: Record<number, string> = {
  0x10: 'Standard speed data',
  0x11: 'Turbo speed data',
  0x12: 'Pure tone',
  0x13: 'Pulse sequence',
  0x14: 'Pure data',
  0x15: 'Direct recording',
  0x16: 'C64 ROM type data (deprecated)',
  0x17: 'C64 turbo data (deprecated)',
  0x18: 'CSW recording',
  0x19: 'Generalized data',
  0x20: 'Pause / Stop the tape',
  0x21: 'Group start',
  0x22: 'Group end',
  0x23: 'Jump to block',
  0x24: 'Loop start',
  0x25: 'Loop end',
  0x26: 'Call sequence',
  0x27: 'Return from sequence',
  0x28: 'Select block',
  0x2a: 'Stop the tape if in 48K mode',
  0x2b: 'Set signal level',
  0x30: 'Text description',
  0x31: 'Message',
  0x32: 'Archive info',
  0x33: 'Hardware type',
  0x34: 'Emulation info (deprecated)',
  0x35: 'Custom info',
  0x40: 'Snapshot (deprecated)',
  0x5a: 'Glue',
};

/** Block types the user can create from the UI, in menu order. */
export const CREATABLE_IDS = [
  0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x18, 0x19, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28,
  0x2a, 0x2b, 0x30, 0x31, 0x32, 0x33, 0x35, 0x5a,
];

export const ARCHIVE_TYPES: Record<number, string> = {
  0x00: 'Full title',
  0x01: 'Software house/publisher',
  0x02: 'Author(s)',
  0x03: 'Year of publication',
  0x04: 'Language',
  0x05: 'Game/utility type',
  0x06: 'Price',
  0x07: 'Protection scheme/loader',
  0x08: 'Origin',
  0xff: 'Comment(s)',
};

export const HARDWARE_INFO: string[] = [
  'Runs on this hardware',
  'Uses this hardware',
  'Runs but does not use the hardware',
  'Does not run on this hardware',
];

export const HARDWARE_TYPES: { name: string; ids: string[] }[] = [
  {
    name: 'Computers',
    ids: [
      'ZX Spectrum 16k', 'ZX Spectrum 48k, Plus', 'ZX Spectrum 48k ISSUE 1', 'ZX Spectrum 128k +(Sinclair)',
      'ZX Spectrum 128k +2 (grey case)', 'ZX Spectrum 128k +2A, +3', 'Timex Sinclair TC-2048', 'Timex Sinclair TS-2068',
      'Pentagon 128', 'Sam Coupe', 'Didaktik M', 'Didaktik Gama', 'ZX-80', 'ZX-81', 'ZX Spectrum 128k, Spanish version',
      'ZX Spectrum, Arabic version', 'Microdigital TK 90-X', 'Microdigital TK 95', 'Byte', 'Elwro 800-3', 'ZS Scorpion 256',
      'Amstrad CPC 464', 'Amstrad CPC 664', 'Amstrad CPC 6128', 'Amstrad CPC 464+', 'Amstrad CPC 6128+', 'Jupiter ACE',
      'Enterprise', 'Commodore 64', 'Commodore 128', 'Inves Spectrum+', 'Profi', 'GrandRomMax', 'Kay 1024',
      'Ice Felix HC 91', 'Ice Felix HC 2000', 'Amaterske RADIO Mistrum', 'Quorum 128', 'MicroART ATM', 'MicroART ATM Turbo 2',
      'Chrome', 'ZX Badaloc', 'TS-1500', 'Lambda', 'TK-65', 'ZX-97',
    ],
  },
  {
    name: 'External storage',
    ids: [
      'ZX Microdrive', 'Opus Discovery', 'MGT Disciple', 'MGT Plus-D', 'Rotronics Wafadrive', 'TR-DOS (BetaDisk)',
      'Byte Drive', 'Watsford', 'FIZ', 'Radofin', 'Didaktik disk drives', 'BS-DOS (MB-02)', 'ZX Spectrum +3 disk drive',
      'JLO (Oliger) disk interface', 'Timex FDD3000', 'Zebra disk drive', 'Ramex Millenia', 'Larken',
      'Kempston disk interface', 'Sandy', 'ZX Spectrum +3e hard disk', 'ZXATASP', 'DivIDE', 'ZXCF',
    ],
  },
  {
    name: 'ROM/RAM type add-ons',
    ids: [
      'Sam Ram', 'Multiface ONE', 'Multiface 128k', 'Multiface +3', 'MultiPrint', 'MB-02 ROM/RAM expansion', 'SoftROM',
      '1k', '16k', '48k', 'Memory in 8-16k used',
    ],
  },
  {
    name: 'Sound devices',
    ids: [
      'Classic AY hardware (compatible with 128k ZXs)', 'Fuller Box AY sound hardware', 'Currah microSpeech', 'SpecDrum',
      'AY ACB stereo (A+C=left, B+C=right); Melodik', 'AY ABC stereo (A+B=left, B+C=right)', 'RAM Music Machine', 'Covox',
      'General Sound', 'Intec Electronics Digital Interface B8001', 'Zon-X AY', 'QuickSilva AY', 'Jupiter ACE',
    ],
  },
  { name: 'Joysticks', ids: ['Kempston', 'Cursor, Protek, AGF', 'Sinclair 2 Left (12345)', 'Sinclair 1 Right (67890)', 'Fuller'] },
  { name: 'Mice', ids: ['AMX mouse', 'Kempston mouse'] },
  { name: 'Other controllers', ids: ['Trickstick', 'ZX Light Gun', 'Zebra Graphics Tablet', 'Defender Light Gun'] },
  { name: 'Serial ports', ids: ['ZX Interface 1', 'ZX Spectrum 128k'] },
  {
    name: 'Parallel ports',
    ids: [
      'Kempston S', 'Kempston E', 'ZX Spectrum +3', 'Tasman', "DK'Tronics", 'Hilderbay', 'INES Printerface',
      'ZX LPrint Interface 3', 'MultiPrint', 'Opus Discovery', 'Standard 8255 chip with ports 31,63,95',
    ],
  },
  { name: 'Printers', ids: ['ZX Printer, Alphacom 32 & compatibles', 'Generic printer', 'EPSON compatible'] },
  { name: 'Modems', ids: ['Prism VTX 5000', 'T/S 2050 or Westridge 2050'] },
  { name: 'Digitizers', ids: ['RD Digital Tracer', "DK'Tronics Light Pen", 'British MicroGraph Pad', 'Romantic Robot Videoface'] },
  { name: 'Network adapters', ids: ['ZX Interface 1'] },
  { name: 'Keyboards & keypads', ids: ['Keypad for ZX Spectrum 128k'] },
  { name: 'AD/DA converters', ids: ['Harley Systems ADC 8.2', 'Blackboard Electronics'] },
  { name: 'EPROM programmers', ids: ['Orme Electronics'] },
  { name: 'Graphics', ids: ['WRX Hi-Res', 'G007', 'Memotech', 'Lambda Colour'] },
];

/** Standard ROM loader timings. */
export const ROM_TIMINGS = { pilot: 2168, sync1: 667, sync2: 735, zero: 855, one: 1710, pilotHeader: 8063, pilotData: 3223 };

/** Create a fresh block of the given id with sensible defaults. */
export function createBlock(id: number): Block {
  const uid = newUid();
  switch (id) {
    case 0x10:
      return { uid, id, pause: 1000, data: new Uint8Array(0) };
    case 0x11:
      return {
        uid, id, pilot: ROM_TIMINGS.pilot, sync1: ROM_TIMINGS.sync1, sync2: ROM_TIMINGS.sync2,
        zero: ROM_TIMINGS.zero, one: ROM_TIMINGS.one, pilotLen: ROM_TIMINGS.pilotData, usedBits: 8, pause: 1000,
        data: new Uint8Array(0),
      };
    case 0x12:
      return { uid, id, pulseLen: 2168, count: 8063 };
    case 0x13:
      return { uid, id, pulses: [667, 735] };
    case 0x14:
      return { uid, id, zero: 855, one: 1710, usedBits: 8, pause: 1000, data: new Uint8Array(0) };
    case 0x15:
      return { uid, id, tstates: 79, pause: 0, usedBits: 8, data: new Uint8Array(0) };
    case 0x18:
      return { uid, id, pause: 0, sampleRate: 44100, compression: 1, pulseCount: 0, data: new Uint8Array(0) };
    case 0x19:
      return {
        uid, id, pause: 1000, totp: 0, npp: 2, pilotSymbols: [], pilotStream: [], totd: 0, npd: 2,
        dataSymbols: [{ flags: 0, pulses: [855, 855] }, { flags: 0, pulses: [1710, 1710] }], data: new Uint8Array(0),
      };
    case 0x20:
      return { uid, id, pause: 1000 };
    case 0x21:
      return { uid, id, name: 'Group' };
    case 0x22:
      return { uid, id };
    case 0x23:
      return { uid, id, offset: 1 };
    case 0x24:
      return { uid, id, count: 2 };
    case 0x25:
      return { uid, id };
    case 0x26:
      return { uid, id, offsets: [1] };
    case 0x27:
      return { uid, id };
    case 0x28:
      return { uid, id, entries: [{ offset: 1, text: 'Selection' }] };
    case 0x2a:
      return { uid, id };
    case 0x2b:
      return { uid, id, level: 0 };
    case 0x30:
      return { uid, id, text: '' };
    case 0x31:
      return { uid, id, time: 5, text: '' };
    case 0x32:
      return { uid, id, entries: [{ type: 0, text: '' }] };
    case 0x33:
      return { uid, id, entries: [{ type: 0, id: 1, info: 0 }] };
    case 0x35:
      return { uid, id, ident: 'Custom          ', data: new Uint8Array(0) };
    case 0x5a:
      return { uid, id, raw: new Uint8Array([0x58, 0x54, 0x61, 0x70, 0x65, 0x21, 0x1a, 1, 20]) };
    default:
      return { uid, id, unknown: true, raw: new Uint8Array(0) };
  }
}

/** Deep copy of plain data (objects, arrays, typed arrays). Avoids structuredClone for older web views. */
export function deepClone<T>(v: T): T {
  if (v === null || typeof v !== 'object') return v;
  if (v instanceof Uint8Array) return new Uint8Array(v) as unknown as T;
  if (Array.isArray(v)) return v.map(deepClone) as unknown as T;
  const out: any = {};
  for (const k of Object.keys(v as object)) out[k] = deepClone((v as any)[k]);
  return out as T;
}

/** Deep clone a block, assigning a new uid. */
export function cloneBlock(b: Block): Block {
  const copy: any = deepClone(b);
  copy.uid = newUid();
  return copy as Block;
}
