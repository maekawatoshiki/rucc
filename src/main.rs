extern crate rucc;

use rucc::version_info;
use rucc::parser;
use rucc::codegen;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        version_info::show_version();
        version_info::show_usage();
    } else {
        let input_file_name = args[1].to_string(); // is this correct?
        let ast = parser::run_file(input_file_name);

        for node in &ast {
            node.show();
        }

        println!("\nllvm-ir test output:");
        unsafe {
            let mut codegen = codegen::Codegen::new("rucc");
            codegen.run(ast);
        }
    }
}
