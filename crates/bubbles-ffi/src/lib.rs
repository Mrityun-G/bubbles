//! C ABI for **bubbles-dialogue** so a non-Rust host can drive `.bub` dialogue over FFI.
//!
//! This crate builds a `cdylib`. Rust callers should depend on
//! [`bubbles-dialogue`](https://docs.rs/bubbles-dialogue) directly — the Rust API exposed here
//! exists so the integration tests and fuzz targets can reach the entry points without loading a
//! shared library, and it is not a supported Rust interface.
//!
//! # The contract
//!
//! The authoritative C declarations are in
//! [`include/bubbles_ffi.h`](https://github.com/ViezeVingertjes/bubbles/blob/main/crates/bubbles-ffi/include/bubbles_ffi.h),
//! and the stability policy — what you may rely on across releases, who frees what, and the event
//! JSON schema — is reproduced in full under [`abi_policy`]. In short:
//!
//! - All text is **UTF-8**. Every input string is a pointer plus a **byte length** (not including a
//!   null terminator); returned strings are NUL-terminated.
//! - Functions are **not thread-safe**. Drive one runner from one thread (Unity's main thread is
//!   fine). Do **not** call back into `bubbles_*` from inside a [`BubblesHostFn`] callback.
//! - Outputs allocated by this library are released with [`bubbles_string_free`] and nothing else.
//! - After [`bubbles_runner_new`], the program handle is consumed and must not be freed or reused.
//! - Check [`bubbles_abi_version`] against [`BUBBLES_ABI_VERSION`] at startup.
//!
//! # JSON events
//!
//! [`bubbles_runner_next_event`] returns a JSON object per [`bubbles::DialogueEvent`]. The `kind`
//! field is always present: `NodeStarted`, `Line`, `Options`, `Command`, `NodeComplete`,
//! `DialogueComplete`, or `Unknown` if a future runtime adds variants this shim does not know yet.
//! Ignore kinds you do not recognise and keep pumping.
//!
//! # Build
//!
//! ```text
//! cargo build -p bubbles-ffi --release
//! ```
//!
//! The shared library is `target/release/libbubbles_ffi.so` (Linux), `libbubbles_ffi.dylib` (macOS),
//! or `bubbles_ffi.dll` (Windows). Copy it next to your host binary and load it — `DllImport` from
//! C#, `dlopen` or a link flag from C. Prebuilt binaries for Windows x64, macOS x64, macOS arm64,
//! and Linux x64 are attached to GitHub releases.
//!
//! Two worked consumer examples live in the repository: a dependency-free C host under
//! `examples/c-host/`, and a .NET 10 P/Invoke smoke test under
//! `crates/bubbles-ffi/tests/dotnet_smoke/`.

#![deny(missing_docs)]
// The workspace denies `unsafe_code`; this crate is the one place that exists to break that
// rule. Every entry point below takes raw pointers from a foreign caller, so the whole crate
// is the unsafe boundary rather than a few marked spots inside it.
#![allow(unsafe_code)]

mod compile_ffi;
mod error;
mod event_json;
mod program_ffi;
mod runner_config;
mod runner_ffi;
mod runner_state;
mod util;
mod value_json;

#[doc = include_str!("../ABI.md")]
pub mod abi_policy {}

use std::ffi::c_char;

pub use compile_ffi::{bubbles_compile, bubbles_compile_files, bubbles_program_free};
pub use error::{bubbles_last_error, bubbles_string_free};
pub use program_ffi::{
    bubbles_program_node_exists, bubbles_program_node_tags_json, bubbles_program_node_titles_json,
    bubbles_program_variable_declarations_json,
};
pub use runner_config::{
    BUBBLES_SALIENCY_BLRV, BUBBLES_SALIENCY_FIRST_AVAILABLE, BUBBLES_SALIENCY_RANDOM_AVAILABLE,
    BubblesHostFn, bubbles_runner_new_with_saliency, bubbles_runner_register_function,
    bubbles_runner_set_locale_json, bubbles_runner_set_saliency,
};
pub use runner_ffi::{
    bubbles_runner_free, bubbles_runner_new, bubbles_runner_next_event,
    bubbles_runner_select_option, bubbles_runner_start,
};
pub use runner_state::{
    bubbles_copy_utf8, bubbles_runner_restore_session_json, bubbles_runner_restore_storage_json,
    bubbles_runner_snapshot_session_json, bubbles_runner_snapshot_storage_json,
    bubbles_runner_variable_get_json, bubbles_runner_variable_set_json,
};

use std::ffi::c_int;

/// Return value: success and an event JSON string was written (see [`bubbles_runner_next_event`]).
pub const BUBBLES_OK: c_int = 0;
/// Return value: [`bubbles_runner_next_event`] has no more events (dialogue finished).
pub const BUBBLES_DONE: c_int = 1;
/// Return value: error; see [`bubbles_last_error`].
pub const BUBBLES_ERR: c_int = -1;

/// One source file for [`bubbles_compile_files`]. Text is UTF-8 bytes; `path` is only used in
/// diagnostics (errors point at the right file).
#[repr(C)]
pub struct BubblesSourceFile {
    /// UTF-8 path bytes (not necessarily NUL-terminated).
    pub path_ptr: *const c_char,
    /// Length of `path_ptr` in bytes.
    pub path_len: usize,
    /// UTF-8 `.bub` source bytes.
    pub text_ptr: *const c_char,
    /// Length of `text_ptr` in bytes.
    pub text_len: usize,
}

/// Version of the C ABI contract described in [`abi_policy`].
///
/// Bumped **only** for a breaking change to that contract, independently of the crate version.
/// Mirrors `BUBBLES_ABI_VERSION` in `include/bubbles_ffi.h`; the `header_matches_rust` test keeps
/// the two from drifting.
pub const BUBBLES_ABI_VERSION: u32 = 1;

/// Returns [`BUBBLES_ABI_VERSION`] for the loaded library.
///
/// The version handshake: a native host calls this at startup and refuses to run if it does not
/// match the `BUBBLES_ABI_VERSION` its bindings were generated against. This is the one symbol
/// guaranteed to exist, keep its signature, and be safe to call before anything else in every
/// release that ships this ABI.
#[unsafe(no_mangle)]
pub const extern "C" fn bubbles_abi_version() -> u32 {
    BUBBLES_ABI_VERSION
}
