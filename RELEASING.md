# Releasing

1. Bump **`version`** in **`Cargo.toml`** under **`[workspace.package]`** (workspace members inherit it).
2. Set the first non-blank line of **`VERSION`** to the same value (used by mdBook `{{#include}}` in the guide).
3. Update **`README.md`** quick start so the line `bubbles-dialogue = "<version>"` matches (GitHub does not process the book includes).
4. Update **`crates/bubbles-ffi/include/bubbles_ffi.h`**: `BUBBLES_FFI_VERSION_MAJOR` / `MINOR` / `PATCH`, `BUBBLES_FFI_VERSION_STRING`, and the `Library version:` line in the top comment block.
5. Update the `bubbles-dialogue` dependency version in **`crates/bubbles-ffi/Cargo.toml`** to match, so the crate stays publishable.
6. Run **`bash scripts/check-version-sync.sh`** (runs in **pre-commit** and in **CI**; it checks every item above).
7. Tag the release (see **Publish** workflow: the tag must match the crate version).
8. **`cargo publish -p bubbles-dialogue`**

Patch releases (`0.6.0` → `0.6.1`) follow the same steps; the book and README stay on the exact semver shown above.

## The C ABI

`BUBBLES_ABI_VERSION` is **not** the crate version and does not move on an ordinary release.
Bump it only when breaking the C contract, and then follow
[`crates/bubbles-ffi/ABI.md`](crates/bubbles-ffi/ABI.md): bump the macro in the header and
the constant in `src/lib.rs`, bump the crate major version, and record the change under
"ABI history".

Two guards keep this honest and both run in CI:

- `crates/bubbles-ffi/tests/header.rs` — the header's macros match the Rust constants and
  the struct and callback layouts have not drifted.
- `scripts/check-ffi-abi.sh` — the built `cdylib` exports exactly the symbols the header
  declares.

## Prebuilt FFI binaries

Pushing a `v*` tag runs the **FFI prebuilt binaries** workflow, which builds the shared
library for Windows x64, macOS x64, macOS arm64, a macOS universal2 slice, and Linux x64,
then attaches them to a **draft** GitHub release with a `SHA256SUMS.txt`.

The draft is never published automatically — review the artifacts and publish it by hand.
The workflow refuses to attach assets to a release that is already published.
