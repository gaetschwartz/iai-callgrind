# Configuration

The configuration of binary benchmarks works the same way as in library
benchmarks with the name changing from `LibraryBenchmarkConfig` to
`BinaryBenchmarkConfig`. Please see
[there](../library_benchmarks/configuration.md) for the basics. However, Binary
benchmarks have some additional configuration possibilities:

- [Delay the Command](./configuration/delay.md)
- [Configure the exit code of the Command](./configuration/exit_code.md).

The [`Sandbox`](../library_benchmarks/configuration/sandbox.md) configuration is
shared with library benchmarks. Use `BinaryBenchmarkConfig::sandbox` instead of
`LibraryBenchmarkConfig::sandbox`.
