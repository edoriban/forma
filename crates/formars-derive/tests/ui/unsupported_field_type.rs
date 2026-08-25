use std::time::SystemTime;

use formars_derive::FormSchema;

#[derive(FormSchema)]
struct Bad {
    when: SystemTime,
}

fn main() {}
