# C host example

A complete dialogue host in one C file. It loads a `.bub` file from disk, compiles it,
pumps the event loop, prints lines, and selects an option — using nothing but the C ABI in
[`bubbles_ffi.h`](../../crates/bubbles-ffi/include/bubbles_ffi.h).

No Rust appears in `main.c`, and once you have the shared library, no Rust toolchain is
needed to build or run it. If you are evaluating bubbles from a non-Rust engine, this is
the shortest path to seeing it work.

## Run it

With a Rust toolchain (builds the library for you):

```sh
cd examples/c-host
make run
```

Expected output:

```text
Innkeeper: Evening. You look like you walked a long road.
  [command: play_music]
  1) A room for the night, please.
  2) Just information.
  3) Nothing, sorry.  (unavailable)
> 1
Innkeeper: Six gold, and the stew is free.
Innkeeper: Sleep well. You have 6 gold left.
Innkeeper: Mind the step on your way out.
```

Run it from a terminal and it prompts you to choose; run it with redirected input, as CI
does, and it picks the first available option so the same binary works unattended.

Option 3 shows as `(unavailable)` because its guard — `<<if $gold > 100>>` — is false. The
runtime reports unavailable options rather than hiding them, so your UI decides whether to
grey them out or filter them. Selecting one returns `BUBBLES_ERR`.

### Without a Rust toolchain

Download the archive for your platform from a
[release](https://github.com/ViezeVingertjes/bubbles/releases), extract it, and point the
Makefile at it:

```sh
make LIBDIR=/path/to/bubbles-ffi-x86_64-unknown-linux-gnu NO_CARGO=1 run
```

The archive contains the shared library and the matching `bubbles_ffi.h`, so
`INCDIR=$LIBDIR` also works if you would rather not reference this repository at all.

### Windows (MSVC)

```bat
cl /std:c11 /W4 /I path\to\include main.c /link /LIBPATH:path\to\lib bubbles_ffi.dll.lib
copy path\to\bubbles_ffi.dll .
c-host.exe tavern.bub Tavern
```

The `.dll.lib` import library is in the Windows release archive. The DLL must sit next to
the executable or on `PATH`.

## Arguments

```sh
./c-host [path-to-bub-file] [start-node]
```

Both are optional and default to `tavern.bub` and `Tavern`.

## What the code demonstrates

Read `main.c` top to bottom; the six numbered steps in `main` are the whole lifecycle.

| Step | Shows |
|---|---|
| 1 | The ABI version handshake: `bubbles_abi_version()` against `BUBBLES_ABI_VERSION`, refusing to run on a mismatch |
| 2 | Loading UTF-8 source and passing it as a pointer plus **byte length** |
| 3 | `bubbles_compile`, and `bubbles_program_node_exists` to turn a typo into a clear message |
| 4 | `bubbles_runner_new` **consuming** the program handle — the pointer is nulled at the call site so cleanup cannot double-free it |
| 5 | `bubbles_runner_start` |
| 6 | Pumping until `BUBBLES_DONE`, dispatching on `kind`, and ignoring unrecognised kinds instead of failing |

It also shows the two rules that cause the most grief in a first integration:

- **Every string the library returns is freed with `bubbles_string_free`**, never `free()`.
  The library may use a different allocator than your program.
- **Option indices are `size_t`**, not `int`. In a managed host this is `nuint`; getting it
  wrong only breaks on 64-bit.

The error path is worth a look too: `fail()` reads `bubbles_last_error()` immediately,
because that pointer is invalidated by the next `bubbles_*` call on the thread.

`main.c` contains a small JSON reader so the example has no dependencies. **Do not copy it
into a game** — use your engine's JSON parser. It is deliberately minimal and only
understands the fields this example reads.

## Next steps

- [C ABI stability policy](../../crates/bubbles-ffi/ABI.md) — the full event JSON schema,
  ownership rules, threading constraints, and what is allowed to change between releases.
- [Native hosts (C ABI)](https://viezevingertjes.github.io/bubbles/integration/native-hosts.html)
  — the same ground in C#, plus host functions, localisation, variables, and save/load.
- `crates/bubbles-ffi/tests/dotnet_smoke/` — a .NET 10 P/Invoke host that exercises the
  features this example leaves out.
