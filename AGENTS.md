# Repository Guidelines

## Project Structure & Module Organization

This is a Rust 2024-edition Bevy chess application. `src/main.rs` contains the Bevy app, UI, input, rendering, and asynchronous engine integration; `src/engine.rs` contains chess rules and engine logic. `assets/models/` holds the board model, `assets/fonts/` contains fonts, and `assets/` also contains image assets. `Cargo.toml` defines dependencies, features, and build profiles. There is currently no separate tests directory.

## Build, Test, and Development Commands

Run commands from the repository root so Bevy can locate assets.

- `RUST_LOG=off cargo run` builds and launches the game with reduced logging.
- `cargo build --release` produces an optimized executable in `target/release/`.
- `cargo test` builds and runs Rust tests.
- `cargo fmt --check` checks formatting; `cargo fmt` formats the code.
- `cargo clippy` checks for common Rust issues.

Use `cargo run --features=default_font` for the alternate font configuration or `--features=salewskiChessDebug` for engine debugging.

## Coding Style & Naming Conventions

Use standard Rust formatting and four-space indentation. Name functions, variables, and modules in `snake_case`, types in `UpperCamelCase`, and constants in `SCREAMING_SNAKE_CASE`. Keep Bevy systems and components close to related frontend code, and keep chess rules in the engine module. Format touched code with `cargo fmt` where practical.

## Testing Guidelines

Use Rust's built-in test framework for new automated checks. Place unit tests in a `#[cfg(test)]` module near the relevant code, or integration tests in `tests/`, and give tests descriptive `snake_case` names. No coverage threshold is specified. For gameplay or visual changes, also run the application and manually check affected interactions, such as piece selection, legal moves, camera controls, and starting a new game.

## Commit & Pull Request Guidelines

Recent commits use short, plain-language subjects such as `update for Bevy 0.18` and `fixed picture path`; describe the change directly and keep each commit focused. Pull requests should explain the behavior changed and how it was tested, link relevant issues when available, and include screenshots or recordings for visible UI changes. Preserve model attribution and license information when changing assets.
