fn path_of_craft_tree_fx_craft_xp_change(
    value: Option<i32>,
    game: &crate::game::Game<'_>,
) -> Option<i32> {
    let (Some(mut amount), Some(player)) = (value, game.lan_acting_uuid) else {
        return value;
    };
    crate::game::pct_fx_craft_enchant::on_xp_change(player, &mut amount);
    Some(amount)
}
