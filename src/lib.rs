pub mod error;
pub mod lexer;
pub mod node;
pub mod parser;
pub mod codegen;
pub mod types;
pub mod version_info;

use std::collections::HashMap;
use std::sync::Mutex;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref MACRO_MAP: Mutex< HashMap<String, lexer::Macro> > = {
        Mutex::new( HashMap::new() )
    };
}
