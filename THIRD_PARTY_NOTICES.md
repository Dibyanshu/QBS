# Third-party notices

QBS uses open-source Rust crates listed in `Cargo.toml` and locked in
`Cargo.lock`. Their licenses remain with their respective authors. The complete
package-by-package license inventory is shipped as `THIRD_PARTY_LICENSES.md`.

The experimental legacy Windows `UserChoice` hash backend is an independent
Rust port based on the algorithm published by DanysysTeam in PS-SFTA:

- Project: https://github.com/DanysysTeam/PS-SFTA
- Copyright: 2022 Danysys
- License: MIT

The backend is isolated in `src/switching/experimental.rs`. Windows does not
document or support this mechanism and can invalidate it at any time.

