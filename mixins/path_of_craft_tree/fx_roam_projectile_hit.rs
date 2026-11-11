// The shooter's UUID is only reachable from here: killer_uuid is private to this engine module.
fn path_of_craft_tree_fx_roam_projectile_hit(game: &mut crate::game::Game<'_>, p: &horizon_client::projectiles::Projectile, hit: horizon_client::projectiles::Hit) -> std::ops::ControlFlow<Result<()>> {
    let shooter = game.killer_uuid(p.guest);
    crate::game::pct_fx_roam_bow::on_projectile_hit(game, p, hit, shooter)
}
