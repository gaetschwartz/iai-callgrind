mod test_when_deprecated {
    use gungraun::main;
    fn some_func() {}
    main!(run = cmd = "some", id = "id", args = []);
}

fn main() {}
