# Running Valgrind with a Custom Runner

By default, Gungraun invokes Valgrind directly to run your benchmarks. However,
if Valgrind is not available on your host system - for example, on Windows or in
restricted environments, you can use the `--valgrind-runner` option to run
Valgrind through a container or alternative execution environment.

This is controlled by two command-line options and their respective environment
variables:

- `--valgrind-runner` (env: `GUNGRAUN_VALGRIND_RUNNER`): Specify the runner
  executable
- `--valgrind-runner-args` (env: `GUNGRAUN_VALGRIND_RUNNER_ARGS`): Pass
  additional arguments to the runner

The command-line options take precedence over environment variables.

## How Gungraun Invokes the Runner

When a custom runner is specified, Gungraun invokes it as follows:

```shell
<RUNNER> --allow-aslr=yes|no [RUNNER_ARGS...] -- /path/to/valgrind [VALGRIND_ARGS...] <BENCHMARK> <BENCHMARK_ARGS>
```

Key points:

- `--allow-aslr` reflects the `--allow-aslr` command-line option (ASLR is
  disabled by default for consistent benchmark results). When using a custom
  runner, it's the runner's responsibility to handle this option. Containers
  typically manage ASLR differently than bare metal, so you may need to
  configure this in your container setup if consistent benchmark results are
  critical.
- Arguments from `--valgrind-runner-args` are placed before the `--` separator
- `/path/to/valgrind` is the resolved path to Valgrind on the host, or just
  `valgrind` if not found
- Your normal `--callgrind-args`, `--cachegrind-args`, etc. are passed through
  to Valgrind after the runner. The benchmark binary is passed as Valgrind's
  first positional argument specifying the executable to profile.
- All specified paths in the valgrind arguments are absolute paths.
- Environment variables are still cleared. This means usually the `PATH` is not
  available. You can configure this behavior and the environment variables with
  a [`LibraryBenchmarkConfig`] or [`BinaryBenchmarkConfig`] in the benchmarks.

## Running Valgrind in a Container

> **Warning:** The examples in this section are mostly untested and need to be
> improved. Be prepared to run into errors and need to make changes.

Since Gungraun passes arguments like `--allow-aslr` directly to the runner, you
need a wrapper script that handles these arguments appropriately for your
container runtime.

### Docker/Podman Example

Create a wrapper script `~/bin/docker-valgrind`:

```bash
#!/usr/bin/env bash
# Wrapper script to run Valgrind inside a Docker container

# Gungraun passes:
#   --allow-aslr=yes|no [runner_args...] -- /path/to/valgrind [valgrind_args...]

# Skip the --allow-aslr argument or handle it. We skip it here for the sake of
# simplicity
shift

# Since we haven't passed any arguments to the script we can skip the `--`
# separator with another shift
shift

# Skip the Valgrind path of the host
shift

# Run Valgrind inside the container. The command below assumes you have a Docker
# image with Valgrind installed and Valgrind is available in the PATH of the
# Docker image.
exec /usr/bin/docker run --rm -i valgrind-image valgrind "$@"
```

> **Note:** You'll need a Docker image with Valgrind installed. For example:
>
> ```dockerfile
> FROM ubuntu:24.04
> RUN apt-get update && apt-get install -y libc6-dbg valgrind
> ```
>
> Build and tag the image: `docker build -t valgrind-image .`

Make it executable:

```bash
chmod +x ~/bin/docker-valgrind
```

Then use it with Gungraun:

```bash
cargo bench -- --valgrind-runner=~/bin/docker-valgrind
```

### Passing Additional Arguments

Use `--valgrind-runner-args` to pass additional arguments to your wrapper:

```bash
cargo bench -- \
    --valgrind-runner=~/bin/docker-valgrind \
    --valgrind-runner-args='--volume=/my/project:/project:ro'
```

Your wrapper script would then incorporate these arguments:

```bash
#!/usr/bin/env bash

# Skip --allow-aslr
shift

# These are the runner args (before --)
runner_args=()
while [[ $# -gt 0 ]] && [[ "$1" != "--" ]]; do
    runner_args+=("$1")
    shift
done

# Skip --
shift

# Skip the Valgrind binary of the host
shift

# Run docker with the runner args and Valgrind
exec docker run --rm -i "${runner_args[@]}" valgrind-image valgrind "$@"
```

### Windows Hosts

Valgrind is only available on Linux and FreeBSD. If you're developing on
Windows, you can use WSL2 with Docker or Podman to run Valgrind in a Linux
container:

```bash
# Using Docker Desktop with WSL2 backend
cargo bench -- --valgrind-runner=~/bin/docker-valgrind
```

[`LibraryBenchmarkConfig`]:
    https://docs.rs/gungraun/0.17.2/gungraun/struct.LibraryBenchmarkConfig.html
[`BinaryBenchmarkConfig`]:
    https://docs.rs/gungraun/0.17.2/gungraun/struct.BinaryBenchmarkConfig.html
