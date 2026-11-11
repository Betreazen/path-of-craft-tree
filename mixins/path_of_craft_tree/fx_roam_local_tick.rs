// Runs in the client library crate, which cannot see the game: the flag is a support module there.
fn path_of_craft_tree_fx_roam_local_tick(world: &mut MotionWorld<'_, '_, '_>) {
    if crate::pct_fx_roam_flags::NO_USE_SLOWDOWN.load(std::sync::atomic::Ordering::Relaxed) {
        world.player.item_use_speed = None;
    }
}
