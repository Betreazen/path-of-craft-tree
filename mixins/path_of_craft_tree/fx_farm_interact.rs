fn path_of_craft_tree_fx_farm_interact(
    game: &mut crate::game::Game<'_>,
    entity: i32,
    kind: &str,
    stack: Option<&horizon_client::items::ItemStack>,
    claimed: &mut bool,
) {
    crate::game::pct_fx_farm_husbandry::on_interact(game, entity, kind, stack, claimed);
}
