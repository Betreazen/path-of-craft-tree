fn path_of_craft_tree_fx_craft_vehicle_damage(game: &crate::game::Game<'_>, kind: &str, damage: &mut f32) {
    crate::game::pct_fx_craft_items::on_vehicle_damage(game, kind, damage);
}
