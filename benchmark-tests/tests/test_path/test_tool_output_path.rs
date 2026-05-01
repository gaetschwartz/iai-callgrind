use ValgrindTool::*;
use gungraun::ValgrindTool;
use rstest::rstest;
use tempfile::tempdir;

use crate::util::fixtures::tool_output_path_f;

#[rstest]
#[case::empty(
    Callgrind,
    &[],
    &[]
)]
#[case::callgrind_out_zero_pid(
    Callgrind,
    &["callgrind.function.bench.out.#0"],
    &[]
)]
#[case::callgrind_out_some_pid(
    Callgrind,
    &["callgrind.function.bench.out.#12345"],
    &[]
)]
#[case::callgrind_out_some_pid_with_trail(
    Callgrind,
    &["callgrind.function.bench.out.#12345-xx-10-rew"],
    &[]
)]
#[case::callgrind_log_zero_pid(
    Callgrind,
    &["callgrind.function.bench.log.#0"],
    &[]
)]
#[case::callgrind_log_some_pid(
    Callgrind,
    &["callgrind.function.bench.log.#12345"],
    &[]
)]
#[case::callgrind_log_some_pid_with_trail(
    Callgrind,
    &["callgrind.function.bench.log.#12345-xx-10-rew"],
    &[]
)]
#[case::callgrind_xtree_some_pid(
    Callgrind,
    &["callgrind.function.bench.xtree.#12345"],
    &[]
)]
#[case::callgrind_xleak_some_pid(
    Callgrind,
    &["callgrind.function.bench.xleak.#12345"],
    &[]
)]
#[case::callgrind_type_does_not_matter_some_pid(
    Callgrind,
    &["callgrind.function.bench.does_not_matter.#12345"],
    &[]
)]
#[case::callgrind_old(
    Callgrind,
    &["callgrind.function.bench.out.old.#12345"],
    &[]
)]
#[case::callgrind_base_foo(
    Callgrind,
    &["callgrind.function.bench.out.base@foo.#12345"],
    &[]
)]
#[case::callgrind_multiple(
    Callgrind,
    &["callgrind.function.bench.out.#12345", "callgrind.function.bench.out.#54321"],
    &[]
)]
#[case::callgrind_multiple_different_types(
    Callgrind,
    &["callgrind.function.bench.out.#12345", "callgrind.function.bench.log.#12354"],
    &[]
)]
#[case::callgrind_dhat_no_clear(
    Callgrind,
    &["dhat.function.bench.out.#12345"],
    &["dhat.function.bench.out.#12345"]
)]
#[case::callgrind_multiple_mixed_dhat_no_clear(
    Callgrind,
    &["callgrind.function.bench.out.#12345", "dhat.function.bench.out.#12345"],
    &["dhat.function.bench.out.#12345"]
)]
#[case::tool_does_not_match_then_no_clear(
    DHAT,
    &["callgrind.function.bench.out.#12345"],
    &["callgrind.function.bench.out.#12345"]
)]
#[case::name_does_not_match_then_no_clear(
    Callgrind,
    &["callgrind.a.b.out.#12345"],
    &["callgrind.a.b.out.#12345"]
)]
#[case::missing_point_then_no_clear(
    Callgrind,
    &["callgrind.function.bench.out#12345"],
    &["callgrind.function.bench.out#12345"]
)]
fn test_clear_temp_files(
    #[case] tool: ValgrindTool,
    #[case] files: &[&str],
    #[case] expected_files: &[&str],
) {
    let temp_dir = tempdir().unwrap();
    let output_path = tool_output_path_f()
        .target_dir(temp_dir.path())
        .name("function.bench")
        .tool(tool)
        .init(true)
        .files(files.iter().map(|f| (*f, "")))
        .fixture();

    output_path
        .clear_temp_files(false)
        .expect("Clearing the temporary files should succeed");

    let dir_entries = std::fs::read_dir(output_path.dir)
        .expect("The output path directory should exist")
        .map(|result| {
            result.map(|d| {
                let path = d.path();
                let file_name = path.file_name().unwrap();
                file_name.to_string_lossy().to_string()
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("Reading the directory should succeed");

    assert_eq!(dir_entries, expected_files);
}
