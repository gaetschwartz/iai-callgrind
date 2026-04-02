# Prerequisites

In order to use Gungraun, you must have [Valgrind] installed. This means that
Gungraun cannot be used on platforms that are not supported by Valgrind.

The default benchmarking tool is `Callgrind` and is in most cases perfectly
suited to do the job but if you want or need to use
[`Cachegrind`](../cachegrind.md) instead of `Callgrind` you require valgrind
version `>= 3.22` and client requests (see below).

## Debug Symbols

It's required to run the Gungraun benchmarks with debugging symbols switched on.
For example in your `~/.cargo/config` or your project's `Cargo.toml`:

```toml
[profile.bench]
debug = true
```

Now, all benchmarks which are run with `cargo bench` include the debug symbols.
(See also [Cargo Profiles][cargo-profiles] and [Cargo Config][cargo-config]).

It's required that settings like `strip = true` or other configuration options
stripping the debug symbols need to be disabled explicitly in the `bench`
profile if you have changed this option for the `release` profile. For example:

```toml
[profile.release]
strip = true

[profile.bench]
debug = true
strip = false
```

## Valgrind Client Requests

If you want to make use of [Valgrind Client Requests][valgrind-client-req]
shipped with Gungraun, you also need `libclang` (clang >= 5.0) installed. See
also the requirements of [bindgen] and of [cc]. It's worth noting that you can
use the `Valgrind Client Requests` of Gungraun without the rest of Gungraun by
specifying the `client_requests` feature and disabling the default features.

More details on the usage and requirements of `Valgrind Client Requests` in
[this](../client_requests.md) chapter of the guide.

## Installation of Valgrind

Gungraun is intentionally independent of a specific version of valgrind.
However, Gungraun was only tested with versions of valgrind >= `3.20.0`. It is
therefore highly recommended to use a recent version of valgrind. Also, if you
want or need to, [building valgrind from source][valgrind-source] is usually a
straightforward process. Just make sure the `valgrind` binary is in your `$PATH`
so that Gungraun can find it.

### Installation of Valgrind with Your Package Manager

#### Alpine Linux

```bash
apk add valgrind
```

#### Arch Linux

```bash
pacman -Sy valgrind
```

#### Debian/Ubuntu

```bash
apt-get install valgrind
```

#### Fedora Linux

```bash
dnf install valgrind
```

#### FreeBSD

```bash
pkg install valgrind
```

#### Valgrind is Available for the Following Distributions

[![Packaging status](https://repology.org/badge/vertical-allrepos/valgrind.svg)](https://repology.org/project/valgrind/versions)

[bindgen]: https://rust-lang.github.io/rust-bindgen/requirements.html
[cargo-config]: https://doc.rust-lang.org/cargo/reference/config.html
[cargo-profiles]: https://doc.rust-lang.org/cargo/reference/profiles.html
[cc]: https://github.com/rust-lang/cc-rs
[Valgrind]: https://www.valgrind.org
[valgrind-client-req]:
    https://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.clientreq
[valgrind-source]:
    https://sourceware.org/git/?p=valgrind.git;a=blob;f=README;h=eabcc6ad88c8cab6dfe73cfaaaf5543023c2e941;hb=HEAD
