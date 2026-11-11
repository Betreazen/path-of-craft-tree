// A stroke comes every ten ticks of use whatever the block's cooldown says, so a fast brush counts each one twice.
fn path_of_craft_tree_fx_gather_brush(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    if !crate::game::pct_fx_gather::brush_fast(game) {
        return;
    }
    let time = game.world.game_time;
    let brush = game.brushing.entry(pos).or_default();
    if time >= brush.cooldown_ends_at {
        brush.count += 1;
    }
}
