extern crate rucc;
use rucc::version_info;
use rucc::common;

extern crate ansi_term;
use self::ansi_term::Colour;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        version_info::show_version();
        version_info::show_usage();
    } else {
        let ref input_file_name = args[1];
        common::run_file(input_file_name);
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

        // for coverage...
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
