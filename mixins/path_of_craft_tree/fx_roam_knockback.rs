fn path_of_craft_tree_fx_roam_knockback(game: &mut crate::game::Game<'_>, power: f64, xd: f64, zd: f64, source: horizon_client::health::HurtSource<'_>) -> std::ops::ControlFlow<()> {
    crate::game::pct_fx_roam_combat::on_knockback(game, power, xd, zd, source)
}
