# guerrillamail-client-c

C bindings for the Rust [`guerrillamail-client`](../guerrillamail-client-rs) crate.

This repo builds a native library with a blocking C ABI. Internally it owns a Tokio runtime and
uses the async Rust client underneath, so C and C++ callers can use a simple request/response API.

## Build

```bash
cargo build
```

Artifacts are written under `target/debug/`:

- `libguerrillamail_client_c.a`
- `libguerrillamail_client_c.dylib` on macOS
- `libguerrillamail_client_c.so` on Linux

The public header is [`include/guerrillamail_client.h`](include/guerrillamail_client.h).

## Regenerate the Header

The checked-in header matches the current ABI. If `cbindgen` is installed locally, regenerate it
with:

```bash
cbindgen --config cbindgen.toml --crate guerrillamail-client-c --output include/guerrillamail_client.h
```

## Usage from C or C++

See [`examples/demo.c`](examples/demo.c) for a minimal consumer. The expected flow is:

1. Create a builder or default client.
2. Call blocking client functions.
3. Free returned strings/lists/details explicitly.
4. On failure, inspect `gm_last_error_message()`.

