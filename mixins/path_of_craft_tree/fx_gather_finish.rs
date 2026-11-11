fn path_of_craft_tree_fx_gather_finish(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, entity: Option<i32>) {
    if entity.is_none() {
        crate::game::pct_fx_gather_fish::on_eaten(game, stack);
    }
}
