//! The binary format the wasm build hands parsed tapes to JavaScript in.
//!
//! `src/tzx/wire.ts` decodes it. Both ends are hand written, so the format is
//! deliberately dull: little-endian, fixed widths that match the block model,
//! no alignment and no compression. Fields appear in the order the TypeScript
//! object literals in `parser.ts` declare them, so the decoded objects come out
//! with the same key order as before.
//!
//! ```text
//! u8  WIRE_VERSION
//! u8  status        0 ok, 1 error
//! error:  str message
//! ok:     u8 major, u8 minor
//!         u32 count, then that many str   (warnings)
//!         u32 count, then that many block:
//!           u8 tag        the block ID, or UNKNOWN_TAG
//!           UNKNOWN_TAG:  u8 id, bytes raw
//!           otherwise:    the fields of that block type, in declaration order
//! ```
//!
//! `str` and `bytes` are both a u32 length and that many bytes; `str` holds the
//! Latin-1 bytes of the TZX text, which the decoder turns back into a JS string
//! one char per byte.

use crate::bytes::{ReadResult, Reader};
use crate::parser::ParsedTape;
use crate::types::{ArchiveEntry, Block, Body, HardwareEntry, PilotRun, SelectEntry, SymDef};

pub const WIRE_VERSION: u8 = 1;
const UNKNOWN_TAG: u8 = 0xff;

pub fn encode_tape(t: &ParsedTape) -> Vec<u8> {
    let mut w = Vec::with_capacity(4096);
    w.push(WIRE_VERSION);
    w.push(0); // ok
    w.push(t.major);
    w.push(t.minor);
    u32v(&mut w, t.warnings.len());
    for warning in &t.warnings {
        string(&mut w, warning);
    }
    encode_blocks_into(&mut w, &t.blocks);
    w
}

/// A bare block list, as the app sends one in for writing or measuring.
pub fn encode_blocks(blocks: &[Block]) -> Vec<u8> {
    let mut w = Vec::with_capacity(4096);
    encode_blocks_into(&mut w, blocks);
    w
}

fn encode_blocks_into(w: &mut Vec<u8>, blocks: &[Block]) {
    u32v(w, blocks.len());
    for b in blocks {
        block(w, b);
    }
}

/// The inverse of [`encode_blocks`], for payloads arriving from JavaScript.
pub fn decode_blocks(buf: &[u8]) -> ReadResult<Vec<Block>> {
    let mut r = Reader::new(buf);
    let count = r.u32()? as usize;
    let mut blocks = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        blocks.push(Block::new(read_block(&mut r)?));
    }
    Ok(blocks)
}

/// A byte string answer, such as a serialized tape.
pub fn encode_bytes(b: &[u8]) -> Vec<u8> {
    let mut w = Vec::with_capacity(b.len() + 8);
    w.push(WIRE_VERSION);
    w.push(0);
    bytes(&mut w, b);
    w
}

/// A TAP answer: the bytes plus the indices of the blocks left out.
pub fn encode_tap(b: &[u8], skipped: &[u32]) -> Vec<u8> {
    let mut w = encode_bytes(b);
    u32v(&mut w, skipped.len());
    for i in skipped {
        u32v(&mut w, *i as usize);
    }
    w
}

/// A TZX version answer.
pub fn encode_version(major: u8, minor: u8) -> Vec<u8> {
    vec![WIRE_VERSION, 0, major, minor]
}

