use std::io::Error;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<(), Error> {
    println!("Started the timeout program");

    let timeout = std::env::args()
        .nth(1)
        .and_then(|t| t.parse::<u64>().ok())
        .unwrap_or(20000);

    sleep(Duration::from_millis(timeout));

    println!("I terminated normally after a timeout of {timeout} ms");
    Ok(())
}
