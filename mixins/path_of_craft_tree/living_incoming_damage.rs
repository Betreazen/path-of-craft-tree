fn path_of_craft_tree_living_incoming_damage(entity: i32, kind: &str, damage_type: &str, amount: &mut f32) {
    crate::game::pct_actions::on_incoming_damage(entity, kind, damage_type, amount);
}
