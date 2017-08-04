extern crate rucc;
use rucc::version_info;
use rucc::parser;
use rucc::codegen;

extern crate regex;
use regex::Regex;

extern crate ansi_term;
use self::ansi_term::Colour;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        version_info::show_version();
        version_info::show_usage();
    } else {
        let ref input_file_name = args[1];
        let ast = parser::Parser::run_file(input_file_name.to_string());

        // DEBUG: for node in &ast {
        // DEBUG:     node.show();
        // DEBUG: }

        // DEBUG: println!("\nllvm-ir test output:");
        unsafe {
            let mut codegen = codegen::Codegen::new("rucc");
            codegen.run(ast);

            let output_file_name = Regex::new(r"\..*$")
                .unwrap()
                .replace_all(input_file_name, ".bc");
            codegen.write_llvm_bitcode_to_file(output_file_name.to_string().as_str());
        }
        println!("{}", Colour::Green.paint("Compiling exited successfully."));
    }
}


#[test]
fn compile_examples() {
    use std::fs;

    let examples_paths = match fs::read_dir("example") {
        Ok(paths) => paths,
        Err(e) => panic!(format!("error: {:?}", e.kind())),
    };
    for path in examples_paths {
        let name = path.unwrap().path().to_str().unwrap().to_string();
        let ast_tree = parser::Parser::run_file(name);
        unsafe {
            codegen::Codegen::new("rucc").run(ast_tree);
        }
    }
}
