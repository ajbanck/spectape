//! The C ABI the wasm build exposes to JavaScript. No wasm-bindgen: the whole
//! interface is a byte buffer in and a byte buffer out, so the module loads
//! with a plain `WebAssembly.instantiate` and needs no generated glue.
//!
//! JavaScript owns both buffers: it calls [`core_alloc`], writes the tape,
//! calls [`core_parse_tape`], reads the `u32` payload length at the returned
//! pointer followed by that many bytes of [`crate::wire`] payload, then frees
//! both with [`core_free`]. `src/tzx/core.ts` is the other end.

use crate::parser::{parse_tap, parse_tape, parse_tzx, ParsedTape};
use crate::wire::{encode_error, encode_tape, WIRE_VERSION};
use std::alloc::{alloc, dealloc, Layout};

fn layout(len: usize) -> Layout {
    Layout::from_size_align(len, 1).expect("byte layout")
}

/// The wire format this module speaks; `core.ts` refuses a mismatch.
#[no_mangle]
pub extern "C" fn core_wire_version() -> u32 {
    WIRE_VERSION as u32
}

/// `len` bytes for JavaScript to write into. Free it with [`core_free`] and the
/// same length.
#[no_mangle]
pub extern "C" fn core_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    unsafe { alloc(layout(len)) }
}

/// # Safety
/// `ptr` must come from [`core_alloc`] with the same `len`.
#[no_mangle]
pub unsafe extern "C" fn core_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    dealloc(ptr, layout(len));
}

/// Parse a tape, TZX or TAP by signature. Returns a buffer holding a `u32`
/// payload length and that many bytes of wire payload, to be freed with
/// `core_free(ptr, 4 + length)`.
///
/// # Safety
/// `ptr` must point at `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn core_parse_tape(ptr: *const u8, len: usize) -> *mut u8 {
    respond(ptr, len, parse_tape)
}

/// As [`core_parse_tape`], but the file must carry the TZX signature; without
/// it the result is an error payload.
///
/// # Safety
/// `ptr` must point at `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn core_parse_tzx(ptr: *const u8, len: usize) -> *mut u8 {
    respond(ptr, len, parse_tzx)
}

/// As [`core_parse_tape`], but the bytes are always read as TAP.
///
/// # Safety
/// `ptr` must point at `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn core_parse_tap(ptr: *const u8, len: usize) -> *mut u8 {
    respond(ptr, len, |b| Ok(parse_tap(b)))
}

/// Run one of the parsers over the input buffer and lay its result out for
/// JavaScript: a `u32` payload length followed by the payload.
unsafe fn respond(
    ptr: *const u8,
    len: usize,
    parse: impl Fn(&[u8]) -> Result<ParsedTape, String>,
) -> *mut u8 {
    let input: &[u8] = if ptr.is_null() || len == 0 { &[] } else { std::slice::from_raw_parts(ptr, len) };
    let payload = match parse(input) {
        Ok(tape) => encode_tape(&tape),
        Err(message) => encode_error(&message),
    };
    let out = core_alloc(4 + payload.len());
    if out.is_null() {
        return out;
    }
    std::ptr::copy_nonoverlapping((payload.len() as u32).to_le_bytes().as_ptr(), out, 4);
    std::ptr::copy_nonoverlapping(payload.as_ptr(), out.add(4), payload.len());
    out
}
