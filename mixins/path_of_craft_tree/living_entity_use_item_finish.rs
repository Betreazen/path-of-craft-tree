fn path_of_craft_tree_living_entity_use_item_finish(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) {
    crate::game::pct_actions::on_eat(game, stack);
}
