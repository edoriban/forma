mod hidden {
    use formars_derive::FormSchema;

    // Private struct → companion inherits the same private visibility
    // (`#vis` propagation); it must NOT be nameable outside this module.
    #[derive(FormSchema)]
    pub(crate) struct SemiPrivate {
        s: String,
    }

    #[derive(FormSchema)]
    struct Secret {
        s: String,
    }
}

// Leaking a private companion through a `pub` return type is E0446.
pub fn leak_secret() -> hidden::SecretSchema {
    hidden::SecretSchema::new()
}

fn main() {}
