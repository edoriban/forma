/// Boolean schema builder.
pub mod bool;
/// Number schema builder over `f64`/`i64`.
pub mod number;
/// String schema builder.
pub mod string;

pub use bool::{BoolSchema, bool};
pub use number::{NumberSchema, number};
pub use string::{StringSchema, string};
