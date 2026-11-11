fn path_of_craft_tree_interact_entity(game: &mut crate::game::Game<'_>, entity: i32, kind: &str, stack: Option<&horizon_client::items::ItemStack>) {
    crate::game::pct_actions::on_interact_entity(game, entity, kind, stack);
}
