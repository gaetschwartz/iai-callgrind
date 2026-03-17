fn main() {
    let arg = std::env::args()
        .nth(1)
        .expect("At least one argument with the exit code or `panic` should be present");

    if arg == "panic" {
        panic!("Exited with panic as requested");
    } else if let Ok(code) = arg.parse::<i32>() {
        std::process::exit(code);
    } else {
        panic!("Illegal argument: {arg}");
    }
}
