#![doc = include_str!("../README.md")]

mod api;
mod internal;

pub use api::{AccessFlags, ClassFile, Member, ParseError, parse_class};

type Result<T> = std::result::Result<T, ParseError>;
