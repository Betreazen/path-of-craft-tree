// A piston moves blocks: a placed block keeps its "placed" mark where it lands (mining.rs).
fn path_of_craft_tree_piston_move(
    dimension: horizon_world::level::Dimension,
    direction: [i32; 3],
    push: &[horizon_core::position::BlockPos],
    destroy: &[horizon_core::position::BlockPos],
) {
    let dimension = match dimension {
        horizon_world::level::Dimension::Overworld => "minecraft:overworld",
        horizon_world::level::Dimension::Nether => "minecraft:the_nether",
        horizon_world::level::Dimension::End => "minecraft:the_end",
    };
    let at = |p: &horizon_core::position::BlockPos| [p.x, p.y, p.z];
    crate::game::pct_mining::on_piston_move(
        dimension,
        direction,
        push.iter().map(at).collect(),
        destroy.iter().map(at).collect(),
    );
}
