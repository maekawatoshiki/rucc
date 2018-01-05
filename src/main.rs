extern crate rucc;
use rucc::common;

extern crate ansi_term;
use self::ansi_term::Colour;

extern crate clap;
use clap::{App, Arg};

const VERSION_STR: &'static str = env!("CARGO_PKG_VERSION");

fn main() {
    let mut app = App::new("rucc")
        .version(VERSION_STR)
        .author("uint256_t")
        .about("rucc is a small toy C compiler in Rust")
        .arg(
            Arg::with_name("version")
                .short("v")
                .long("version")
                .help("Show version info"),
        )
        .arg(Arg::with_name("FILE").help("Input file").index(1));
    let app_matches = app.clone().get_matches();

    if app_matches.is_present("version") {
        app.write_version(&mut ::std::io::stdout());
        println!();
    } else if let Some(filename) = app_matches.value_of("FILE") {
        common::run_file(filename);
        println!("{}", Colour::Green.paint("Compiling exited successfully."));
    } else {
        app.print_help();
        println!();
    }
}

#[test]
fn compare_with_clang_output() {
    use std::fs;
    use std::process::Command;
    use rucc::{codegen, lexer, parser};

    let examples_paths = match fs::read_dir("example") {
        Ok(paths) => paths,
        Err(e) => panic!(format!("error: {:?}", e.kind())),
    };
    for path in examples_paths {
        let name = path.unwrap().path().to_str().unwrap().to_string();
        println!("testing {}...", name);

        // for coverage...
        unsafe {
            let mut nodes = Vec::new();
            let mut lexer = lexer::Lexer::new(name.to_string());
            let mut parser = parser::Parser::new(&mut lexer);
            let mut codegen = codegen::Codegen::new("test");
            loop {
                match parser.read_toplevel(&mut nodes) {
                    Err(parser::Error::EOF) => break,
                    Err(_) => continue,
                    _ => {}
                }
                for node in &nodes {
                    node.show();
                }
                match codegen.run(&nodes) {
                    Ok(_) => {}
                    Err(e) => panic!(format!("err in codegen: {:?}", e)),
                }
                nodes.clear();
            }
        }

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
