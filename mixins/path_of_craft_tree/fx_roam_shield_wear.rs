fn path_of_craft_tree_fx_roam_shield_wear(player: u128, item: &str, amount: &mut i32) {
    crate::game::pct_fx_roam_combat::on_shield_wear(player, item, *amount);
}
