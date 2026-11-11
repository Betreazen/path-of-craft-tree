fn path_of_craft_tree_fx_roam_launch(game: &mut crate::game::Game<'_>, p: &mut horizon_client::projectiles::Projectile) {
    crate::game::pct_fx_roam_bow::on_launch(game, p);
}
