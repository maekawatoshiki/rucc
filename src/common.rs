use parser;
use codegen;

extern crate regex;
use self::regex::Regex;

// parse -> codegen -> write llvm bitcode to output file
pub fn run_file<'a>(filename: &'a str) {
    let nodes = parser::Parser::run_file(filename.to_string());

    // DEBUG: for node in &ast {
    // DEBUG:     node.show();
    // DEBUG: }

    // DEBUG: println!("\nllvm-ir test output:");
    unsafe {
        let mut codegen = codegen::Codegen::new("rucc");
        codegen.run(nodes);

        let output_file_name = Regex::new(r"\..*$").unwrap().replace_all(filename, ".bc");
        codegen.write_llvm_bitcode_to_file(output_file_name.to_string().as_str());
    }
}
