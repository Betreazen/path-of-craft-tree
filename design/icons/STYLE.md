# Pixel icons for the Path-of-craft-tree skill tree

Every node of the tree gets an original 16×16 pixel icon in the spirit of vanilla Minecraft items: the same chunky pixels, outline and top-left lighting, but our own drawings. Never copy or trace a vanilla texture. A player must tell what a node does from its icon alone, and neighbouring icons must not look alike.

## Format

- Icons live in `design/icons/<sector>.json`: an object of `"<icon id>": [16 strings of 16 characters]`.
- Each character is one pixel from `design/icons/palette.json`. A dot is transparent. No other characters.
- Icon ids and what each node does are in `design/icon-list.json` (`name`, `about`, and for big nodes their `upgrades`). `old_icon` is the vanilla item used until now: a hint at the subject, not something to copy.
- Render and look at your work: `node design/tools/icons.mjs <sector>` from the repository root, then open `design/out/sheet-<sector>.png` (numbers match `sheet-<sector>.txt`). Fix every problem the script prints.

## Drawing rules

1. **Silhouette first.** One main object, large: it fills 12–15 pixels of the 16. It must read at 8×8, because the tree shrinks icons when zoomed out. No tiny scenes, no text, no letters or digits.
2. **Outline.** A continuous 1-pixel outline in `K` around the whole silhouette. Inside, separate parts with the darkest shade of their material, not black.
3. **Light from the top left.** Light shades on top and left edges, dark shades at the bottom and right. 2–4 shades per material. No anti-aliasing, no dithering noise, no stray single pixels.
4. **Vanilla conventions.** Tools and weapons lie on the diagonal with the handle bottom-left and the head top-right. Wooden handles use `b c d`. Iron uses `3 4 5 6`, gold `g h i j`, diamond `l m n o`, and so on, as named in the palette.
5. **Materials mean things.** Use the colour of the thing the node is about: gold for loot and luck, emerald green for experience, lapis for magic and range, red for damage and health, fire colours for heat and speed bursts.

## Badges for repeated ideas

The same idea appears in many branches: tool speed, durability, extra drops, experience. Draw the branch's object (pickaxe, shovel, axe, rod, hoe, and so on) and put a badge from `design/icons/badges.json` in the bottom-right corner, at rows 9–15 and columns 9–15, copied exactly. Clear the object's pixels under the badge so its outline stays whole. The badges:

| Badge | Use for |
| --- | --- |
| speed | faster work, attack speed, draw speed, cooking or brewing speed |
| durability | tools or items wear slower, shield toughness |
| extra | extra drops, double results, more yield |
| xp | more experience |
| time | longer durations |
| save | something is not used up: ingredients, fuel, blocks, arrows |
| chance | luck, odds, rare finds |
| damage | more damage |
| range | radius, reach, distance |
| fire | fire and heat |
| health | health, healing, regeneration |

Use badges mainly for small nodes (`small:` ids). Big nodes (`big:` ids) need their own distinct picture that shows the ability: Жила a pickaxe splitting a vein of ore, Лесоповал a falling tree, and so on. A big node may use a badge only when the picture alone would be unclear.

Branch emblems (`branch:` ids) are bold symbols of the profession, simpler than node icons, with a strong silhouette.
