fn path_of_craft_tree_fx_farm_moisture(pos: BlockPos, from: i32, to: i32, cancel: &mut bool) {
    crate::game::pct_fx_farm::on_moisture_change(pos.x, pos.z, from, to, cancel);
}
