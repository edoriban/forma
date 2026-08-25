use formars_core::prelude::*;
use formars_ui::*;
use leptos::prelude::*;

#[component]
fn App() -> impl IntoView {
    let mut form = use_form(ValidateOn::Blur);
    let email = form
        .controller
        .register(FieldPath::key("email"), Box::new(string().min(8).email()))
        .expect("fresh registration");
    let controller = form.controller.clone();
    let ctrl_for_outcome = controller.clone();
    let on_outcome =
        move |outcome: SubmitOutcome<(), FormaError>| match outcome {
            SubmitOutcome::Success(()) => {}
            SubmitOutcome::HandlerError(server) => {
                ctrl_for_outcome.apply_server_errors(&server);
            }
            SubmitOutcome::ValidationFailed(err) => {
                let _ = err;
            }
        };
    view! {
        <Form
            controller
            on_outcome
            on_submit=|_snap: FormSnapshot| async { Ok::<(), FormaError>(()) }
            submit_label="Go".to_string()
        >
            <TextField field=email label="Email".to_string() />
        </Form>
    }
}

fn main() {
    mount_to_body(App);
}
