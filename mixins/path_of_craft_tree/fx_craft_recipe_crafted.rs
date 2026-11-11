fn path_of_craft_tree_fx_craft_recipe_crafted(game: &mut crate::game::Game<'_>, recipe: &str, ingredients: &[Option<horizon_client::items::ItemStack>]) {
    crate::game::pct_fx_craft_items::on_recipe_crafted(game, recipe, ingredients);
}
