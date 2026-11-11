fn path_of_craft_tree_fx_craft_map_filled(game: &mut crate::game::Game<'_>, player: u128, map_id: i32) {
    crate::game::pct_fx_craft_items::on_map_filled(game, player, map_id);
}
