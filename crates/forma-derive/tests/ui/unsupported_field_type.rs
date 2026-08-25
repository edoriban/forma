use std::time::SystemTime;

use forma_derive::FormSchema;

#[derive(FormSchema)]
struct Bad {
    when: SystemTime,
}

fn main() {}
