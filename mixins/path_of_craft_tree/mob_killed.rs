fn path_of_craft_tree_mob_killed(game: &mut crate::game::Game<'_>, player: u128, entity: u32, kind: &str) {
    crate::game::pct_actions::on_mob_killed(game, player, entity, kind);
}
