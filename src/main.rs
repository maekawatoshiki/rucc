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

            let output_file_name = Regex::new(r"\..*$").unwrap().replace_all(
                input_file_name,
                ".bc",
            );
            codegen.write_llvm_bitcode_to_file(output_file_name.to_string().as_str());
        }
        println!("{}", Colour::Green.paint("Compiling exited successfully."));
    }
}


#[test]
fn compile_examples() {
    use std::fs;
    use std::process::Command;

    let examples_paths = match fs::read_dir("example") {
        Ok(paths) => paths,
        Err(e) => panic!(format!("error: {:?}", e.kind())),
    };
    for path in examples_paths {
        let name = path.unwrap().path().to_str().unwrap().to_string();
        println!("testing {}...", name);

        // // for coverage...
        // let ast = parser::Parser::run_file(name.to_string());
        // for node in &ast {
        //     node.show();
        // }
        // unsafe {
        //     codegen::Codegen::new("test").run(ast);
        // }

        Command::new("./rucc.sh")
            .arg(name.to_string())
            .spawn()
            .expect("failed to run")
            .wait()
            .expect("failed to run");
        let output1 = Command::new("./a.out").output().expect("failed to run");
        Command::new("clang")
            .arg(name)
            .arg("-lm")
            .spawn()
            .expect("failed to run")
            .wait()
            .expect("failed to run");
        let output2 = Command::new("./a.out").output().expect("failed to run");
        assert!(output1 == output2);
    }
}
