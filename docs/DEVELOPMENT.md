# Development

## Prerequisites

| Component | Version | Needed for |
| --- | --- | --- |
| Rust | stable, per `rust-toolchain.toml` | everything |
| rustfmt, clippy | matching the toolchain | formatting and lint |
| CMake | ≥ 3.24 | Windows application (Phase 6) |
| Qt | 6.5 LTS or newer (Core, Gui, Widgets/Quick) | Windows application |
| MSVC | Visual Studio 2022 toolchain | Windows application |
| JDK | 17 | Android application (Phase 7) |
| Android SDK | API 34+, platform tools | Android application |
| Android NDK | r26+ | building the Rust core for Android |
| `cargo-ndk` | latest | cross-compiling the core for Android ABIs |

Only the first three are required for the current phase.

## First run

```bash
git clone https://github.com/merajXD/XoPo_Vault.git
cd XoPo_Vault
cargo fmt --all --check
cargo test --workspace
```

The core has no external services and no database, so tests run offline after the first dependency fetch.

## Everyday commands

```bash
cargo test --workspace                       # all unit and integration tests
cargo test -p xopo-domain                    # one crate
cargo test -p xopo-domain address            # tests whose name matches
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo doc --workspace --no-deps --open       # API documentation
cargo build -p xopo-cli                      # the `xopo` developer CLI
```

### Trying the core from the command line

The `xopo` binary is a development tool, not the product interface. It runs the same engine calls the
future interfaces will use, against a real encrypted vault:

```bash
./target/debug/xopo init /tmp/demo.xopo --name "Demo Vault"
./target/debug/xopo wallet add /tmp/demo.xopo --label Demo --network bitcoin,ethereum --phrase-file phrase.txt
./target/debug/xopo unlock /tmp/demo.xopo
```

Use only public test vectors as the phrase. `--help` lists every command; `README.md` shows a full
session with its output.

CI runs the same three checks (`fmt --check`, `clippy -D warnings`, `test`) on every push.

## Workflow

1. Read the current code and `docs/ROADMAP.md` before starting; do not assume a file exists.
2. Keep changes inside one phase and one concern.
3. Add or update tests for every rule you introduce.
4. Run `cargo fmt`, `cargo test`, `cargo clippy`.
5. Update the affected document: architecture, schema, protocol, design system or roadmap.
6. Commit one meaningful milestone, not a checkpoint of unfinished code.

## Commit messages

Short imperative subject, then a body explaining *why*:

```text
Add network-scoped address validation

Reject an address on the wrong network at construction time so an
Ethereum address can never be stored in a Bitcoin account. Checksums
remain the adapter's job.
```

## Conventions

### Rust

* Errors: one explicit `thiserror` enum per crate. No `anyhow` in library crates, no `unwrap`/`expect` outside tests, no `panic!` on user input.
* Public items are documented; `# Errors` sections list every error a fallible function can return.
* Validated types are constructed through fallible constructors, and `Deserialize` reuses them.
* No `unsafe` in the Rust core. The FFI crate will encapsulate the only permitted `unsafe` blocks, each with a safety comment.
* No floating point for money, ever.
* No `println!`/`eprintln!` in libraries. Diagnostics go through a logging facade added with the security phase, which masks sensitive fields.

### Data

* Stored and synchronized values are canonical: `m/84'/0'/0'/0/0`, `"bitcoin"`, `"0.00000001"`, atomic units as integers.
* Never store display text where a stable identifier exists.
* Timestamps are UTC milliseconds since the Unix epoch; formatting happens in the UI.

### Names

* Network identifiers are kebab-case and stable across storage, wire format and UI (`bitcoin`, `bnb-chain`).
* Tickers are uppercase ASCII (`USDT`); names are user-facing text.

## Adding a network

1. Add the variant to `NetworkId` and its `info()` row (display name, native symbol, decimals, SLIP-0044 coin type, family, tier).
2. Extend `matches_network` in `address.rs` with structural rules, plus valid/invalid tests.
3. Add the adapter under `crates/xopo-chain-adapters` implementing the shared trait.
4. Add derivation tests using the network's published test vectors before enabling real balances.
5. Update `docs/ARCHITECTURE.md` and `docs/ROADMAP.md`.

## Adding a crate

Add it to `[workspace] members`, reuse `[workspace.package]` fields and `[lints] workspace = true`, and document its responsibility in `docs/ARCHITECTURE.md`. Do not create a crate that only re-exports another one.

## Test data

* Test fixtures are labelled as fixtures in the code that defines them.
* No fixture is ever presented by the application as a real balance, price or transaction.
* Published test vectors (BIP-32/39/44/84, chain address checksums) will be checked in with their source noted; derived values are not invented.

## Platform builds (later phases)

Windows:

```bash
cmake -S platforms/windows -B platforms/windows/build -DCMAKE_BUILD_TYPE=Release
cmake --build platforms/windows/build --config Release
```

Android:

```bash
cd platforms/android && ./gradlew :app:assembleDebug
```

These commands are documented ahead of implementation; they will be verified when the platform phases land. The Rust core must be built and tested on the host first, in both cases.

## Environment limitations

* The Windows application cannot be compiled on Linux without the Qt/MSVC toolchain, and the Android application requires the Android SDK/NDK. A phase is only reported as built when its build actually ran in the right environment.
* No step in this project may require network access at runtime except optional public market data and optional balance queries, both of which must degrade to an explicit "unavailable" state.
