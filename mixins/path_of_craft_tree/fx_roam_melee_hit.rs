fn path_of_craft_tree_fx_roam_melee_hit(game: &mut crate::game::Game<'_>, index: usize, amount: f32, source: horizon_client::mobs::Damage) -> std::ops::ControlFlow<bool> {
    crate::game::pct_fx_roam_combat::on_melee_hit(game, index, amount, source)
}
