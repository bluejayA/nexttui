# Build Instructions — BL-P2-074

## Prerequisites
- Rust (edition 2024, toolchain per `rust-toolchain.toml`)
- Cargo (bundled with Rust)
- Network access for first-time dependency fetch

## Steps

1. Format check:
   ```
   cargo fmt --all --check
   ```
2. Lib/tests build (primary):
   ```
   cargo build --lib --tests
   ```
3. Binary build:
   ```
   cargo build --bin nexttui
   ```

## Expected Output
- `cargo fmt --all --check` → 0 diff, exit 0.
- `cargo build --lib --tests` → `Finished 'dev' profile` with 0 errors/warnings.
- `cargo build --bin nexttui` → `target/debug/nexttui` executable produced.

## Last Verified
- **Commit base**: 551265b (main)
- **Branch**: feat/bl-p2-074-switch-cloud-wire
- **Timestamp**: 2026-04-20T10:15+09:00
- **Status**: ✅ 전부 통과
