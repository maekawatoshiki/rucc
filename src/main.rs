extern crate rucc;

use rucc::version_info;
use rucc::parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        version_info::show_version();
        version_info::show_usage();
    } else {
        let input_file_name = args[1].to_string(); // is this correct?
        parser::run_file(input_file_name);
    }
}
