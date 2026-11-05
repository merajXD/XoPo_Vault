# Scripts

Developer and release tooling. Only scripts that are actually used belong here.

| Script | Purpose | Status |
| --- | --- | --- |
| `check.sh` | Run format check, clippy and tests exactly as CI does | Available |

Planned, to be added with their phases:

* `build-android-core.sh` — cross-compile `xopo-core` per ABI with `cargo-ndk` (Phase 7).
* `build-windows-core.ps1` — build the C ABI DLL with the MSVC target (Phase 6).
* `package.sh` — assemble release artifacts (Phase 9).
