pub mod error;
pub mod lexer;
pub mod node;
pub mod parser;
pub mod codegen;
pub mod types;
pub mod version_info;

use std::collections::HashMap;
use std::sync::Mutex;
use std::rc::Rc;
use std::collections::VecDeque;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref MACRO_MAP: Mutex< HashMap<String, lexer::Macro> > = {
        Mutex::new( HashMap::new() )
    };
    pub static ref ENV_MAP: Mutex< VecDeque<HashMap<String, node::AST>> > = {
        let mut vecdeque = VecDeque::new();
        vecdeque.push_back(HashMap::new()); // global env
        Mutex::new( vecdeque )
    };
}
