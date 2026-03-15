# Gungraun Agent Guidelines

This document provides instructions for AI agents working on the Gungraun repository.

## 1. Build, Lint, and Test Commands

Gungraun uses `just` as a task runner. Always prefer `just` commands over direct `cargo` invocations when available to ensure consistency with CI/CD.

### formatting & Linting

- **Format Code (Rust):** `just fmt` (Requires nightly toolchain)
- **Format TOML:** `just fmt-toml`
- **Format Prettier (JSON/YAML/MD):** `just fmt-prettier`
    - **Important:** Always run `just fmt-prettier` after making changes to `AGENTS.md`
- **Lint (Clippy):** `just lint` (Uses stable toolchain)
- **Check All Formatting:** `just check-fmt-all`

### Testing

- **Run All Tests:** `just test-all` (Excludes client-request-tests and benchmarks)
- **Run Package Tests:** `just test package package=<package_name>`
    - Example: `just test package package=gungraun`
- **Run UI Tests:** `just test-ui` (Fixed to MSRV compiler)
    - **Overwrite UI Test Fixtures:** `just test-ui-overwrite`
- **Run Doc Tests:** `just test-doc`

### Benchmarks (System Tests)

- **Run Single Benchmark System Test:** `just full-bench-test <bench_name>`
    - Example: `just full-bench-test test_lib_bench_tools`
- **Run All Benchmark System Tests:** `just full-bench-test-all`

## 2. Code Style & Conventions

### General

- **Rust Edition:** 2021
- **Line Length:** 100 characters for comments (enforced by rustfmt).
- **Newlines:** Unix style (`\n`).

### Formatting & Imports

- **Rustfmt:** Strictly adhere to `rustfmt.toml`.
    - `imports_granularity = "Module"`
    - `group_imports = "StdExternalCrate"`
- **Import Order:** std -> external crates -> crate modules.
- **Sorting:** Imports and modules should be sorted alphabetically.

### Naming Conventions

- **Types/Traits:** `UpperCamelCase`
- **Functions/Methods/Modules/Variables:** `snake_case`
- **Constants/Statics:** `SCREAMING_SNAKE_CASE`
- **Files:** `snake_case.rs`

### Error Handling

- **Library (`gungraun`):** Use specific, typed errors where possible.
- **Runner (`gungraun-runner`):**
    - Uses a central `Error` enum in `src/error.rs`.
    - Variants include `BenchmarkError`, `ConfigurationError`, `JobError` (wrapping `anyhow`), etc.
    - `JobError` wraps `anyhow::Error` for internal task failures.
    - Implement `Display` for user-facing error messages.
    - Use `thiserror` (if available) or manual `std::error::Error` implementation.

### Code Structure

- **Workspace:** Multi-crate workspace.
    - `gungraun`: Main library crate.
    - `gungraun-runner`: Binary runner.
    - `gungraun-macros`: Proc-macros.
    - `benchmark-tests`: System tests.
- **Documentation:**
    - Extensive doc comments (`///`) on public items.
    - Top-level crate documentation in `lib.rs`.
    - Examples in doc comments are encouraged.
    - When documenting functions or methods, describe parameters in fluent text rather than using structured `# Arguments` sections with parameter lists. Parameters should be naturally integrated into the documentation prose.
    - **Rustdoc Links:** Use reference-style links for long paths with multiple `::` accessors.
        - Use short form inline: `[`ToolOutputPath`]`
        - Define full path at bottom: `[`ToolOutputPath`]: crate::runner::tool::path::ToolOutputPath`
        - Example:
            ```rust
            /// Generates flamegraph summaries using [`Config`] and [`ToolOutputPath`].
            ///
            /// [`Config`]: crate::runner::common::Config
            /// [`ToolOutputPath`]: crate::runner::tool::path::ToolOutputPath
            ```
        - Short, simple types like `Config`, `Header`, `Streams` can use short form inline.
        - Long paths like `crate::runner::tool::path::ToolOutputPath` should use reference-style.
    - **Type References in Documentation:** Use rustdoc links for types, not parameter names.
        - Use `[`Type`]` for types/structs/enums/variants (e.g., `[`Command`]`, `[`Child`]`, `[`Stdin::Setup`]`)
        - Use backticks `` `parameter_name` `` only when referring to the parameter itself, not its type
        - Example:
            ```rust
            /// Applies this [`Stdin`] configuration to a [`Command`] for the selected [`Stream`].
            ///
            /// This method configures the given [`Command`] according to this [`Stdin`], using the
            /// [`Stream`] to select which process stream is being configured. When this is
            /// [`Stdin::Setup`], it optionally pipes data from the provided [`Child`].
            ///
            /// [`Command`]: std::process::Command
            /// [`Child`]: std::process::Child
            /// [`Stdin::Setup`]: crate::api::Stdin::Setup
            ```

### Testing Guidelines

- **Unit Tests:** Co-located in the same file or `mod tests` within the file.
- **Integration Tests:** Located in `tests/` directory of the package.
- **Benchmarks:** defined using `#[library_benchmark]` and `#[binary_benchmark]` attributes.

## 3. Workflow specific

- **Dependencies:** Check `Cargo.toml` before adding new dependencies. Use `cargo add` only if necessary and approved.
- **Lockfile:** Do not manually edit `Cargo.lock`.
- **Pre-commit:** Ensure `just fmt` and `just lint` pass before committing.
