pub mod ast;
pub mod check;
pub mod diagnostics;
pub mod emit;
mod installation;
pub mod lexer;
pub mod modules;
pub mod parser;
pub mod project;
mod routes;
mod rust_names;
pub mod source;
pub mod symbols;
#[cfg(test)]
mod tests;
