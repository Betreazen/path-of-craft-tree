fn path_of_craft_tree_fx_gather_melee(value: (f32, f32, bool, bool), game: &crate::game::Game<'_>, target_kind: &str) -> (f32, f32, bool, bool) {
    crate::game::pct_fx_gather_dig::on_melee_damage(game, target_kind, value)
}
