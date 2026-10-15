# C ABI stability policy

This document is the contract between `bubbles-ffi` and native hosts (Unity, Godot .NET,
C, C++, any P/Invoke caller). It states what you may rely on across releases and what may
change under you.

The authoritative declaration of the surface is
[`include/bubbles_ffi.h`](https://github.com/ViezeVingertjes/bubbles/blob/main/crates/bubbles-ffi/include/bubbles_ffi.h).
If this document and the header disagree, the header wins and the disagreement is a bug.

## Two version numbers

| Number | Where | Meaning |
|---|---|---|
| Crate version | `BUBBLES_FFI_VERSION_STRING` in the header, `CARGO_PKG_VERSION` in Rust | Cargo semver of the `bubbles-ffi` package. Moves on every release. |
| ABI version | `BUBBLES_ABI_VERSION` in the header, `bubbles_abi_version()` at runtime | Integer, bumped **only** for a breaking change to this contract. Currently `1`. |

The two are independent. A new crate version with additive-only changes keeps the same ABI
version; the ABI version does not track crate patch or minor releases.

Check the ABI version at startup and refuse to run against a library you do not understand:

```c
if (bubbles_abi_version() != BUBBLES_ABI_VERSION) {
    fprintf(stderr, "bubbles ABI mismatch: header %d, library %u\n",
            BUBBLES_ABI_VERSION, bubbles_abi_version());
    return 1;
}
```

`bubbles_abi_version` is the one symbol guaranteed to exist, keep its signature, and be safe
to call before anything else, in every release that ships this ABI. It is the handshake.

## Stable: you may rely on these

Within a single ABI version, across any number of crate releases:

### Symbols and linkage

- Every public entry point is `extern "C"`, unmangled, and named with a `bubbles_` prefix.
- The 26 symbols declared in the header keep their names, parameter counts, parameter types,
  and return types.
- The calling convention is the platform C default (`cdecl` on x86, the standard AAPCS/SysV
  convention elsewhere). Declare it explicitly in managed hosts — `CallingConvention.Cdecl`
  for .NET.
- No symbol is removed and no signature is changed. A behavioural change that cannot be
  expressed additively gets a new symbol with a suffixed name instead.

### Integer widths and types

- Status codes are C `int`, which is 32-bit on every supported target, so `int32_t` in the
  header is accurate. Treat the return of every status-returning function as a signed 32-bit
  integer.
- Lengths, byte counts, and option indices are `size_t` — pointer-width. In .NET this is
  `nuint`, not `int`. Getting this wrong is the single most common P/Invoke bug against this
  library and it only shows up on 64-bit.
- `bubbles_abi_version` returns `uint32_t`.

### Status codes

| Macro | Value | Meaning |
|---|---|---|
| `BUBBLES_OK` | `0` | Success. Any out-parameter has been written. |
| `BUBBLES_DONE` | `1` | `bubbles_runner_next_event` only: dialogue finished, `*out_event_json` set to `NULL`. |
| `BUBBLES_ERR` | `-1` | Failure. Call `bubbles_last_error()`. Out-parameters are **not** written. |

These three numeric values are frozen. No function returns any other value. Compare against
the macros rather than testing `rc != 0`, because `BUBBLES_DONE` is a success.

### Saliency constants

`BUBBLES_SALIENCY_FIRST_AVAILABLE` = `0`, `BUBBLES_SALIENCY_BLRV` = `1`,
`BUBBLES_SALIENCY_RANDOM_AVAILABLE` = `2`. These values are frozen. Passing an unrecognised
value returns `BUBBLES_ERR` rather than falling back to a default, so a host built against a
newer header fails loudly against an older library instead of silently changing behaviour.

### Struct layout

`BubblesSourceFile` is `#[repr(C)]`. Field order, types, and the absence of padding
directives are stable:

```c
typedef struct BubblesSourceFile {
    const char *path_ptr;
    size_t      path_len;
    const char *text_ptr;
    size_t      text_len;
} BubblesSourceFile;
```

No field is reordered, removed, or retyped. No field is added — growing a `repr(C)` struct
breaks callers that allocate arrays of it, so a wider input struct would be a new type with
a new name.

### Callback signature

`BubblesHostFn` is stable in full:

```c
typedef int32_t (*BubblesHostFn)(void *userdata, const char *args_json_ptr,
                                 size_t args_json_len, char **out_result_json);
```

### Handles

`program` and `runner` are opaque `void *`. They are stable *as opaque tokens*: you may
store, copy, and pass them back. You may not dereference them, assume a size, or assume one
kind of handle is interchangeable with the other. The pointed-to Rust types are explicitly
not part of this ABI and change freely.

### Strings

All text crossing the boundary is UTF-8.

- **Inputs** are a pointer plus a **byte length**, never NUL-terminated by requirement. A
  trailing NUL is allowed but must not be counted in the length. Invalid UTF-8 returns
  `BUBBLES_ERR` rather than reading past the slice or producing replacement characters.
- **Outputs** are NUL-terminated so `Marshal.PtrToStringUTF8` and `strlen` work without a
  separate length.

### Event JSON schema

`bubbles_runner_next_event` returns one JSON **object** per event. The `kind` field is always
present and always a string. See the dedicated section below.

## Not stable: do not rely on these

Any of these may change in a patch release without an ABI version bump.

- **Error message text.** `bubbles_last_error()` returns a human-readable diagnostic for logs.
  Never parse it, match on it, or show it to players. Branch on the status code instead.
- **New symbols.** Additive only, but they do appear. Resolve symbols you need by name and
  tolerate the presence of ones you do not know.
- **New JSON fields.** Events, variable declarations, and introspection output gain fields.
  Your JSON parser must ignore unknown object keys.
- **New `kind` values.** `DialogueEvent` is `#[non_exhaustive]` in Rust. A future runtime
  variant this shim does not recognise serialises as `{"kind":"Unknown"}`. Treat both an
  unrecognised `kind` and `"Unknown"` as "ignore and keep pumping" — never as fatal.
- **New saliency constants.** Additive; existing values stay put.
- **Snapshot JSON internals.** The output of `bubbles_runner_snapshot_session_json` and
  `bubbles_runner_snapshot_storage_json` is an **opaque blob**. Store it, hand it back to the
  matching restore function, and do not inspect or synthesise it. The one guarantee is that a
  snapshot round-trips through the same crate version. Across crate versions, restore may
  return `BUBBLES_ERR`, so treat a save-file load failure as a recoverable condition in your
  game, not a crash.
- **Ordering of JSON object keys**, and whitespace within returned JSON.
- **Which thread-local slot holds the last error**, and whether a successful call clears it
  (see Errors below for what *is* guaranteed).
- **The Rust API of the `bubbles_ffi` crate** (the `lib` target). It exists so the integration
  tests and fuzz targets can call the entry points without loading a shared library. It is not
  a supported Rust interface; depend on `bubbles-dialogue` directly instead.

## Allocation and ownership

The rule in one line: **every pointer this library hands you is freed with
`bubbles_string_free`, and nothing else.** Not `free()`, not `Marshal.FreeHGlobal`, not
`delete`. The library may be built against a different allocator than your host, so mixing
them is undefined behaviour, not merely untidy.

| Pointer | Produced by | Owner | How to release |
|---|---|---|---|
| Event JSON (`out_event_json`) | `bubbles_runner_next_event` on `BUBBLES_OK` | caller | `bubbles_string_free` |
| Any `out_json` | `bubbles_program_*_json`, `bubbles_runner_variable_get_json`, `bubbles_runner_snapshot_*_json` | caller | `bubbles_string_free` |
| `bubbles_copy_utf8` result | you, explicitly | caller, until handed to the library | `bubbles_string_free` |
| Host-fn result (`*out_result_json`) | your callback, via `bubbles_copy_utf8` | **library** — it frees it | do **not** free it yourself |
| `bubbles_last_error()` result | library, thread-local | **library** | never free; copy before the next call |
| `program` handle | `bubbles_compile`, `bubbles_compile_files` | caller | `bubbles_program_free`, **or** transfer to `bubbles_runner_new*` |
| `runner` handle | `bubbles_runner_new`, `bubbles_runner_new_with_saliency` | caller | `bubbles_runner_free` |
| Your input buffers | you | you | yours; the library never retains them past the call |

Three consequences worth stating explicitly, because each is a real crash:

1. **`bubbles_runner_new` and `bubbles_runner_new_with_saliency` consume the program handle.**
   Precisely: the handle is consumed whenever both `program` and `out_runner` are non-NULL —
   on success *and* on a failure such as an invalid saliency kind. Since passing a NULL
   `out_runner` is a bug you should never ship, the practical rule is unconditional: after
   calling either constructor, the program handle is gone. Never `bubbles_program_free` it,
   never build a second runner from it, and null your copy at the call site.
2. **Input pointers are borrowed for the duration of the call only.** The library copies what
   it needs. In a garbage-collected host you must pin inputs across the call (`GCHandle` in
   .NET) but you may release them immediately after it returns.
3. **A host-function result must come from `bubbles_copy_utf8`**, because the library frees it
   with its own allocator. This holds on the error path too: when your callback returns
   `BUBBLES_ERR`, `*out_result_json` must still be a `bubbles_copy_utf8` string containing the
   error message. Leaving it `NULL` is itself reported as a function error.

`bubbles_string_free(NULL)` and `bubbles_program_free(NULL)` and `bubbles_runner_free(NULL)`
are all no-ops, so you do not need null guards in cleanup paths.

## Errors

On `BUBBLES_ERR`, `bubbles_last_error()` returns a NUL-terminated UTF-8 message, or `NULL` if
there is somehow none.

What is guaranteed:

- The pointer is valid until the **next** `bubbles_*` call **on the same thread**. Copy the
  string if you need to keep it — logging it immediately is fine, stashing the pointer is not.
- The error slot is **thread-local**. An error raised on one thread is not visible from
  another, and a call on thread A cannot invalidate a pointer thread B is holding.
- After `BUBBLES_ERR`, out-parameters are not written. Initialise them to `NULL` / `0` and they
  stay that way; do not read them on the error path.
- A call that fails **argument validation** leaves the handle usable. Invalid UTF-8, malformed
  JSON, an unknown node name, an out-of-range or unavailable option index: all reported, none
  fatal. Retry with corrected input on the same runner.
- `bubbles_runner_free` always succeeds and is always safe to call, including after an error.

What is not: the message text, whether a *successful* call leaves a stale message behind, and
whether a runner is still usable after `bubbles_runner_next_event` itself returns
`BUBBLES_ERR`. That failure means the script faulted mid-execution; stop pumping that dialogue
and free the runner rather than trying to resume it.

Only read `bubbles_last_error()` immediately after a call that returned `BUBBLES_ERR`.

## Threading

The surface is **not thread-safe**. Specifically:

- A single `runner` handle must only be touched from one thread at a time. There is no internal
  locking; concurrent calls on one handle are a data race.
- Independent handles on independent threads are fine — there is no shared mutable global
  state, and the error slot is per-thread.
- Do **not** call back into any `bubbles_*` function from inside a `BubblesHostFn` callback.
  The runner is mutably borrowed for the duration of the callback and re-entering it is
  undefined behaviour.

Unity, Godot, and Bevy hosts: drive dialogue from one thread (the main thread is fine) and
this constraint costs you nothing.

## Event JSON schema

Every event is a JSON object with a string `kind`. Unknown keys may be added at any time;
ignore the ones you do not use. `null` is used for absent optional values.

```json
{ "kind": "NodeStarted", "node": "Intro" }
```

```json
{
  "kind": "Line",
  "speaker": "Aria",
  "text": "Evening, friend.",
  "line_id": "aria_greet_01",
  "tags": ["cheerful"],
  "line_mode": "normal",
  "spans": [{ "name": "wave", "start": 0, "length": 7, "properties": [] }]
}
```

```json
{
  "kind": "Options",
  "options": [
    {
      "text": "Hello.",
      "available": true,
      "line_id": null,
      "tags": [],
      "group": null,
      "spans": []
    }
  ]
}
```

```json
{ "kind": "Command", "name": "play_music", "args": ["tavern_theme"], "tags": [] }
```

```json
{ "kind": "NodeComplete", "node": "Intro" }
```

```json
{ "kind": "DialogueComplete" }
```

```json
{ "kind": "Unknown" }
```

Field types:

| Field | Type | Notes |
|---|---|---|
| `kind` | string | Always present. |
| `node` | string | `NodeStarted` / `NodeComplete`. The node title. |
| `speaker` | string or `null` | `null` for narration and unattributed lines. |
| `text` | string | Markup tags stripped, `{expr}` fragments already evaluated. |
| `line_id` | string or `null` | From `#line:<id>`, without the `line:` prefix. Key voice-over on this. |
| `tags` | array of string | Trailing `#tag` metadata, `#` stripped. Mode and `line:` tags remain here. |
| `line_mode` | string | `"normal"`, `"narration"`, or `"debug"`. Lowercase. New values may be added. |
| `spans` | array of object | Markup spans over `text`, in source order. Empty when there is no markup. |
| `options` | array of object | `Options` only. Index into this array is what `bubbles_runner_select_option` takes. |
| `available` | bool | An option whose guard evaluated false. Show it greyed out or filter it; selecting it returns `BUBBLES_ERR`. |
| `group` | string or `null` | From `#group:<name>`, for radio-button style UI constraints. |
| `name`, `args` | string, array of string | `Command` only. |

A markup span is `{"name": string, "start": number, "length": number, "properties": array}`.
`start` and `length` are **byte** offsets into `text`, not character or UTF-16 code-unit
offsets — a .NET host indexing a `string` with them will be wrong for any non-ASCII line.
Convert via the UTF-8 bytes. `length` is `0` for self-closing tags like `[pause /]`.
`properties` is an array of two-element `[key, value]` string arrays, *not* an object, because
a markup tag may repeat a key:

```json
{ "name": "color", "start": 6, "length": 4, "properties": [["value", "red"]] }
```

### Dialogue `Value` JSON

`bubbles_runner_variable_get_json` and the host-function argument array use the dialogue
`Value` encoding, which is externally tagged:

```json
{"Number": 42.0}
{"Text": "sword"}
{"Bool": true}
```

Reading an unset variable yields the bare literal `null`.

For **writing** — `bubbles_runner_variable_set_json` and host-function return values — both the
tagged form above and plain JSON scalars (`42`, `"sword"`, `true`) are accepted. Plain `null`,
arrays, and objects with an unrecognised key are rejected with `BUBBLES_ERR`. Numbers are IEEE
754 doubles; integers beyond 2^53 lose precision.

The asymmetry is deliberate and stable: accept the convenient form, emit the explicit one.
Write a parser that handles the tagged form and you are future-proof.

## Supported targets

Prebuilt binaries are produced by the `ffi-prebuilt` workflow for:

| Target | Artifact |
|---|---|
| `x86_64-pc-windows-msvc` | `bubbles_ffi.dll` (+ `bubbles_ffi.dll.lib` import library) |
| `x86_64-apple-darwin` | `libbubbles_ffi.dylib` |
| `aarch64-apple-darwin` | `libbubbles_ffi.dylib` |
| `x86_64-unknown-linux-gnu` | `libbubbles_ffi.so` |

Other targets are expected to work — there is no platform-specific code in this crate — but
are not built or tested by CI. See
[the native hosts chapter](https://viezevingertjes.github.io/bubbles/integration/native-hosts.html)
for notes on iOS, Android, and WebAssembly.

Linux binaries are built on the oldest GitHub-hosted Ubuntu runner available and link glibc
dynamically; the glibc version floor is whatever that runner ships. If you need an older
floor or a musl build, build from source.

## Breaking the ABI

If a change cannot be made additively:

1. `bubbles_abi_version()` is bumped, and the header's `BUBBLES_ABI_VERSION` with it.
2. The crate gets a major version bump.
3. The change is listed below with a migration note.

Hosts that follow the handshake at the top of this document get a clear startup failure rather
than memory corruption.

## ABI history

### Version 1 — `bubbles-ffi` 2.0.0

Initial documented ABI. 26 symbols, three status codes, three saliency strategies, one
`repr(C)` input struct, one callback type, and the event JSON schema above.
