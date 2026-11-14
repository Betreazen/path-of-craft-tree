// An explosion turns this block to air: a placed block's mark goes with it (mining.rs).
fn path_of_craft_tree_explosion_hit_block(game: &crate::game::Game<'_>, pos: [i32; 3]) {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    if !game.res.facts().is_air(state) {
        crate::game::pct_mining::on_explosion_block(game.native_dimension(), pos);
    }
}
