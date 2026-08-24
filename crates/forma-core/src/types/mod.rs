/// Boolean schema builder.
pub mod bool;
/// Number schema builder over `f64`/`i64`.
pub mod number;
/// Object schema builder over the ordered `Value::Object` currency.
pub mod object;
/// String schema builder.
pub mod string;

pub use bool::{BoolSchema, bool};
pub use number::{NumberSchema, number};
pub use object::{ObjectSchema, object};
pub use string::{StringSchema, string};
