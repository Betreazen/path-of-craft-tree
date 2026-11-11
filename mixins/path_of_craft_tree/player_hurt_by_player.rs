fn path_of_craft_tree_player_hurt_by_player(victim_entity: i32) {
    crate::game::pct_effects::on_pvp_hit(victim_entity);
}
