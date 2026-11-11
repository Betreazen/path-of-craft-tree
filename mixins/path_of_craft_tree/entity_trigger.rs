fn path_of_craft_tree_entity_trigger(game: &mut crate::game::Game<'_>, name: &str) {
    crate::game::pct_actions::on_advancement_entity_trigger(game, name);
}