fn read_block(r: &mut Reader) -> ReadResult<Body> {
    let tag = r.u8()?;
    if tag == UNKNOWN_TAG {
        let id = r.u8()?;
        return Ok(Body::Unknown { id, raw: read_bytes(r)? });
    }
    Ok(match tag {
        0x10 => Body::Standard { pause: r.u16()?, data: read_bytes(r)? },
        0x11 => Body::Turbo {
            pilot: r.u16()?,
            sync1: r.u16()?,
            sync2: r.u16()?,
            zero: r.u16()?,
            one: r.u16()?,
            pilot_len: r.u16()?,
            used_bits: r.u8()?,
            pause: r.u16()?,
            data: read_bytes(r)?,
        },
        0x12 => Body::PureTone { pulse_len: r.u16()?, count: r.u16()? },
        0x13 => Body::PulseSeq { pulses: read_u16s(r)? },
        0x14 => Body::PureData {
            zero: r.u16()?,
            one: r.u16()?,
            used_bits: r.u8()?,
            pause: r.u16()?,
            data: read_bytes(r)?,
        },
        0x15 => Body::Direct { tstates: r.u16()?, pause: r.u16()?, used_bits: r.u8()?, data: read_bytes(r)? },
        0x18 => Body::Csw {
            pause: r.u16()?,
            sample_rate: r.u32()?,
            compression: r.u8()?,
            pulse_count: r.u32()?,
            data: read_bytes(r)?,
        },
        0x19 => {
            let pause = r.u16()?;
            let totp = r.u32()?;
            let npp = r.u8()?;
            let pilot_symbols = read_sym_defs(r)?;
            let mut pilot_stream = Vec::new();
            for _ in 0..r.u32()? {
                pilot_stream.push(PilotRun { symbol: r.u8()?, reps: r.u16()? });
            }
            Body::Generalized {
                pause,
                totp,
                npp,
                pilot_symbols,
                pilot_stream,
                totd: r.u32()?,
                npd: r.u8()?,
                data_symbols: read_sym_defs(r)?,
                data: read_bytes(r)?,
            }
        }
        0x20 => Body::Pause { pause: r.u16()? },
        0x21 => Body::GroupStart { name: read_str(r)? },
        0x22 => Body::GroupEnd,
        0x23 => Body::Jump { offset: r.i16()? },
        0x24 => Body::LoopStart { count: r.u16()? },
        0x25 => Body::LoopEnd,
        0x26 => {
            let mut offsets = Vec::new();
            for _ in 0..r.u32()? {
                offsets.push(r.i16()?);
            }
            Body::Call { offsets }
        }
        0x27 => Body::Return,
        0x28 => {
            let mut entries = Vec::new();
            for _ in 0..r.u32()? {
                entries.push(SelectEntry { offset: r.i16()?, text: read_str(r)? });
            }
            Body::Select { entries }
        }
        0x2a => Body::Stop48,
        0x2b => Body::SignalLevel { level: r.u8()? },
        0x30 => Body::Text { text: read_str(r)? },
        0x31 => Body::Message { time: r.u8()?, text: read_str(r)? },
        0x32 => {
            let mut entries = Vec::new();
            for _ in 0..r.u32()? {
                entries.push(ArchiveEntry { kind: r.u8()?, text: read_str(r)? });
            }
            Body::Archive { entries }
        }
        0x33 => {
            let mut entries = Vec::new();
            for _ in 0..r.u32()? {
                entries.push(HardwareEntry { kind: r.u8()?, id: r.u8()?, info: r.u8()? });
            }
            Body::Hardware { entries }
        }
        0x35 => Body::Custom { ident: read_str(r)?, data: read_bytes(r)? },
        0x5a => Body::Glue { raw: read_bytes(r)? },
        other => {
            return Err(crate::bytes::ReadError(format!(
                "Wire payload has block tag {other:02x}, which the core does not know"
            )))
        }
    })
}

fn read_bytes(r: &mut Reader) -> ReadResult<Vec<u8>> {
    let n = r.u32()? as usize;
    r.bytes(n)
}

fn read_str(r: &mut Reader) -> ReadResult<String> {
    let n = r.u32()? as usize;
    r.str(n)
}

fn read_u16s(r: &mut Reader) -> ReadResult<Vec<u16>> {
    let n = r.u32()? as usize;
    let mut out = Vec::with_capacity(n.min(1024));
    for _ in 0..n {
        out.push(r.u16()?);
    }
    Ok(out)
}

fn read_sym_defs(r: &mut Reader) -> ReadResult<Vec<SymDef>> {
    let n = r.u32()? as usize;
    let mut out = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        out.push(SymDef { flags: r.u8()?, pulses: read_u16s(r)? });
    }
    Ok(out)
}

pub fn encode_error(message: &str) -> Vec<u8> {
    let mut w = Vec::with_capacity(message.len() + 8);
    w.push(WIRE_VERSION);
    w.push(1); // error
    string(&mut w, message);
    w
}

