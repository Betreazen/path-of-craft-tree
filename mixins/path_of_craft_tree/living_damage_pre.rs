fn path_of_craft_tree_living_damage_pre(entity: i32, kind: &str, damage_type: &str, amount: &mut f32) {
    crate::game::pct_effects::on_final_damage(entity, kind, damage_type, amount);
}
