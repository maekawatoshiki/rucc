mod version_info;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        version_info::show_version();
    } else {
        let input_file_name = args[1].to_string(); // is this correct?
        // TODO: pass lexer input_file_name
    }
}
