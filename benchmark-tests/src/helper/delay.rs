use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let mut args = std::env::args().skip(1);
    let delay = args
        .next()
        .expect("The amount of milliseconds to delay the process should be present")
        .parse::<u64>()
        .expect("The delay must be a valid number in milliseconds");
    let exe = args
        .next()
        .expect("The executable to delay should be present");
    let exe_args = args.collect::<Vec<String>>();

    sleep(Duration::from_millis(delay));
    let status = Command::new(exe)
        .args(&exe_args)
        .status()
        .expect("Running the delayed command should succeed");

    std::process::exit(
        status
            .code()
            .expect("The exit code from the delayed command should be present"),
    );
}
