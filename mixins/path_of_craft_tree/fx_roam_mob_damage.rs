fn path_of_craft_tree_fx_roam_mob_damage(game: &mut crate::game::Game<'_>, amount: f32, from: Option<[f64; 3]>, damage_type: &str, known: Option<Option<u32>>) -> std::ops::ControlFlow<bool> {
    crate::game::pct_fx_roam_combat::on_player_hurt(game, amount, from, damage_type, known)
}
