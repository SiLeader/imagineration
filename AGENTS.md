# Repository Guidelines

## Project Structure & Module Organization

This is a Rust 2024 workspace. The root package, `imagineration`, is an Axum HTTP server with its entry point in
`src/main.rs`, configuration loading in `src/config.rs`, and HTTP routes under `src/routes/`. `src/routes/mod.rs`
defines shared route state, types, and route registration; individual endpoint handlers live in files such as
`list_models.rs`, `post_generate.rs`, `list_images.rs`, `get_image.rs`, and `get_image_metadata.rs`. The
`imagineration-generator/` crate contains image generation logic and generator-specific modules in
`imagineration-generator/src/` (`config.rs`, `fields.rs`, `models.rs`, `output.rs`, `presets.rs`, and related support
modules). API documentation lives in `docs/apis/`, with broader directory and runtime layout notes in
`docs/directory-structure.md`.

Runtime paths are configured in `imagineration.toml`. By default, model files are read from `models/` and generated
PNG/JSON pairs are written under `data/images/`.

## Build, Test, and Development Commands

- `cargo check --workspace --all-targets`: fast compile check for all crates and targets.
- `cargo test --workspace --all-targets`: run all unit tests in the server and generator crate.
- `cargo fmt --all`: format Rust code using `rustfmt`.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: run lints and fail on warnings.
- `cargo run -- --config imagineration.toml`: start the local server using the default config (`127.0.0.1:3000`).

Use `RUSTC_WRAPPER=` before Cargo commands if a local wrapper causes inconsistent builds.
Enable hardware backends with Cargo features when needed: `vulkan`, `cuda`, `metal`, or `hip`.

## Coding Style & Naming Conventions

Follow standard Rust formatting: four-space indentation, `snake_case` for functions/modules, `PascalCase` for types, and
`SCREAMING_SNAKE_CASE` for constants. Prefer workspace dependencies in the root `Cargo.toml` when a crate is shared by
multiple packages. Keep HTTP route types and request/response serialization close to the relevant files in
`src/routes/`; keep generator-specific config parsing, model handling, field mapping, and output handling in
`imagineration-generator/src/`.

## Testing Guidelines

Tests currently live inline in `mod tests` blocks, including `src/routes/list_models.rs`,
`src/routes/input_assets.rs`, and `imagineration-generator/src/lib.rs`. Name tests after the behavior being verified,
for example `detects_model_kind_from_extension`. Add focused unit tests for parsing, path resolution, metadata, request
validation, and input asset handling. Run `cargo test --workspace --all-targets` before opening a PR.
Keep each source file to approximately 800 lines or fewer, and each function to 50 lines or fewer (preferably 20
lines or fewer).
Use the `mod` keyword to properly separate modules, and ensure that unnecessary identifiers do not leak outside the
module.

## Commit & Pull Request Guidelines

Recent history uses short imperative summaries such as `Add Content-Disposition header to get_image response` and
`Refactor application structure to modularize routes and enhance maintainability`. Keep commits scoped and explain
user-visible behavior or refactors clearly. PRs should include a concise description, testing performed, related issue
links when applicable, and API documentation updates when endpoints or payloads change. Include screenshots or sample
responses only when behavior is visual or HTTP-facing.

## Security & Configuration Tips

Do not commit model weights, generated images, secrets, or machine-specific paths. Keep local overrides in config files
outside version control when needed, and document any new required config keys in `imagineration.toml` and `docs/`.
