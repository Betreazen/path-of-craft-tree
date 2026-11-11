fn path_of_craft_tree_fx_roam_food(player: u128, from: i32, to: i32, cancel: &mut bool) {
    crate::game::pct_fx_roam_world::on_food_changed(player, from, to, cancel);
}
