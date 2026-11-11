fn path_of_craft_tree_player_container_open(player: u128, menu: &str, pos: Option<[i32; 3]>) {
    crate::game::pct_actions::on_container_open(player, menu, pos);
}
