fn path_of_craft_tree_use_item_on(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3]) {
    crate::game::pct_actions::on_use_item_on(game, stack, pos);
}
