'use strict';
// The game's own sprite atlas (data/atlas.png), embedded in data.js: frames, icons, banners.
(function () {
  const S = window.PCTS;
  const atlas = new Image();
  const ready = new Promise((resolve, reject) => {
    atlas.onload = resolve;
    atlas.onerror = () => reject(new Error('атлас иконок не загрузился'));
  });
  atlas.src = S.atlas;

  /** Draws sprite `key` with its anchor (the atlas's origin point) at (x, y), `scale` times its size. */
  function draw(ctx, key, x, y, scale) {
    const s = S.sprites[key];
    if (!s) return false;
    const [sx, sy, w, h, ox, oy] = s;
    ctx.drawImage(atlas, sx, sy, w, h, x - ox * scale, y - oy * scale, w * scale, h * scale);
    return true;
  }

  /** A small canvas with one sprite, for icons in the page (scaled up by CSS, pixelated). */
  function canvas(key, box) {
    const s = S.sprites[key];
    const c = document.createElement('canvas');
    if (!s) return c;
    const [sx, sy, w, h] = s;
    const side = box || Math.max(w, h);
    c.width = c.height = side;
    const ctx = c.getContext('2d');
    ctx.imageSmoothingEnabled = false;
    const k = Math.min(side / w, side / h);
    ctx.drawImage(atlas, sx, sy, w, h, (side - w * k) / 2, (side - h * k) / 2, w * k, h * k);
    return c;
  }

  window.PCTSprites = { atlas, ready, draw, canvas };
})();
