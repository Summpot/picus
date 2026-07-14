use picus::UiComponent;

// Authoring components require Default + Clone unless `runtime_only`.
#[derive(Clone, UiComponent)]
struct NoDefault;

fn main() {}
