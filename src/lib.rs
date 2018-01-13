pub mod common;
pub mod error;
pub mod lexer;
pub mod node;
pub mod parser;
pub mod codegen;
pub mod types;

// for LLVMLinkInInterpreter
#[link(name = "ffi")]
extern "C" {}

#[macro_use]
extern crate lazy_static;

extern crate itertools;

use std::sync::Mutex;
use std::marker::Send;

unsafe impl Send for codegen::Codegen {}

lazy_static! {
    static ref CODEGEN: Mutex<codegen::Codegen> = {
        unsafe {
            Mutex::new(codegen::Codegen::new("rucc"))
        }
    };
}
