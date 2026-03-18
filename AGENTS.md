# Gungraun Agent Guidelines

This document provides instructions for AI agents working on the Gungraun
repository.

## 1. Build, Lint, and Test Commands

Gungraun uses `just` as a task runner. Always prefer `just` commands over direct
`cargo` invocations when available to ensure consistency with CI/CD.

### formatting & Linting

- **Format Code (Rust):** `just fmt` (Requires nightly toolchain)
- **Format TOML:** `just fmt-toml`
- **Format Prettier (JSON/YAML/MD):** `just fmt-prettier`
    - **Important:** Always run `just fmt-prettier` after making changes to
      `AGENTS.md`
- **Lint (Clippy):** `just lint` (Uses stable toolchain)
- **Check All Formatting:** `just check-fmt-all`

### Testing

- **Run All Tests:** `just test-all` (Excludes client-request-tests and
  benchmarks)
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

### Markdown

- **Reference-Style Links:** Always use reference-style links in markdown files.
    - Use short, descriptive link references inline.
    - Define all references at the bottom of the file.
    - Sort reference definitions alphabetically by reference name.
    - **Short Form:** When the link text and reference name are identical, use
      the implicit reference syntax `[Text]` instead of `[Text][Text]`.
        - Prefer: `[Guide]` → Not: `[Guide][Guide]`
    - Example:

        ```markdown
        See the [Issue Tracker][issue-tracker] for existing bugs and [pull
        requests][pull-requests] for ongoing work. Also check the [Guide] for
        documentation.

        [Guide]: https://gungraun.github.io/gungraun/
        [issue-tracker]: https://github.com/gungraun/gungraun/issues
        [pull-requests]: https://github.com/gungraun/gungraun/pulls
        ```

    - **Rationale:** Keeps content readable, makes URL updates easier, and
      follows the project's existing convention in files like `CONTRIBUTING.md`.

### Naming Conventions

- **Types/Traits:** `UpperCamelCase`
- **Functions/Methods/Modules/Variables:** `snake_case`
- **Constants/Statics:** `SCREAMING_SNAKE_CASE`
- **Files:** `snake_case.rs`

### Error Handling

- **Library (`gungraun`):** Use specific, typed errors where possible.
- **Runner (`gungraun-runner`):**
    - Uses a central `Error` enum in `src/error.rs`.
    - Variants include `BenchmarkError`, `ConfigurationError`, `JobError`
      (wrapping `anyhow`), etc.
    - `JobError` wraps `anyhow::Error` for internal task failures.
    - Implement `Display` for user-facing error messages.
    - Use `thiserror` (if available) or manual `std::error::Error`
      implementation.

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
    - When documenting functions or methods, describe parameters in fluent text
      rather than using structured `# Arguments` sections with parameter lists.
      Parameters should be naturally integrated into the documentation prose.
    - **Rustdoc Links:** Use reference-style links for long paths with multiple
      `::` accessors.
        - Use short form inline: `[`ToolOutputPath`]`
        - Define full path at bottom:
          `[`ToolOutputPath`]: crate::runner::tool::path::ToolOutputPath`
        - Example:

            ```rust
            /// Generates flamegraph summaries using [`Config`] and [`ToolOutputPath`].
            ///
            /// [`Config`]: crate::runner::common::Config
            /// [`ToolOutputPath`]: crate::runner::tool::path::ToolOutputPath
            ```

        - Short, simple types like `Config`, `Header`, `CapturedOutput` can use
          short form inline.
        - Long paths like `crate::runner::tool::path::ToolOutputPath` should use
          reference-style.

    - **Type References in Documentation:** Use rustdoc links for types, not
      parameter names.
        - Use `[`Type`]` for types/structs/enums/variants (e.g., `[`Command`]`,
          `[`Child`]`, `[`Stdin::Setup`]`)
        - Use backticks `` `parameter_name` `` only when referring to the
          parameter itself, not its type
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
- **Benchmarks:** defined using `#[library_benchmark]` and `#[binary_benchmark]`
  attributes.

#### System Test Documentation

System test configuration files (`.conf.yml`) must include a test case
description comment block at the top with the following fields:

| Field             | Required | Purpose                                                                                      |
| ----------------- | -------- | -------------------------------------------------------------------------------------------- |
| Test Case         | Yes      | Unique identifier for the test usually the file name of the test without the `.rs` suffix    |
| Description       | Yes      | Brief explanation of what is being tested                                                    |
| Test Steps        | Yes      | Numbered sequence of actions performed during the test                                       |
| Test Inputs       | Yes      | Specific inputs, configurations, or scenarios being tested                                   |
| Expected Outcomes | Yes      | Clear, measurable expectations for correct execution                                         |
| Preconditions     | No       | Conditions that must be met before test execution                                            |
| Postconditions    | No       | Expected state after test execution (only include if it adds value beyond Expected Outcomes) |
| Test Environment  | No       | Specific environment requirements (e.g., tool versions)                                      |

Do not include a Notes field. Integrate any necessary context into the
appropriate required fields instead.

Example:

```yaml
# Test Case: `test_lib_bench_iter`
#
# Description:
#   Validates the `iter` parameter of library benchmarks (`benches::foo(iter =
#   ...)`), which creates benchmarks for each element of an iterator.
#
# Test Steps:
#   1. Run all benchmarks with no prior baseline
#   2. Run all benchmarks again with the first run as baseline
#
# Test Inputs:
#   - Various iterator configurations: tuples, vectors, ranges, and generic types
#   - Edge cases: empty iterator, single element
#   - Benchmarks with and without setup and teardown functions
#   - Benchmarks with DHAT to test that the iterator allocation itself isn't
#     attributed to the benchmark metrics
#
# Expected Outcomes:
#   - First run: All benchmarks complete successfully (exit code 0)
#   - Second run: All benchmarks complete successfully and compare against the
#     first run without differences
#   - Output matches expected stdout for the corresponding Rust version
#   - No errors or error output
#   - Slightly different output for MSRV and stable in the description of the
#     benchmark (e.g., `vec! [1, 2]` in MSRV vs `vec![1, 2]` in newer versions).
#   - The benchmark order is stable and equals the iterator order.
#   - If the iterator is empty no benchmark should be created for this `benches`
#     directive

groups: ...
```

## 3. Workflow specific

- **Dependencies:** Check `Cargo.toml` before adding new dependencies. Use
  `cargo add` only if necessary and approved.
- **Lockfile:** Do not manually edit `Cargo.lock`.
- **Pre-commit:** Ensure `just fmt` and `just lint` pass before committing.
