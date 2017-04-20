pub mod error;
pub mod lexer;
pub mod node;
pub mod parser;
pub mod version_info;

#[macro_use]
extern crate lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    pub static ref MACRO_MAP: Mutex< HashMap<String, lexer::Macro> > = {
        Mutex::new( HashMap::new() )
    };
}
