use formars_derive::FormSchema;

#[derive(FormSchema)]
#[form(deny_unknown_fields)]
struct Config {
    retries: u32,
}

fn main() {}
