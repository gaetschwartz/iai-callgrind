# CI Installation

Gungraun is designed to work in continuous integration environments. This page
covers how to install `gungraun-runner` and Valgrind in your CI pipeline.

## Using the `setup-gungraun` GitHub Action (Recommended)

The [`setup-gungraun`][setup-gungraun] GitHub Action installs both
`gungraun-runner` and Valgrind on Linux runners. It automatically detects the
`gungraun-runner` version that matches your project's `gungraun` library
dependency, so you don't need to keep versions in sync manually.

### Basic Usage

```yaml
- name: Setup gungraun-runner and Valgrind
  uses: gungraun/setup-gungraun@v1
```

By default, the action:

- Installs `gungraun-runner` with the version matching your `gungraun` library
  dependency (tries `binstall`, then `release`, then `source`)
- Installs Valgrind (tries pre-built binaries from
  [gungraun/valgrind-builder][valgrind-builder], then source, then system
  package manager)
- Installs `libc` debug symbols

### Specifying a Runner Version

```yaml
- name: Setup gungraun-runner 0.18.0 and Valgrind
  uses: gungraun/setup-gungraun@v1
  with:
      runner-version: 0.18.0
```

The `runner-version` option accepts a semver version like `0.18.0`, `latest`, or
`auto` (the default, which detects the version from `Cargo.toml`).

### Skipping Valgrind Installation

If Valgrind is already available or you want to install it separately:

```yaml
- name: Setup gungraun-runner only
  uses: gungraun/setup-gungraun@v1
  with:
      valgrind-strategy: none
```

### Building Valgrind from Source

```yaml
- name: Setup gungraun-runner and Valgrind (from source)
  uses: gungraun/setup-gungraun@v1
  with:
      valgrind-strategy: source
      install-build-deps: true
```

### Skipping Runner Installation

```yaml
- name: Setup Valgrind only
  uses: gungraun/setup-gungraun@v1
  with:
      runner-strategy: none
```

### Custom Valgrind Build

```yaml
- name: Setup gungraun-runner and Valgrind (custom binary)
  uses: gungraun/setup-gungraun@v1
  with:
      valgrind-url: https://github.com/custom/valgrind-builder/valgrind-3.23.0-x86_64-linux.tar.gz
      valgrind-sha-url: https://github.com/custom/valgrind-builder/valgrind-3.23.0-x86_64-linux.tar.gz.sha256
```

## Manual Installation

If you prefer not to use the `setup-gungraun` action, you can install
`gungraun-runner` and Valgrind manually.

### Installing `gungraun-runner` from Source

Since the `gungraun-runner` version must match the `gungraun` library version,
it's best to automate this step in CI:

```yaml
- name: Install gungraun-runner
  run: |
      version=$(cargo metadata --format-version=1 |\
        jq '.packages[] | select(.name == "gungraun").version' |\
        tr -d '"'
      )
      cargo install gungraun-runner --version $version
```

### Installing `gungraun-runner` with `binstall`

Speed up installation by using [binstall] with the
[taiki-e/install-action][install-action]:

```yaml
- uses: taiki-e/install-action@cargo-binstall
- name: Install gungraun-runner
  run: |
      version=$(cargo metadata --format-version=1 |\
        jq '.packages[] | select(.name == "gungraun").version' |\
        tr -d '"'
      )
      cargo binstall --no-confirm gungraun-runner --version $version
```

### Installing Valgrind

For manual Valgrind installation options, see
[Prerequisites](./prerequisites.md).

[binstall]: https://github.com/cargo-bins/cargo-binstall
[install-action]: https://github.com/taiki-e/install-action
[setup-gungraun]: https://github.com/gungraun/setup-gungraun
[valgrind-builder]: https://github.com/gungraun/valgrind-builder
