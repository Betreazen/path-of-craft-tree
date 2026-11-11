// Node frames, icons, rank badges, branch banners and the stone backdrop, all built pixel by pixel.
'use strict';
(() => {
  const { rgb, shade, mute, Pix, frame, rectDistance, tinyText, tinyWidth } = PCT;
  const ICONS = window.PCT_ICONS || { palette: {}, icons: {} };
  const O = rgb('#08080a');

  const tone = (hi, mid, lo, inner) => ({ hi: rgb(hi), mid: rgb(mid), lo: rgb(lo), in: rgb(inner) });
  /** Frame colours by node state: green can be learned, gold learned, grey locked, red switched off. */
  const STATES = {
    locked: tone('#7a7a84', '#50505a', '#2c2c33', '#141418'),
    available: tone('#8fe873', '#3f9e37', '#1b5419', '#0e1d0d'),
    partial: tone('#8fe873', '#3f9e37', '#1b5419', '#0e1d0d'),
    learned: tone('#ffe680', '#d39b14', '#7a4a00', '#251905'),
    off: tone('#de877d', '#96403a', '#4a1a19', '#1d0a0a'),
  };
  const GOLD = { l: rgb('#ffd85a'), d: rgb('#8a5a00'), m: rgb('#d39b14') };
  PCT.STATES = STATES;

  const bevel = (s) => ({ l: s.hi, d: s.lo, m: s.mid });
  const inset = (s) => ({ l: shade(s.in, 0.45), d: shade(s.in, 2.2), m: s.in });

  // ---- Icons ----
  const iconCache = new Map();
  const colourOf = (ch) => (ICONS.palette[ch] ? rgb(ICONS.palette[ch]) : 0);

  const placeholder = () => {
    const p = new Pix(16, 16);
    frame(p, 3, 3, 10, 10, rectDistance(10, 10, 2), [O, { l: rgb('#8c8c94'), d: rgb('#3e3e46'), m: rgb('#5e5e66') }], rgb('#5a5a62'));
    p.rect(7, 7, 2, 2, rgb('#2e2e34'));
    p.set(7, 7, rgb('#1e1e22'));
    p.set(9, 9, rgb('#7a7a82'));
    return p;
  };

  /** The 16×16 pixels of an icon id, or a neutral socket when the icon is not drawn yet. */
  const iconPix = (id) => {
    if (iconCache.has(id)) return iconCache.get(id);
    const rows = ICONS.icons[id];
    let p;
    if (Array.isArray(rows) && rows.length === 16) {
      p = new Pix(16, 16);
      rows.forEach((row, y) => [...row].forEach((ch, x) => ch !== '.' && p.set(x, y, colourOf(ch))));
    } else p = placeholder();
    iconCache.set(id, p);
    return p;
  };
  PCT.hasIcon = (id) => Array.isArray(ICONS.icons[id]);

  /** Halves an icon, keeping the most common opaque colour of each 2×2 block. */
  const halve = (src) => {
    const p = new Pix(src.w >> 1, src.h >> 1);
    for (let y = 0; y < p.h; y++) {
      for (let x = 0; x < p.w; x++) {
        const block = [src.get(2 * x, 2 * y), src.get(2 * x + 1, 2 * y), src.get(2 * x, 2 * y + 1), src.get(2 * x + 1, 2 * y + 1)].filter(Boolean);
        if (block.length < 2) continue;
        const counts = new Map();
        for (const c of block) counts.set(c, (counts.get(c) || 0) + 1);
        p.set(x, y, [...counts].sort((a, b) => b[1] - a[1])[0][0]);
      }
    }
    return p;
  };

  const recolour = (src, fn) => {
    const p = new Pix(src.w, src.h);
    for (let i = 0; i < src.d.length; i++) if (src.d[i]) p.d[i] = fn(src.d[i]);
    return p;
  };

  const iconFor = (id, state, small) => {
    let p = iconPix(id);
    if (small) p = halve(p);
    if (state === 'locked') return recolour(p, (c) => mute(c, 0.55, 0.58));
    if (state === 'off') return recolour(p, (c) => mute(c, 0.6, 0.75));
    return p;
  };

  // ---- Rank badge ----
  const badgeColours = (rank, ranks) =>
    rank >= ranks
      ? { bg: rgb('#7a4a00'), top: rgb('#a86f00'), ink: rgb('#ffe680') }
      : rank > 0
        ? { bg: rgb('#1b5419'), top: rgb('#2f7f2a'), ink: rgb('#ffffff') }
        : { bg: rgb('#26262c'), top: rgb('#3a3a42'), ink: rgb('#b4b4bc') };

  /** A `rank/ranks` badge whose top-right corner is at (right, top). */
  const badge = (pix, right, top, rank, ranks) => {
    const label = `${rank}/${ranks}`;
    const w = tinyWidth(label) + 4;
    const x = right - w + 1;
    const { bg, top: hi, ink } = badgeColours(rank, ranks);
    frame(pix, x, top, w, 9, rectDistance(w, 9, 1), [O, { l: hi, d: bg, m: bg }], bg);
    tinyText(pix, label, x + 2, top + 2, ink);
  };

  /** Rank pips for the small, zoomed-out frames. */
  const pips = (pix, right, top, rank, ranks) => {
    const w = ranks * 3 + 1;
    const x = right - w + 1;
    pix.rect(x, top, w, 4, O);
    for (let i = 0; i < ranks; i++) pix.rect(x + 1 + i * 3, top + 1, 2, 2, i < rank ? rgb('#ffd85a') : rgb('#4a4a52'));
  };

  // ---- Frames: full size (16 px icons) and compact (8 px icons, for zoomed-out views) ----
  const PAD = 4;

  const fullSmall = (p, s, state) => {
    const rings = [O, bevel(s), s.mid, state === 'partial' ? GOLD : inset(s)];
    frame(p, PAD, PAD, 24, 24, rectDistance(24, 24, 1), rings, s.in);
    return { size: 24, icon: 4 };
  };
  const fullUpgrade = (p, s, state) => {
    const rings = [O, bevel(s), state === 'partial' ? GOLD : s.mid];
    frame(p, PAD + 1, PAD + 1, 22, 22, rectDistance(22, 22, 3), rings, s.in);
    return { size: 24, icon: 4 };
  };
  const tab = (p, x, y, w, h, s) => frame(p, x, y, w, h, rectDistance(w, h, 1), [O], s.hi);
  const rivet = (p, x, y, s) => {
    frame(p, x, y, 4, 4, rectDistance(4, 4, 1), [O], s.mid);
    p.set(x + 1, y + 1, s.hi);
  };
  const fullBig = (p, s, state) => {
    const o = PAD + 2;
    tab(p, o + 10, PAD, 8, 4, s);
    tab(p, o + 10, PAD + 28, 8, 4, s);
    tab(p, PAD, o + 10, 4, 8, s);
    tab(p, PAD + 28, o + 10, 4, 8, s);
    const rings = [O, bevel(s), s.mid, { l: s.mid, d: s.lo, m: s.mid }, state === 'partial' ? GOLD.l : O, inset(s)];
    frame(p, o, o, 28, 28, rectDistance(28, 28, 2), rings, s.in);
    for (const [x, y] of [[o - 1, o - 1], [o + 25, o - 1], [o - 1, o + 25], [o + 25, o + 25]]) rivet(p, x, y, s);
    return { size: 32, icon: 8 };
  };
  const fullStart = (p, s) => {
    const c = PAD + 22;
    for (let k = 0; k < 6; k++) {
      // four spikes pointing out of the centre, like a compass rose
      for (let i = -k; i <= k; i++) {
        const col = Math.abs(i) === k ? O : s.hi;
        p.set(c + i - (i > 0 ? 1 : 0), PAD + k, col);
        p.set(c + i - (i > 0 ? 1 : 0), PAD + 43 - k, Math.abs(i) === k ? O : s.lo);
        p.set(PAD + k, c + i - (i > 0 ? 1 : 0), col);
        p.set(PAD + 43 - k, c + i - (i > 0 ? 1 : 0), Math.abs(i) === k ? O : s.lo);
      }
    }
    const rings = [O, bevel(s), s.mid, { l: s.mid, d: s.lo, m: s.mid }, O, GOLD, inset(s)];
    frame(p, PAD + 4, PAD + 4, 36, 36, rectDistance(36, 36, 9), rings, s.in);
    return { size: 44, icon: 14 };
  };

  const compact = {
    small: (p, s) => (frame(p, PAD, PAD, 12, 12, rectDistance(12, 12, 1), [O, bevel(s)], s.in), { size: 12, icon: 2 }),
    upgrade: (p, s) => (frame(p, PAD, PAD, 12, 12, rectDistance(12, 12, 2), [O, bevel(s)], s.in), { size: 12, icon: 2 }),
    big: (p, s) => (frame(p, PAD, PAD, 16, 16, rectDistance(16, 16, 1), [O, bevel(s), s.mid, O], s.in), { size: 16, icon: 4 }),
    start: (p, s) => (frame(p, PAD, PAD, 20, 20, rectDistance(20, 20, 5), [O, bevel(s), s.mid, O], s.in), { size: 20, icon: 6 }),
  };
  const full = { small: fullSmall, upgrade: fullUpgrade, big: fullBig, start: fullStart };

  const spriteCache = new Map();

  /**
   * The sprite of a node: { canvas, half } where the canvas is centred on the node and `half` is half
   * of the frame's side for hit tests. `level` is 0.5 for compact frames, else the integer scale.
   */
  PCT.nodeSprite = (kind, state, icon, rank, ranks, level) => {
    const key = `${kind}|${state}|${icon}|${rank}/${ranks}|${level}`;
    const hit = spriteCache.get(key);
    if (hit) return hit;
    const small = level < 1;
    const s = STATES[state];
    const p = new Pix(52, 52);
    const shape = (small ? compact : full)[kind](p, s, state);
    p.blit(iconFor(icon, state, small), PAD + shape.icon, PAD + shape.icon);
    const right = PAD + shape.size - 1;
    const top = PAD;
    if (ranks > 1 && small) pips(p, right + 2, top - 2, rank, ranks);
    else if (ranks > 1) badge(p, right + (kind === 'big' ? 1 : 3), top - (kind === 'big' ? 1 : 3), rank, ranks);
    const cropped = crop(p, PAD + shape.size / 2);
    const scale = small ? 1 : level;
    const sprite = { canvas: cropped.pix.canvas(scale), cx: cropped.cx * scale, cy: cropped.cy * scale, half: (shape.size / 2) * scale };
    spriteCache.set(key, sprite);
    return sprite;
  };

  /** Trims empty borders, remembering where the node's centre ends up. */
  const crop = (p, centre) => {
    let [x0, y0, x1, y1] = [p.w, p.h, -1, -1];
    for (let y = 0; y < p.h; y++) {
      for (let x = 0; x < p.w; x++) {
        if (!p.d[y * p.w + x]) continue;
        x0 = Math.min(x0, x);
        y0 = Math.min(y0, y);
        x1 = Math.max(x1, x);
        y1 = Math.max(y1, y);
      }
    }
    const out = new Pix(x1 - x0 + 1, y1 - y0 + 1);
    for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) out.d[(y - y0) * out.w + (x - x0)] = p.d[y * p.w + x];
    return { pix: out, cx: centre - x0, cy: centre - y0 };
  };

  // ---- Branch banner ----
  const IRON = tone('#9a9aa6', '#5c5c68', '#34343c', '#171a26');
  const WOOD = { l: rgb('#a37f48'), d: rgb('#5a3f1e'), m: rgb('#7d5c2f') };
  const bannerCache = new Map();

  /**
   * A hanging banner with the branch emblem, its cloth tinted with the branch colour;
   * { canvas, cx, cy } with the centre on the emblem.
   */
  PCT.bannerSprite = (icon, scale, colour) => {
    const key = `${icon}|${scale}|${colour}`;
    if (bannerCache.has(key)) return bannerCache.get(key);
    const tint = colour ? rgb(colour) : IRON.mid;
    const cloth = (k, base) => PCT.mix(base, shade(tint, k), 0.55);
    const p = new Pix(30, 36);
    const [w, h] = [24, 31];
    const body = rectDistance(w, h, 0);
    const notch = (x, y) => h - 1 - y - (5 - Math.abs(x - (w - 1) / 2) * 0.9);
    const distance = (x, y) => Math.min(body(x, y), Math.floor(notch(x, y)));
    const rings = [O, GOLD, { l: cloth(0.8, IRON.hi), d: cloth(0.35, IRON.lo), m: cloth(0.55, IRON.mid) }, cloth(0.3, IRON.lo)];
    frame(p, 3, 3, w, h, distance, rings, cloth(0.18, IRON.in));
    frame(p, 0, 1, 30, 4, rectDistance(30, 4, 1), [O], WOOD.m);
    p.rect(1, 2, 28, 1, WOOD.l);
    p.blit(iconPix(icon), 7, 9);
    const sprite = { canvas: p.canvas(scale), cx: 15 * scale, cy: 17 * scale };
    bannerCache.set(key, sprite);
    return sprite;
  };

  // ---- Stone backdrop: clumpy dark stone, original procedural pixels, tiling every 128 px ----
  const TILE = 128;
  const STONE = ['#151519', '#1a1a1f', '#1f1f25', '#24242a', '#29292f', '#2f2f36'].map((h) => rgb(h));
  let seed = 1337;
  const random = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);

  /** Tileable value noise: a grid of random values every `step` pixels, blended smoothly. */
  const noise = (step) => {
    const n = TILE / step;
    const grid = Array.from({ length: n * n }, random);
    const at = (i, j) => grid[(((j % n) + n) % n) * n + (((i % n) + n) % n)];
    const ease = (t) => t * t * (3 - 2 * t);
    return (x, y) => {
      const [i, j] = [Math.floor(x / step), Math.floor(y / step)];
      const [u, v] = [ease(x / step - i), ease(y / step - j)];
      const top = at(i, j) * (1 - u) + at(i + 1, j) * u;
      const bottom = at(i, j + 1) * (1 - u) + at(i + 1, j + 1) * u;
      return top * (1 - v) + bottom * v;
    };
  };

  const stoneTile = () => {
    const t = new Uint32Array(TILE * TILE);
    const [broad, mid] = [noise(32), noise(8)];
    const clumps = Array.from({ length: (TILE / 2) * (TILE / 2) }, random);
    for (let y = 0; y < TILE; y++) {
      for (let x = 0; x < TILE; x++) {
        const clump = clumps[(y >> 1) * (TILE / 2) + (x >> 1)];
        let v = 0.6 + broad(x, y) * 1.6 + mid(x, y) * 1.4 + (clump - 0.5) * 1.3;
        if (clump > 0.97) v += 1.2;
        if (clump < 0.03) v -= 1.2;
        t[y * TILE + x] = STONE[Math.max(0, Math.min(STONE.length - 1, Math.round(v)))];
      }
    }
    return t;
  };

  /** The backdrop tile at six darkness levels, for the dithered vignette. */
  PCT.backdropTiles = () => {
    const base = stoneTile();
    return [1, 0.86, 0.74, 0.62, 0.5, 0.4].map((k) => base.map((c) => shade(c, k)));
  };
  PCT.TILE = TILE;
  // The pieces on their own, for design/tools/game-atlas.mjs, which packs them for the game.
  PCT.parts = { full, compact, iconFor, badge, pips, crop, iconPix, PAD };
})();
