extern crate rucc;
use rucc::version_info;
use rucc::parser;
use rucc::codegen;

extern crate regex;
use regex::Regex;


fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        version_info::show_version();
        version_info::show_usage();
    } else {
        let ref input_file_name = args[1];
        let ast = parser::run_file(input_file_name.to_string());

        for node in &ast {
            node.show();
        }

        println!("\nllvm-ir test output:");
        unsafe {
            let mut codegen = codegen::Codegen::new("rucc");
            codegen.run(ast);

            let output_file_name = Regex::new(r"\..*$")
                .unwrap()
                .replace_all(input_file_name, ".ll");
            codegen.write_llvmir_to_file(output_file_name.to_string().as_str());
        }
    }
}
