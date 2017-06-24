pub fn error_exit(line: i32, msg: &str) -> ! {
    println!("error: {}: {}", line, msg);
    panic!();
}
