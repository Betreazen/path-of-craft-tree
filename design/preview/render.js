// Drawing the tree: the stone backdrop with branch zones, pixel links, node frames, branch banners
// and the energy flowing along learned links. Only what is on screen is drawn.
'use strict';
(() => {
  const { rgb, shade, model } = PCT;
  const TILE = PCT.TILE;
  const tiles = PCT.backdropTiles();
  const FLOW_SPEED = 80; // world units per second
  const FLOW_PERIOD = 110; // world units between particles along a chain

  const screen = (PCT.screen = { W: 0, H: 0, PX: 2, cam: { px: 0, py: 0, zoom: 1 } });
  let staticCanvas;
  let sctx;
  let img;
  let buf;
  let vig;
  let flows = [];

  PCT.toScreen = (x, y) => [screen.W / 2 + (x + screen.cam.px) * screen.cam.zoom, screen.H / 2 + (y + screen.cam.py) * screen.cam.zoom];
  PCT.toWorld = (sx, sy) => [(sx - screen.W / 2) / screen.cam.zoom - screen.cam.px, (sy - screen.H / 2) / screen.cam.zoom - screen.cam.py];

  /** Frame scale for the current zoom: 1–3 full frames, 0.5 compact frames, 0 plain dots. */
  PCT.level = () => {
    const room = model.spacing * screen.cam.zoom * 0.8;
    for (const L of [3, 2, 1]) if (24 * L <= room) return L;
    return room >= 11 ? 0.5 : 0;
  };

  // A 4×4 ordered-dither matrix: the vignette steps through darkness levels in a pixel pattern.
  const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((v) => v / 16);

  PCT.resizeStatic = (W, H) => {
    staticCanvas = document.createElement('canvas');
    staticCanvas.width = W;
    staticCanvas.height = H;
    sctx = staticCanvas.getContext('2d');
    sctx.imageSmoothingEnabled = false;
    img = sctx.createImageData(W, H);
    buf = new Uint32Array(img.data.buffer);
    vig = new Uint8Array(W * H);
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        const r = Math.hypot((x - W / 2) / (W / 2), (y - H / 2) / (H / 2));
        const v = Math.max(0, Math.min(1, (r - 0.45) / 0.85));
        vig[y * W + x] = Math.min(tiles.length - 1, Math.floor(v * (tiles.length - 1) + BAYER[(y & 3) * 4 + (x & 3)]));
      }
    }
  };

  // Per branch and zone level, how much each colour channel of the stone is multiplied by.
  const zoneFactors = model.branches.map((b) => {
    const c = rgb(b.colour);
    return [0, 1, 2, 3, 4].map((k) => {
      const a = k * 0.13;
      return [0, 8, 16].map((s) => 1 - a + (((c >>> s) & 255) / 255) * 2 * a);
    });
  });

  const backdrop = () => {
    const { W, H, cam } = screen;
    const z = model.zones;
    const ox = (((Math.floor(cam.px * cam.zoom * 0.5) % TILE) + TILE) % TILE);
    const oy = (((Math.floor(cam.py * cam.zoom * 0.5) % TILE) + TILE) % TILE);
    const colOf = new Int32Array(W);
    for (let x = 0; x < W; x++) {
      const c = Math.floor(((x - W / 2) / cam.zoom - cam.px - z.x0) / z.cell);
      colOf[x] = c >= 0 && c < z.cols ? c : -1;
    }
    for (let y = 0; y < H; y++) {
      const r = Math.floor(((y - H / 2) / cam.zoom - cam.py - z.y0) / z.cell);
      const row = r >= 0 && r < z.rows ? r * z.cols : -1;
      const ty = ((y - oy) & (TILE - 1)) * TILE;
      for (let x = 0, i = y * W; x < W; x++, i++) {
        let c = tiles[vig[i]][ty + ((x - ox) & (TILE - 1))];
        const col = colOf[x];
        if (row >= 0 && col >= 0) {
          const k = z.level[row + col];
          if (k) {
            const f = zoneFactors[z.branch[row + col]][k];
            const red = Math.min(255, (c & 255) * f[0]);
            const green = Math.min(255, ((c >>> 8) & 255) * f[1]);
            const blue = Math.min(255, ((c >>> 16) & 255) * f[2]);
            c = (0xff000000 | (blue << 16) | (green << 8) | red) >>> 0;
          }
        }
        buf[i] = c;
      }
    }
  };

  const fill = (x, y, w, h, c) => {
    const { W, H } = screen;
    const [x0, x1] = [Math.max(0, x | 0), Math.min(W, (x + w) | 0)];
    if (x0 >= x1) return;
    for (let j = Math.max(0, y | 0), j1 = Math.min(H, (y + h) | 0); j < j1; j++) buf.fill(c, j * W + x0, j * W + x1);
  };

  /** The part of a segment's parameter range [0, 1] inside the screen plus a margin. */
  const clip = (x0, y0, x1, y1, pad) => {
    let [t0, t1] = [0, 1];
    const dx = x1 - x0;
    const dy = y1 - y0;
    const edges = [[-dx, x0 + pad], [dx, screen.W + pad - x0], [-dy, y0 + pad], [dy, screen.H + pad - y0]];
    for (const [p, q] of edges) {
      if (p === 0) {
        if (q < 0) return null;
      } else {
        const t = q / p;
        if (p < 0) t0 = Math.max(t0, t);
        else t1 = Math.min(t1, t);
      }
    }
    return t0 <= t1 ? [t0, t1] : null;
  };

  /** A pixel path walked in blocks of `unit`, `width` blocks thick. */
  const line = (x0, y0, x1, y1, unit, width, c) => {
    const range = clip(x0, y0, x1, y1, width * unit);
    if (!range) return;
    const [ax, ay, bx, by] = [x0 / unit, y0 / unit, x1 / unit, y1 / unit].map(Math.round);
    const [dx, dy] = [bx - ax, by - ay];
    const steps = Math.max(Math.abs(dx), Math.abs(dy), 1);
    const across = Math.floor((width - 1) / 2);
    const flat = Math.abs(dx) >= Math.abs(dy);
    for (let i = Math.floor(range[0] * steps), end = Math.ceil(range[1] * steps); i <= end; i++) {
      const gx = ax + Math.round((dx * i) / steps);
      const gy = ay + Math.round((dy * i) / steps);
      if (flat) fill(gx * unit, (gy - across) * unit, unit, width * unit, c);
      else fill((gx - across) * unit, gy * unit, width * unit, unit, c);
    }
  };

  const LINK = {
    gold: [rgb('#f2bb1d'), rgb('#1a1003')],
    off: [rgb('#9a443e'), rgb('#140707')],
    open: [rgb('#4c9a3e'), rgb('#08100a')],
    plain: [rgb('#5c5c66'), rgb('#0a0a0d')],
  };
  const linkLook = (a, b) => {
    const [fa, fb] = [model.isFull(a), model.isFull(b)];
    if (fa && fb) return model.isOff(a) || model.isOff(b) ? LINK.off : LINK.gold;
    if ((fa && model.canLearn(b)) || (fb && model.canLearn(a))) return LINK.open;
    return LINK.plain;
  };

  const drawLinks = (L, pos) => {
    const unit = L >= 1 ? L : 1;
    const [core, outline] = L >= 1 ? [2, 4] : L > 0 ? [1, 3] : [1, 0];
    const looks = model.edges.map(([a, b]) => linkLook(a, b));
    for (const pass of outline ? [1, 0] : [0]) {
      model.edges.forEach(([a, b], i) => {
        const [p, q] = [pos.get(a), pos.get(b)];
        line(p[0], p[1], q[0], q[1], unit, pass ? outline : core, looks[i][pass]);
      });
    }
  };

  const onScreen = (p, margin) => p[0] > -margin && p[1] > -margin && p[0] < screen.W + margin && p[1] < screen.H + margin;

  const drawDots = (pos) => {
    const big = model.spacing * screen.cam.zoom > 7;
    for (const n of model.nodes) {
      const p = pos.get(n);
      if (!onScreen(p, 8)) continue;
      const s = PCT.STATES[model.state(n)];
      const size = { small: big ? 3 : 2, upgrade: big ? 3 : 2, big: big ? 5 : 4, start: 7 }[n.kind];
      const [x, y] = [Math.round(p[0] - size / 2), Math.round(p[1] - size / 2)];
      if (n.kind === 'big' || n.kind === 'start') fill(x - 1, y - 1, size + 2, size + 2, rgb('#08080a'));
      fill(x, y, size, size, model.state(n) === 'locked' ? s.mid : s.hi);
    }
  };

  /** Where a node's sprite goes, snapped to the grid of its chunky pixels. */
  const spot = (p, L) => {
    const u = L >= 1 ? L : 1;
    return [Math.round(p[0] / u) * u, Math.round(p[1] / u) * u];
  };

  PCT.spriteOf = (n, L) => PCT.nodeSprite(n.kind, model.state(n), n.icon, model.rank(n), n.ranks, L);

  const drawNodes = (L, pos) => {
    const margin = 40 * Math.max(1, L);
    for (const n of model.nodes) {
      const p = pos.get(n);
      if (!onScreen(p, margin)) continue;
      const sprite = PCT.spriteOf(n, L);
      const [x, y] = spot(p, L);
      sctx.drawImage(sprite.canvas, x - sprite.cx, y - sprite.cy);
    }
  };

  /** White pulled towards the branch colour by `k`, as '#rrggbb'. */
  const tint = (hex, k) => {
    const c = PCT.mix(rgb('#ffffff'), rgb(hex), k);
    return `#${[0, 8, 16].map((s) => ((c >>> s) & 255).toString(16).padStart(2, '0')).join('')}`;
  };
  PCT.tint = tint;

  /** Banners where branches leave the centre; zoomed far out, just the names over each branch. */
  const drawBranches = (L) => {
    const near = L > 0;
    const scale = Math.max(1, Math.min(L, 2));
    const labels = [];
    for (const b of model.branches) {
      const at = PCT.toScreen(near ? b.anchor.x : b.label.x, near ? b.anchor.y : b.label.y);
      if (!onScreen(at, 120)) continue;
      const label = PCT.text(b.name, tint(b.colour, 0.35), true);
      if (!near) {
        labels.push([label, Math.round(at[0] - label.width / 2), Math.round(at[1] - label.height / 2)]);
        continue;
      }
      const banner = PCT.bannerSprite(b.icon, scale, b.colour);
      const [x, y] = spot(at, scale);
      sctx.drawImage(banner.canvas, x - banner.cx, y - banner.cy);
      labels.push([label, Math.round(x - label.width / 2), y - banner.cy + banner.canvas.height + 1]);
    }
    for (const [label, x, y] of labels) sctx.drawImage(label, x, y);
  };

  /** Learned links in the order energy flows: from the end nearer the start to the other. */
  const collectFlows = (pos) => {
    flows = [];
    for (const [a, b] of model.edges) {
      if (!model.isFull(a) || !model.isFull(b) || model.isOff(a) || model.isOff(b)) continue;
      const [from, to] = a.dist <= b.dist ? [a, b] : [b, a];
      const [p, q] = [pos.get(from), pos.get(to)];
      if (!clip(p[0], p[1], q[0], q[1], 4)) continue;
      flows.push({ p, q, len: Math.hypot(to.x - from.x, to.y - from.y), d0: from.dist });
    }
  };

  /** Redraws the cached layer: backdrop, links, nodes and banners. */
  PCT.renderStatic = () => {
    const L = PCT.level();
    const pos = new Map(model.nodes.map((n) => [n, PCT.toScreen(n.x, n.y)]));
    backdrop();
    drawLinks(L, pos);
    if (L === 0) drawDots(pos);
    sctx.putImageData(img, 0, 0);
    if (L > 0) drawNodes(L, pos);
    drawBranches(L);
    collectFlows(pos);
    return staticCanvas;
  };

  PCT.flowCount = () => flows.length;

  /** One small glowing square per learned link, travelling outward. */
  PCT.drawFlow = (ctx, seconds) => {
    const L = PCT.level();
    const core = L >= 1 ? 2 * L : 2;
    const travelled = seconds * FLOW_SPEED;
    ctx.fillStyle = 'rgba(255, 196, 64, 0.38)';
    const spots = [];
    for (const f of flows) {
      const period = Math.max(FLOW_PERIOD, f.len);
      const s = (((travelled - f.d0) % period) + period) % period;
      if (s > f.len) continue;
      const t = s / f.len;
      const x = Math.round((f.p[0] + (f.q[0] - f.p[0]) * t - core / 2) / (core / 2)) * (core / 2);
      const y = Math.round((f.p[1] + (f.q[1] - f.p[1]) * t - core / 2) / (core / 2)) * (core / 2);
      if (x < -core || y < -core || x > screen.W || y > screen.H) continue;
      spots.push(x, y);
      ctx.fillRect(x - 1, y - 1, core + 2, core + 2);
    }
    ctx.fillStyle = '#fff4b8';
    for (let i = 0; i < spots.length; i += 2) ctx.fillRect(spots[i], spots[i + 1], core, core);
  };

  PCT.shadeHex = (hex, k) => PCT.css(shade(rgb(hex), k));
})();
