use lexer;
use parser;
use codegen;
use std::io::{stderr, Write};

extern crate regex;
use self::regex::Regex;

extern crate ansi_term;
use self::ansi_term::Colour;

use CODEGEN;

// parse -> codegen -> write llvm bitcode to output file
pub fn run_file<'a>(filename: &'a str) {
    // parser::Parser::new(&mut lexer).run(&mut nodes);

    // DEBUG: for node in &ast {
    // DEBUG:     node.show();
    // DEBUG: }

    // DEBUG: println!("\nllvm-ir test output:");
    unsafe {
        let mut nodes = Vec::new();
        let mut lexer = lexer::Lexer::new(filename.to_string());
        let mut parser = parser::Parser::new(&mut lexer);

        loop {
            match parser.read_toplevel(&mut nodes) {
                Err(parser::Error::EOF) => break,
                Err(_) => continue,
                _ => {}
            }
            match CODEGEN.lock().unwrap().run(&nodes) {
                Ok(_) => {}
                // TODO: implement err handler for codegen
                Err(codegen::Error::MsgWithPos(msg, pos)) => {
                    writeln!(
                        &mut stderr(),
                        "{}: {} {}: {}",
                        parser.lexer.get_filename(),
                        Colour::Red.bold().paint("error:"),
                        pos.line,
                        msg
                    ).unwrap();
                    writeln!(
                        &mut stderr(),
                        "{}",
                        parser.lexer.get_surrounding_code_with_err_point(pos.pos)
                    ).unwrap();
                    println!(
                        "{} error{} generated.",
                        parser.err_counts + 1,
                        if parser.err_counts + 1 > 1 { "s" } else { "" }
                    );
                    ::std::process::exit(-1);
                }
                _ => panic!("this is a bug. fix soon"),
            }
            nodes.clear();
        }
        parser.show_total_errors();

        let output_file_name = Regex::new(r"\..*$").unwrap().replace_all(filename, ".bc");
        CODEGEN.lock().unwrap().write_llvm_bitcode_to_file(
            output_file_name.to_string().as_str(),
        );
    }
}
