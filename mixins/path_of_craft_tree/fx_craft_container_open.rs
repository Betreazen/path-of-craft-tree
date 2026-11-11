fn path_of_craft_tree_fx_craft_container_open(game: &crate::game::Game<'_>, player: u128, menu: &str, pos: Option<[i32; 3]>) {
    crate::game::pct_fx_craft_items::on_container_open(game, player, menu, pos);
}
