# Changelog

All notable changes to this project will be documented in this file.

This changelog starts from the current `0.1.0` repository state. Earlier incremental changes were
not backfilled from commit history.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.1.0]

### Added

- Blocking C bindings for the Rust `guerrillamail-client` crate.
- A builder-based client configuration API for proxy settings, TLS certificate handling, user
  agent overrides, and request timeouts.
- Client operations for creating disposable email addresses, listing messages, fetching message
  details, and deleting email addresses.
- FFI-safe result types for strings, message lists, and email details, plus explicit free
  functions for owned return values.
- Thread-local last-error helpers for retrieving and clearing human-readable error messages.
- A checked-in public header at `include/guerrillamail_client.h` and a C usage example at
  `examples/demo.c`.
