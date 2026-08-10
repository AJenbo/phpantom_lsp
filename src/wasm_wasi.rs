//! WASI reactor exports for the browser.
//!
//! The module is instantiated once (a reactor: `_initialize`, no `_start`) and
//! then driven through [`lsp_handle`], which takes one LSP JSON-RPC message and
//! returns the response. Strings are marshalled by hand through linear memory
//! ([`lsp_alloc`]/[`lsp_dealloc`]) because this target does not use
//! wasm-bindgen.
//!
//! The dispatcher persists across calls in a thread-local. wasm is
//! single-threaded here, so that is effectively a global.

use crate::lsp_dispatch::LspDispatcher;
use std::cell::Cell;

thread_local! {
    static DISPATCHER: LspDispatcher = LspDispatcher::new();

    /// Length of the buffer returned by the last [`lsp_handle`] call. Kept
    /// separate so `lsp_handle` can return a plain pointer (`u32`) rather than
    /// a packed `u64`, which JS would surface as a `BigInt`.
    static LAST_LEN: Cell<usize> = const { Cell::new(0) };
}

/// Length in bytes of the buffer returned by the most recent [`lsp_handle`].
#[unsafe(no_mangle)]
pub extern "C" fn lsp_response_len() -> usize {
    LAST_LEN.with(|c| c.get())
}

/// Allocate `len` bytes in wasm linear memory and return the pointer. The host
/// fills it (a UTF-8 JSON-RPC message) and passes it to [`lsp_handle`].
///
/// Returns a null pointer for `len == 0`, which [`lsp_dealloc`] ignores.
#[unsafe(no_mangle)]
pub extern "C" fn lsp_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    // A boxed slice has capacity exactly `len`, so `lsp_dealloc` can rebuild
    // the original allocation from the pointer and length alone. `Vec` is not
    // usable here: `with_capacity` may over-allocate, and freeing with the
    // wrong capacity is undefined behaviour.
    into_raw(vec![0u8; len].into_boxed_slice())
}

/// Free a buffer previously returned by [`lsp_alloc`] or [`lsp_handle`].
///
/// # Safety
/// `ptr` and `len` must come from a single prior `lsp_alloc`/`lsp_handle` call
/// and must not have been freed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsp_dealloc(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
}

/// Handle one LSP JSON-RPC message located at `ptr`/`len`.
///
/// Returns a pointer to the response bytes, whose length is read separately via
/// [`lsp_response_len`]; the host reads them and then frees them with
/// [`lsp_dealloc`]. Returns a null pointer when there is no response (the
/// message was a notification).
///
/// # Safety
/// `ptr`/`len` must describe a readable buffer of `len` bytes, as returned by
/// [`lsp_alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsp_handle(ptr: *const u8, len: usize) -> *mut u8 {
    let message = if ptr.is_null() || len == 0 {
        String::new()
    } else {
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(ptr, len) }).into_owned()
    };

    let response = DISPATCHER.with(|d| d.handle(&message));

    match response {
        None => {
            LAST_LEN.with(|c| c.set(0));
            std::ptr::null_mut()
        }
        Some(text) => {
            let bytes = text.into_bytes().into_boxed_slice();
            LAST_LEN.with(|c| c.set(bytes.len()));
            into_raw(bytes)
        }
    }
}

/// Hand ownership of a boxed byte slice to the host.
fn into_raw(bytes: Box<[u8]>) -> *mut u8 {
    Box::into_raw(bytes).cast::<u8>()
}
