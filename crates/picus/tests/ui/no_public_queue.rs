//! Removed queue/drain APIs must not be reachable from the facade.
use picus::UiEventQueue;

fn main() {
    let _ = UiEventQueue::default();
}
