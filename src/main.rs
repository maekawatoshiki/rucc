mod version_info;
mod node;
mod lexer;

use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::str;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        version_info::show_version();
        version_info::show_usage();
    } else {
        let input_file_name = args[1].to_string(); // is this correct?
        let mut file = OpenOptions::new()
            .read(true)
            .open(input_file_name.to_string())
            .unwrap();
        let mut s = String::new();
        file.read_to_string(&mut s);
        let mut lexer = lexer::Lexer::new(input_file_name, s.as_str());
        // test
        let mut tok: Option<lexer::Token>;
        loop {
            tok = lexer.read_token();
            match tok {
                Some(t) => {
                    println!("token: {}", t.val);
                }
                None => break,
            }
        }
    }
}
