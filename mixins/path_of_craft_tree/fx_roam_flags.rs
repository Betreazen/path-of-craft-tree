// Lives in the client library so the movement step, which cannot see the game, can read it.
pub static NO_USE_SLOWDOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
