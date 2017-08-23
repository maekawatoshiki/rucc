use lexer;
use parser;
use codegen;
use std::io::{stderr, Write};

extern crate regex;
use self::regex::Regex;

extern crate ansi_term;
use self::ansi_term::Colour;

// parse -> codegen -> write llvm bitcode to output file
pub fn run_file<'a>(filename: &'a str) {
    let mut lexer = lexer::Lexer::new(filename.to_string());
    let mut parser = parser::Parser::new(&mut lexer);
    // parser::Parser::new(&mut lexer).run(&mut nodes);

    // DEBUG: for node in &ast {
    // DEBUG:     node.show();
    // DEBUG: }

    // DEBUG: println!("\nllvm-ir test output:");
    let mut nodes = Vec::new();
    unsafe {
        let mut codegen = codegen::Codegen::new("rucc");
        loop {
            match parser.read_toplevel(&mut nodes) {
                Err(parser::Error::EOF) => break,
                Err(_) => continue,
                _ => {}
            }
            match codegen.run(&nodes) {
                Ok(_) => {}
                // TODO: implement err handler for codegen
                Err(codegen::Error::MsgWithPos(msg, pos)) => {
                    writeln!(
                        &mut stderr(),
                        "{}: {}: {}",
                        Colour::Red.bold().paint("error:"),
                        pos.line,
                        msg
                    ).unwrap();
                    writeln!(
                        &mut stderr(),
                        "{}",
                        parser.lexer.get_surrounding_code_with_err_point(pos.pos)
                    ).unwrap();
                    ::std::process::exit(-1);
                }
                _ => panic!("this is a bug. fix soon"),
            }
            nodes.clear();
        }
        parser.show_total_errors();

        let output_file_name = Regex::new(r"\..*$").unwrap().replace_all(filename, ".bc");
        codegen.write_llvm_bitcode_to_file(output_file_name.to_string().as_str());
    }
}
