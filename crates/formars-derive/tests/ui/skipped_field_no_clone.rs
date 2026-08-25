use std::sync::mpsc::Receiver;

use formars_derive::FormSchema;

#[derive(FormSchema)]
struct WithHandle {
    name: String,
    #[form(skip)]
    handle: Receiver<u8>,
}

fn main() {}
