const VERSION_STR: &'static str = env!("CARGO_PKG_VERSION");

pub fn show_version() {
    println!("rucc (v{})", VERSION_STR);
}

pub fn show_usage() {
    println!("usage: rucc [options] <FILE>");
}
