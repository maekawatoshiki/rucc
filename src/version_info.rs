const VERSION_STR: &'static str = "0.1.1";

pub fn show_version() {
    println!("rucc (v{})", VERSION_STR);
}

pub fn show_usage() {
    println!("usage: rucc [options] <FILE>");
}
