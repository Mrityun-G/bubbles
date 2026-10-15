//! Keeps `include/bubbles_ffi.h` from drifting away from the Rust side.
//!
//! The header is what native hosts actually compile against, so a mismatch between a macro
//! here and a constant in the crate is a shipped bug that Rust tests would otherwise never
//! see. `scripts/check-ffi-abi.sh` covers the complementary direction: that the exported
//! symbols of the built `cdylib` are exactly the ones declared here.

use bubbles_ffi::{
    BUBBLES_ABI_VERSION, BUBBLES_DONE, BUBBLES_ERR, BUBBLES_OK, BUBBLES_SALIENCY_BLRV,
    BUBBLES_SALIENCY_FIRST_AVAILABLE, BUBBLES_SALIENCY_RANDOM_AVAILABLE, bubbles_abi_version,
};

const HEADER: &str = include_str!("../include/bubbles_ffi.h");

/// Every `extern "C"` entry point this ABI promises, in header order.
///
/// This list *is* the ABI surface: adding to it is additive and fine, removing from it or
/// renaming an entry is a breaking change that requires bumping `BUBBLES_ABI_VERSION`.
const EXPORTED_SYMBOLS: &[&str] = &[
    "bubbles_abi_version",
    "bubbles_last_error",
    "bubbles_string_free",
    "bubbles_copy_utf8",
    "bubbles_compile",
    "bubbles_compile_files",
    "bubbles_program_free",
    "bubbles_program_node_exists",
    "bubbles_program_node_titles_json",
    "bubbles_program_node_tags_json",
    "bubbles_program_variable_declarations_json",
    "bubbles_runner_new",
    "bubbles_runner_new_with_saliency",
    "bubbles_runner_free",
    "bubbles_runner_set_saliency",
    "bubbles_runner_set_locale_json",
    "bubbles_runner_register_function",
    "bubbles_runner_start",
    "bubbles_runner_next_event",
    "bubbles_runner_select_option",
    "bubbles_runner_variable_get_json",
    "bubbles_runner_variable_set_json",
    "bubbles_runner_snapshot_session_json",
    "bubbles_runner_snapshot_storage_json",
    "bubbles_runner_restore_storage_json",
    "bubbles_runner_restore_session_json",
];

/// Reads `#define <name> <value>` from the header, trimming one layer of `(...)` so that
/// `#define BUBBLES_ERR (-1)` parses as `-1`.
fn define(name: &str) -> String {
    let needle = format!("#define {name} ");
    let line = HEADER
        .lines()
        .find(|l| l.starts_with(&needle))
        .unwrap_or_else(|| panic!("header is missing `#define {name}`"));
    let value = line[needle.len()..].trim();
    value
        .strip_prefix('(')
        .and_then(|v| v.strip_suffix(')'))
        .unwrap_or(value)
        .trim()
        .to_owned()
}

fn define_i32(name: &str) -> i32 {
    define(name)
        .parse()
        .unwrap_or_else(|e| panic!("`#define {name}` is not an integer: {e}"))
}

#[test]
fn abi_version_matches_rust() {
    assert_eq!(
        define_i32("BUBBLES_ABI_VERSION"),
        i32::try_from(BUBBLES_ABI_VERSION).unwrap(),
        "BUBBLES_ABI_VERSION in the header disagrees with the Rust constant"
    );
    assert_eq!(
        bubbles_abi_version(),
        BUBBLES_ABI_VERSION,
        "bubbles_abi_version() must return BUBBLES_ABI_VERSION"
    );
}

#[test]
fn header_version_matches_crate_version() {
    let crate_version = env!("CARGO_PKG_VERSION");
    assert_eq!(
        define("BUBBLES_FFI_VERSION_STRING"),
        format!("\"{crate_version}\""),
        "bump BUBBLES_FFI_VERSION_* in include/bubbles_ffi.h to match Cargo.toml"
    );

    let mut parts = crate_version.split('.');
    for (component, field) in ["MAJOR", "MINOR", "PATCH"].iter().zip(&mut parts) {
        assert_eq!(
            define(&format!("BUBBLES_FFI_VERSION_{component}")),
            field,
            "BUBBLES_FFI_VERSION_{component} disagrees with CARGO_PKG_VERSION"
        );
    }

    // The header also states the version in its top comment block, for anyone reading the
    // file rather than compiling it.
    assert!(
        HEADER.contains(&format!("Library version: {crate_version}")),
        "update the `Library version:` line in the header's comment block"
    );
}

#[test]
fn status_codes_match_rust() {
    assert_eq!(define_i32("BUBBLES_OK"), BUBBLES_OK);
    assert_eq!(define_i32("BUBBLES_DONE"), BUBBLES_DONE);
    assert_eq!(define_i32("BUBBLES_ERR"), BUBBLES_ERR);
}

#[test]
fn saliency_constants_match_rust() {
    assert_eq!(
        define_i32("BUBBLES_SALIENCY_FIRST_AVAILABLE"),
        BUBBLES_SALIENCY_FIRST_AVAILABLE
    );
    assert_eq!(define_i32("BUBBLES_SALIENCY_BLRV"), BUBBLES_SALIENCY_BLRV);
    assert_eq!(
        define_i32("BUBBLES_SALIENCY_RANDOM_AVAILABLE"),
        BUBBLES_SALIENCY_RANDOM_AVAILABLE
    );
}

#[test]
fn header_declares_every_exported_symbol() {
    for symbol in EXPORTED_SYMBOLS {
        assert!(
            HEADER.contains(&format!("{symbol}(")),
            "include/bubbles_ffi.h does not declare `{symbol}`"
        );
    }
}

/// `BubblesSourceFile` is `#[repr(C)]` and its layout is frozen by the ABI policy, so the
/// header's field order and types must match the Rust struct exactly. Compared with
/// whitespace collapsed, since the header aligns its columns for readability.
#[test]
fn source_file_layout_matches_rust() {
    let body = HEADER
        .split_once("typedef struct BubblesSourceFile {")
        .expect("header is missing `typedef struct BubblesSourceFile`")
        .1
        .split_once('}')
        .expect("unterminated BubblesSourceFile declaration")
        .0;

    let fields: Vec<String> = body
        .split(';')
        .map(|f| f.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|f| !f.is_empty())
        .collect();

    assert_eq!(
        fields,
        [
            "const char *path_ptr",
            "size_t path_len",
            "const char *text_ptr",
            "size_t text_len",
        ],
        "BubblesSourceFile layout in the header does not match the Rust `#[repr(C)]` struct"
    );
}

/// The callback signature is stable in full, so pin it as a normalised string.
#[test]
fn host_fn_signature_matches_rust() {
    let normalised: String = HEADER.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalised.contains(
            "typedef int32_t (*BubblesHostFn)(void *userdata, const char *args_json_ptr, \
             size_t args_json_len, char **out_result_json);"
        ),
        "the BubblesHostFn typedef in include/bubbles_ffi.h changed; \
         this is a breaking ABI change and needs a BUBBLES_ABI_VERSION bump"
    );
}
