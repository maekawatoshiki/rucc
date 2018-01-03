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
fn compile_examples() {
    use std::fs;
    use rucc::{codegen, parser};
    use std::process::Command;

    let examples_paths = match fs::read_dir("example") {
        Ok(paths) => paths,
        Err(e) => panic!(format!("error: {:?}", e.kind())),
    };
    for path in examples_paths {
        let name = path.unwrap().path().to_str().unwrap().to_string();
        println!("testing {}...", name);

        // for coverage...
        let ast = parser::run_file(name.to_string());
        unsafe {
            codegen::Codegen::new("test").run(&ast);
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
