# bubbles-ffi

C ABI shared library for driving `.bub` dialogue from Unity, C#, C++, Godot .NET, or any
host that can load a native plugin and call a C function. No Rust required.

- **ABI version:** 1 (`bubbles_abi_version()`)
- **Header:** [`include/bubbles_ffi.h`](include/bubbles_ffi.h)
- **Stability policy:** [`ABI.md`](ABI.md) — read this before you ship

## Get it

Download the archive for your platform from a
[release](https://github.com/ViezeVingertjes/bubbles/releases) — Windows x64, macOS
universal, macOS x64, macOS arm64, and Linux x64 are prebuilt. Each archive contains the
library, `bubbles_ffi.h`, `ABI.md`, and both licences, so one download is everything you
need to compile against.

Or build it yourself:

```sh
cargo build -p bubbles-ffi --release
```

| Platform | File |
|----------|------|
| Linux    | `target/release/libbubbles_ffi.so` |
| macOS    | `target/release/libbubbles_ffi.dylib` |
| Windows  | `target/release/bubbles_ffi.dll` (+ `bubbles_ffi.dll.lib` import library) |

## Try it

A complete host in one dependency-free C file:

```sh
cd examples/c-host && make run
```

## The rules in brief

- All text is **UTF-8**. Inputs are a pointer plus a **byte length**; returned strings are
  NUL-terminated.
- Free every string the library returns with `bubbles_string_free`, never `free()`.
- Status is `BUBBLES_OK` / `BUBBLES_DONE` / `BUBBLES_ERR`. `BUBBLES_DONE` is a success, so
  `rc != BUBBLES_OK` is not an error test.
- `bubbles_runner_new` **consumes** the program handle.
- Not thread-safe — drive one runner from one thread.
- Check `bubbles_abi_version()` against `BUBBLES_ABI_VERSION` at startup.

[`ABI.md`](ABI.md) has the whole contract: the event JSON schema, an ownership table, error
semantics, threading, and what may change between releases.

## Rust callers

Depend on [`bubbles-dialogue`](https://crates.io/crates/bubbles-dialogue) instead. The Rust
API this crate exposes exists so its own tests and fuzz targets can reach the entry points
without loading a shared library; it is not a supported Rust interface.

## More

P/Invoke declarations, host functions, localisation, variables, save/load, the .NET smoke
test, and notes on iOS, Android, and WebAssembly are in the
[Native hosts (C ABI)](https://viezevingertjes.github.io/bubbles/integration/native-hosts.html)
chapter of the guide.
