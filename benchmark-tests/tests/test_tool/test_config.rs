use gungraun_runner::api::{Tool, Tools, ValgrindTool};
use gungraun_runner::fixtures::tool_configs_f;

#[test]
fn test_tool_configs_apply_cli_valgrind_args_to_default_tool() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args(&["--valgrind-args='--trace-children=no --num-callers=50'"])
        .fixture();

    let callgrind_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool == ValgrindTool::Callgrind)
        .expect("callgrind config should be present");

    assert!(!callgrind_config.args.trace_children);
    assert!(
        callgrind_config
            .args
            .other
            .contains(&"--num-callers=50".to_owned())
    );
}

#[test]
fn test_tool_configs_apply_cli_valgrind_args_to_additional_tool() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args(&["--valgrind-args=--trace-children=no"])
        .tools(Tools(vec![Tool::new(ValgrindTool::Memcheck)]))
        .fixture();

    let memcheck_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool == ValgrindTool::Memcheck)
        .expect("memcheck config should be present");

    assert!(!memcheck_config.args.trace_children);
}

#[test]
fn test_tool_configs_cli_tool_args_override_cli_valgrind_args() {
    let tool_configs = tool_configs_f()
        .raw_command_line_args(&[
            "--valgrind-args=--trace-children=no",
            "--callgrind-args=--trace-children=yes",
        ])
        .fixture();

    let callgrind_config = tool_configs
        .0
        .iter()
        .find(|config| config.tool == ValgrindTool::Callgrind)
        .expect("callgrind config should be present");

    assert!(callgrind_config.args.trace_children);
}
