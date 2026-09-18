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

use crate::parser::ParsedTape;
use crate::types::{Block, Body, SymDef};

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
    u32v(&mut w, t.blocks.len());
    for b in &t.blocks {
        block(&mut w, b);
    }
    w
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
