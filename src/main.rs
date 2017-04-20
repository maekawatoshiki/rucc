extern crate rucc;

use rucc::version_info;
use rucc::parser;

use std::str;


fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        version_info::show_version();
        version_info::show_usage();
    } else {
        let input_file_name = args[1].to_string(); // is this correct?
        parser::run_file(input_file_name);

        println!("parser test:");
        let v = parser::run("1 + (2, 3)".to_string());
        for e in v {
            e.show();
            println!("\nconst eval: {}", e.eval_constexpr());
        }
    }
}
