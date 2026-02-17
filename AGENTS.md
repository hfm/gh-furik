# Repository Guidelines

## Purpose and Scope
`gh-furik` is a Rust implementation of a GitHub CLI extension that summarizes GitHub activity for a date range. Keep changes focused on CLI behavior, GitHub API integration, output formatting, and release packaging.

## Project Structure & Module Organization
- `src/main.rs`: CLI entry point and argument handling.
- `src/github/`: GitHub auth, client setup, GraphQL fetch/query modules.
- `src/github/graphql/queries/*.graphql`: GraphQL query definitions (formatted with dprint).
- `src/formatter.rs`: text rendering and output formatting.
- `.github/workflows/`: CI (`ci.yml`) and release automation (`release.yml`).
- `gh-furik`: extension launcher script that prefers release binary, then local build.

## Build, Test, and Development Commands
- `cargo build --release --locked`: build production binary.
- `cargo test --all-features`: run unit tests.
- `cargo fmt -- --check`: verify Rust formatting.
- `cargo clippy --all-targets --all-features -- -D warnings`: strict linting.
- `dprint fmt`: format GraphQL files under `src/github/graphql/queries/`.
- `gh furik --from 2025-01-01 --to 2025-01-31`: run the extension locally (after install/build).

## Coding Style & Naming Conventions
- Follow standard Rust style (`rustfmt`) with 4-space indentation.
- Treat clippy warnings as errors in CI; keep code warning-free.
- Use `snake_case` for functions/modules/files, `PascalCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Prefer small, composable modules in `src/github/graphql/` over large multi-purpose files.

## Testing Guidelines
- Place unit tests near implementation in `mod tests` blocks.
- Use descriptive test names that reflect behavior (for example, `resolves_token_from_env`).
- Cover auth fallback behavior, GraphQL parsing, and formatter output.
- Run `cargo test --all-features` before opening a PR.

## Commit & Pull Request Guidelines
- Use imperative, concise commit subjects (for example, `Handle empty enterprise token values`).
- Keep commits focused; avoid mixing refactors with behavior changes.
- PRs should include: purpose, user-visible impact, test/lint results, and linked issues.
- Include sample CLI output when changing formatter or command UX.

## Agent-Focused Context Hygiene
- Keep this file concise and broadly applicable.
- Put task-specific details in focused docs (for example, `docs/testing.md`, `docs/release.md`) and reference them from PRs.
- Prefer pointing to source files over duplicating long instructions.
