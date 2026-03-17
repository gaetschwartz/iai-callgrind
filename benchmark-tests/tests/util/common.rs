use std::path::PathBuf;

use gungraun::ValgrindTool;
use gungraun_runner::runner::tasks::ProcessHandler;
use lazy_static::lazy_static;

pub const DEFAULT_TOOL: ValgrindTool = ValgrindTool::Callgrind;

#[macro_export]
macro_rules! assert_not_elapsed {
    ($time:expr, $body:expr) => {{
        let start = std::time::Instant::now();
        let result = $body;
        let elapsed = start.elapsed();
        assert!(elapsed < $time);
        result
    }};
}

lazy_static! {
    pub static ref BENCH_BIN_FAKE_EXE: PathBuf =
        PathBuf::from(env!("CARGO_BIN_EXE_bench-bin-fake"));
    pub static ref ECHO_EXE: PathBuf = PathBuf::from(env!("CARGO_BIN_EXE_echo"));
    pub static ref TIMEOUT_EXE: PathBuf = PathBuf::from(env!("CARGO_BIN_EXE_timeout"));
    pub static ref DELAY_EXE: PathBuf = PathBuf::from(env!("CARGO_BIN_EXE_delay"));
    pub static ref CAT_EXE: PathBuf = PathBuf::from(env!("CARGO_BIN_EXE_cat"));
    pub static ref EXIT_WITH_EXE: PathBuf = PathBuf::from(env!("CARGO_BIN_EXE_exit-with"));
    pub static ref FILE_EXISTS_EXE: PathBuf = PathBuf::from(env!("CARGO_BIN_EXE_file-exists"));
}

/// Cleanup the process handler child processes to avoid zombie processes if possible
pub fn cleanup_test_process_handler(process_handler: ProcessHandler) {
    if let Some((_, mut child)) = process_handler.setup {
        child
            .kill()
            .expect("Killing the setup process should succeed");
        child
            .wait()
            .expect("Waiting for the setup process should succeed");
    }
    if let Some(tool_command_child) = process_handler.bench {
        if let Some(mut child) = tool_command_child.child {
            child
                .kill()
                .expect("Killing the benchmark process should succeed");
            child
                .wait()
                .expect("Waiting for the benchmark process should succeed");
        }
    }
    if let Some((_, mut child)) = process_handler.teardown {
        child
            .kill()
            .expect("Killing the teardown process should succeed");
        child
            .wait()
            .expect("Waiting for the teardown process should succeed");
    }
}