fn block(w: &mut Vec<u8>, b: &Block) {
    if let Body::Unknown { id, raw } = &b.body {
        w.push(UNKNOWN_TAG);
        w.push(*id);
        bytes(w, raw);
        return;
    }
    w.push(b.id());
    match &b.body {
        Body::Standard { pause, data } => {
            u16v(w, *pause);
            bytes(w, data);
        }
        Body::Turbo { pilot, sync1, sync2, zero, one, pilot_len, used_bits, pause, data } => {
            for v in [*pilot, *sync1, *sync2, *zero, *one, *pilot_len] {
                u16v(w, v);
            }
            w.push(*used_bits);
            u16v(w, *pause);
            bytes(w, data);
        }
        Body::PureTone { pulse_len, count } => {
            u16v(w, *pulse_len);
            u16v(w, *count);
        }
        Body::PulseSeq { pulses } => u16s(w, pulses),
        Body::PureData { zero, one, used_bits, pause, data } => {
            u16v(w, *zero);
            u16v(w, *one);
            w.push(*used_bits);
            u16v(w, *pause);
            bytes(w, data);
        }
        Body::Direct { tstates, pause, used_bits, data } => {
            u16v(w, *tstates);
            u16v(w, *pause);
            w.push(*used_bits);
            bytes(w, data);
        }
        Body::Csw { pause, sample_rate, compression, pulse_count, data } => {
            u16v(w, *pause);
            u32v(w, *sample_rate as usize);
            w.push(*compression);
            u32v(w, *pulse_count as usize);
            bytes(w, data);
        }
        Body::Generalized {
            pause,
            totp,
            npp,
            pilot_symbols,
            pilot_stream,
            totd,
            npd,
            data_symbols,
            data,
        } => {
            u16v(w, *pause);
            u32v(w, *totp as usize);
            w.push(*npp);
            sym_defs(w, pilot_symbols);
            u32v(w, pilot_stream.len());
            for run in pilot_stream {
                w.push(run.symbol);
                u16v(w, run.reps);
            }
            u32v(w, *totd as usize);
            w.push(*npd);
            sym_defs(w, data_symbols);
            bytes(w, data);
        }
        Body::Pause { pause } => u16v(w, *pause),
        Body::GroupStart { name } => string(w, name),
        Body::GroupEnd => {}
        Body::Jump { offset } => u16v(w, *offset as u16),
        Body::LoopStart { count } => u16v(w, *count),
        Body::LoopEnd => {}
        Body::Call { offsets } => {
            u32v(w, offsets.len());
            for o in offsets {
                u16v(w, *o as u16);
            }
        }
        Body::Return => {}
        Body::Select { entries } => {
            u32v(w, entries.len());
            for e in entries {
                u16v(w, e.offset as u16);
                string(w, &e.text);
            }
        }
        Body::Stop48 => {}
        Body::SignalLevel { level } => w.push(*level),
        Body::Text { text } => string(w, text),
        Body::Message { time, text } => {
            w.push(*time);
            string(w, text);
        }
        Body::Archive { entries } => {
            u32v(w, entries.len());
            for e in entries {
                w.push(e.kind);
                string(w, &e.text);
            }
        }
        Body::Hardware { entries } => {
            u32v(w, entries.len());
            for e in entries {
                w.push(e.kind);
                w.push(e.id);
                w.push(e.info);
            }
        }
        Body::Custom { ident, data } => {
            string(w, ident);
            bytes(w, data);
        }
        Body::Glue { raw } => bytes(w, raw),
        Body::Unknown { .. } => unreachable!("handled above"),
    }
}

fn sym_defs(w: &mut Vec<u8>, defs: &[SymDef]) {
    u32v(w, defs.len());
    for d in defs {
        w.push(d.flags);
        u16s(w, &d.pulses);
    }
}

fn u16v(w: &mut Vec<u8>, v: u16) {
    w.extend_from_slice(&v.to_le_bytes());
}

fn u32v(w: &mut Vec<u8>, v: usize) {
    w.extend_from_slice(&(v as u32).to_le_bytes());
}

fn u16s(w: &mut Vec<u8>, v: &[u16]) {
    u32v(w, v.len());
    for n in v {
        u16v(w, *n);
    }
}

fn bytes(w: &mut Vec<u8>, b: &[u8]) {
    u32v(w, b.len());
    w.extend_from_slice(b);
}

fn string(w: &mut Vec<u8>, s: &str) {
    let b = crate::bytes::string_to_latin1(s);
    bytes(w, &b);
}
