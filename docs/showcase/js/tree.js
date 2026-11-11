'use strict';
// The tree on the page: the game's frames, icons and colour zones drawn on a canvas, with the
// learning rules of the game window (neighbours, ranks, big nodes switched off) and free points.
(function () {
  const S = window.PCTS;
  const SP = window.PCTSprites;
  const byId = new Map(S.nodes.map((n) => [n.id, n]));
  const ORDER = { small: 0, upgrade: 1, big: 2, start: 3 };
  const DRAW = [...S.nodes].sort((a, b) => ORDER[a.k] - ORDER[b.k]);
  const SIDE = { small: 24, upgrade: 24, big: 32, start: 44 };
  const COMPACT_SIDE = { small: 12, upgrade: 12, big: 16, start: 20 };
  const STAGE_RATIO = 340 / 560;
  const MIN_ZOOM = 0.1;
  const MAX_ZOOM = 3;
  const ZOOM_STEP = 1.25;
  const CLICK_SLOP = 4;
  const ZONE_RADIUS = 95;
  const ZONE_RES = 0.12; // zone map pixels per world unit
  const LINK = { locked: '#3b3b46', open: '#4bd14b', learned: '#f2c230' };
  const SQUARE = { locked: '#55555f', available: '#4bd14b', partial: '#f2c230', learned: '#f2c230', off: '#e0564a' };

  for (const n of S.nodes) n.adj = [];
  const edges = [];
  for (const n of S.nodes) {
    for (const id of n.l || []) {
      const m = byId.get(id);
      if (!m || m === n || n.adj.includes(m)) continue;
      n.adj.push(m);
      m.adj.push(n);
      edges.push([n, m]);
    }
  }

  const bounds = (list) => {
    const xs = list.map((n) => n.x);
    const ys = list.map((n) => n.y);
    return { x0: Math.min(...xs), x1: Math.max(...xs), y0: Math.min(...ys), y1: Math.max(...ys) };
  };
  const WORLD = bounds(S.nodes);

  // ---- Colour zones: one soft blob per node in its branch colour, drawn once per branch, small,
  // and scaled; a branch page draws the other branches' zones faintly ----
  const zoneMaps = new Map();
  function zones(branch) {
    if (zoneMaps.has(branch)) return zoneMaps.get(branch);
    const pad = ZONE_RADIUS * 2;
    const w = Math.ceil((WORLD.x1 - WORLD.x0 + pad * 2) * ZONE_RES);
    const h = Math.ceil((WORLD.y1 - WORLD.y0 + pad * 2) * ZONE_RES);
    const c = document.createElement('canvas');
    c.width = w;
    c.height = h;
    const ctx = c.getContext('2d');
    const r = ZONE_RADIUS * ZONE_RES;
    for (const n of S.nodes) {
      const b = S.branches[n.b];
      if (!b || (branch && n.b !== branch)) continue;
      const x = (n.x - WORLD.x0 + pad) * ZONE_RES;
      const y = (n.y - WORLD.y0 + pad) * ZONE_RES;
      const g = ctx.createRadialGradient(x, y, 0, x, y, r);
      g.addColorStop(0, b.colour + '30');
      g.addColorStop(1, b.colour + '00');
      ctx.fillStyle = g;
      ctx.fillRect(x - r, y - r, r * 2, r * 2);
    }
    const map = { canvas: c, x0: WORLD.x0 - pad, y0: WORLD.y0 - pad };
    zoneMaps.set(branch, map);
    return map;
  }

  /** Where each branch's name goes on the overview: past its farthest node, away from the centre. */
  const LABELS = Object.entries(S.branches).map(([id, b]) => {
    const own = S.nodes.filter((n) => n.b === id);
    const far = own.reduce((a, n) => (Math.hypot(n.x, n.y) > Math.hypot(a.x, a.y) ? n : a), own[0]);
    const d = Math.hypot(far.x, far.y) || 1;
    return { id, name: b.name, colour: b.colour, x: (far.x / d) * (d + 150), y: (far.y / d) * (d + 150) };
  });

  const mix = (hex, t) => {
    const v = parseInt(hex.slice(1), 16);
    const ch = (s) => Math.round(255 + (((v >> s) & 255) - 255) * t);
    return `rgb(${ch(16)},${ch(8)},${ch(0)})`;
  };
  const percent = (v) => (v >= 0 ? '+' : '') + (v * 100).toFixed(1).replace('.', ',') + '%';
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

  class TreeView {
    /**
     * `opts.focus`: a branch id to frame and highlight; `opts.onChange(view)` after a click learns.
     */
    constructor(section, opts) {
      this.opts = opts || {};
      this.focus = this.opts.focus || null;
      this.ranks = new Map([[S.start, 1]]);
      this.off = new Set();
      const stage = section.querySelector('.stage');
      this.view = section.querySelector('.view');
      this.canvas = document.createElement('canvas');
      this.canvas.className = 'tree';
      this.canvas.setAttribute('aria-label', this.focus ? 'Ветвь ' + S.branches[this.focus].name + ' в дереве' : 'Дерево целиком');
      this.view.appendChild(this.canvas);
      this.tip = document.createElement('div');
      this.tip.className = 'tip pct-tip';
      this.tip.hidden = true;
      this.view.appendChild(this.tip);
      this.stage = stage;
      this.ctx = this.canvas.getContext('2d');
      this.hover = null;
      this.bindInput();
      this.resize = () => this.layout();
      window.addEventListener('resize', this.resize);
      this.layout();
      this.home();
    }

    stop() {
      window.removeEventListener('resize', this.resize);
    }

    layout() {
      const w = Math.max(200, this.stage.clientWidth);
      const h = Math.round(window.innerWidth <= 860 ? w * 0.9 : w * STAGE_RATIO);
      const dpr = window.devicePixelRatio || 1;
      this.w = w;
      this.h = h;
      this.canvas.width = Math.round(w * dpr);
      this.canvas.height = Math.round(h * dpr);
      this.canvas.style.width = w + 'px';
      this.canvas.style.height = h + 'px';
      this.dpr = dpr;
      if (this.zoom) this.draw();
    }

    /** The default view: the whole tree, or the focused branch with the centre in sight. */
    home() {
      const list = this.focus ? S.nodes.filter((n) => n.b === this.focus || n.k === 'start') : S.nodes;
      const b = bounds(list);
      const margin = this.focus ? 60 : 200;
      this.cx = (b.x0 + b.x1) / 2;
      this.cy = (b.y0 + b.y1) / 2;
      this.zoom = Math.min(this.w / (b.x1 - b.x0 + margin * 2), this.h / (b.y1 - b.y0 + margin * 2));
      this.draw();
    }

    zoomBy(k, sx, sy) {
      const px = sx ?? this.w / 2;
      const py = sy ?? this.h / 2;
      const [wx, wy] = this.toWorld(px, py);
      this.zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, this.zoom * k));
      this.cx = wx - (px - this.w / 2) / this.zoom;
      this.cy = wy - (py - this.h / 2) / this.zoom;
      this.draw();
    }

    resetLearned() {
      this.ranks = new Map([[S.start, 1]]);
      this.off.clear();
      this.draw();
      if (this.opts.onChange) this.opts.onChange(this);
    }

    toScreen(x, y) {
      return [(x - this.cx) * this.zoom + this.w / 2, (y - this.cy) * this.zoom + this.h / 2];
    }

    toWorld(sx, sy) {
      return [(sx - this.w / 2) / this.zoom + this.cx, (sy - this.h / 2) / this.zoom + this.cy];
    }

    // ---- Learning, as in the game window ----
    rank(n) {
      return this.ranks.get(n.id) || 0;
    }

    full(n) {
      return this.rank(n) >= (n.r || 1);
    }

    state(n) {
      if (this.off.has(n.id)) return 'off';
      const r = this.rank(n);
      if (r >= (n.r || 1)) return 'learned';
      if (r > 0) return 'partial';
      return n.adj.some((m) => this.full(m)) ? 'available' : 'locked';
    }

    learnedCount() {
      let count = 0;
      for (const [id, r] of this.ranks) if (id !== S.start) count += r;
      return count;
    }

    click(n) {
      const st = this.state(n);
      if (st === 'available' || st === 'partial') this.ranks.set(n.id, this.rank(n) + 1);
      else return;
      this.draw();
      if (this.opts.onChange) this.opts.onChange(this);
    }

    toggle(n) {
      if (n.k !== 'big' || this.rank(n) === 0) return;
      if (this.off.has(n.id)) this.off.delete(n.id);
      else this.off.add(n.id);
      this.draw();
    }

    // ---- Drawing ----
    draw() {
      const { ctx, dpr, w, h, zoom } = this;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.imageSmoothingEnabled = true;
      ctx.fillStyle = '#0b0b0e';
      ctx.fillRect(0, 0, w, h);
      const paint = (z, alpha) => {
        const [zx, zy] = this.toScreen(z.x0, z.y0);
        ctx.globalAlpha = alpha;
        ctx.drawImage(z.canvas, zx, zy, (z.canvas.width / ZONE_RES) * zoom, (z.canvas.height / ZONE_RES) * zoom);
        ctx.globalAlpha = 1;
      };
      paint(zones(null), this.focus ? 0.25 : 1);
      if (this.focus) paint(zones(this.focus), 0.8);
      ctx.imageSmoothingEnabled = false;
      this.drawLinks();
      this.drawNodes();
      if (zoom < 0.45) this.drawLabels();
    }

    dim(n) {
      return this.focus && n.b !== this.focus && n.k !== 'start';
    }

    drawLinks() {
      const { ctx, zoom } = this;
      ctx.lineWidth = Math.max(1, Math.min(3, 2 * zoom));
      for (const [a, b] of edges) {
        const fa = this.full(a);
        const fb = this.full(b);
        const kind = fa && fb ? 'learned' : fa || fb ? 'open' : 'locked';
        ctx.globalAlpha = this.dim(a) && this.dim(b) ? 0.35 : 1;
        ctx.strokeStyle = LINK[kind];
        const [x1, y1] = this.toScreen(a.x, a.y);
        const [x2, y2] = this.toScreen(b.x, b.y);
        if (Math.max(x1, x2) < -40 || Math.min(x1, x2) > this.w + 40 || Math.max(y1, y2) < -40 || Math.min(y1, y2) > this.h + 40) continue;
        ctx.beginPath();
        ctx.moveTo(x1, y1);
        ctx.lineTo(x2, y2);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
    }

    drawNodes() {
      const { ctx, zoom } = this;
      const tiny = zoom < 0.3;
      const compact = zoom < 1;
      const scale = compact ? 1 : zoom;
      for (const n of DRAW) {
        const [x, y] = this.toScreen(n.x, n.y);
        if (x < -40 || x > this.w + 40 || y < -40 || y > this.h + 40) continue;
        const st = this.state(n);
        ctx.globalAlpha = this.dim(n) ? 0.35 : 1;
        if (tiny) {
          const side = Math.max(3, Math.round(COMPACT_SIDE[n.k] * zoom * 2.2));
          ctx.fillStyle = '#0d0f14';
          ctx.fillRect(Math.round(x - side / 2) - 1, Math.round(y - side / 2) - 1, side + 2, side + 2);
          ctx.fillStyle = SQUARE[st];
          ctx.fillRect(Math.round(x - side / 2), Math.round(y - side / 2), side, side);
          continue;
        }
        const px = Math.round(x);
        const py = Math.round(y);
        const prefix = compact ? 'c' : '';
        SP.draw(ctx, `${prefix}frame:${n.k}:${st}`, px, py, scale);
        const variant = st === 'locked' ? 'l' : st === 'off' ? 'o' : 'n';
        SP.draw(ctx, `${prefix}icon:${n.i}:${variant}`, px, py, scale);
        if ((n.r || 1) > 1) {
          const key = `${compact ? 'pips' : 'badge'}:${this.rank(n)}/${n.r}`;
          const s = S.sprites[key];
          if (s) {
            const side = compact ? COMPACT_SIDE[n.k] : SIDE[n.k];
            const reach = compact ? 2 : n.k === 'big' ? 1 : 3;
            const right = px + (side / 2 + reach) * scale;
            const top = py - (side / 2 + reach) * scale;
            ctx.drawImage(SP.atlas, s[0], s[1], s[2], s[3], right - s[2] * scale, top, s[2] * scale, s[3] * scale);
          }
        }
      }
      ctx.globalAlpha = 1;
    }

    drawLabels() {
      const { ctx } = this;
      ctx.font = '600 13px "Segoe UI", system-ui, sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.lineJoin = 'round';
      for (const l of LABELS) {
        const [x, y] = this.toScreen(l.x, l.y);
        ctx.globalAlpha = this.focus && l.id !== this.focus ? 0.45 : 1;
        ctx.lineWidth = 4;
        ctx.strokeStyle = '#0b0b0e';
        ctx.strokeText(l.name, x, y);
        ctx.fillStyle = mix(l.colour, 0.55);
        ctx.fillText(l.name, x, y);
      }
      ctx.globalAlpha = 1;
    }

    // ---- Pointer ----
    nodeAt(sx, sy) {
      const [wx, wy] = this.toWorld(sx, sy);
      const reach = Math.max(14 / this.zoom, 12);
      let best = null;
      let bestD = reach;
      for (const n of S.nodes) {
        const d = Math.hypot(n.x - wx, n.y - wy);
        if (d < bestD) {
          best = n;
          bestD = d;
        }
      }
      return best;
    }

    showTip(n, sx, sy) {
      if (!n) {
        this.tip.hidden = true;
        return;
      }
      const b = S.branches[n.b];
      const lines = ['<b class="' + (n.k === 'big' ? 'gold' : '') + '">' + esc(n.n) + '</b>'];
      if (b) lines.push('<div style="color:' + mix(b.colour, 0.55) + '">' + esc(b.name) + '</div>');
      const ranks = n.r || 1;
      const repeated = ranks > 1 && (n.rd || []).includes(n.d);
      if (n.d && !repeated) lines.push('<div class="grey">' + esc(n.d) + '</div>');
      if (ranks > 1) {
        const r = this.rank(n);
        lines.push('<div class="aqua">Ранг ' + r + '/' + ranks + '</div>');
        (n.rd || []).forEach((d, i) => lines.push('<div class="' + (i < r ? '' : i === r ? 'green' : 'dark') + '">' + (i + 1) + ': ' + esc(d) + '</div>'));
      }
      if (n.k === 'small' && !(ranks > 1 && (n.rd || []).length)) (n.e || []).forEach((v) => lines.push('<div class="green">' + percent(v) + '</div>'));
      const st = this.state(n);
      const status = {
        off: ['red', 'Выключено. ПКМ: включить'],
        learned: ['grey', n.k === 'big' ? 'Изучено. ПКМ: выключить' : 'Изучено'],
        partial: ['green', 'Клик: ранг ' + (this.rank(n) + 1) + '/' + ranks],
        available: ['green', ranks > 1 ? 'Клик: изучить, ранг 1/' + ranks : 'Клик: изучить'],
        locked: ['red', 'Нужен соседний узел'],
      }[st];
      if (n.k !== 'start') lines.push('<div class="' + status[0] + '">' + status[1] + '</div>');
      this.tip.innerHTML = lines.join('');
      this.tip.hidden = false;
      const tw = this.tip.offsetWidth;
      const th = this.tip.offsetHeight;
      this.tip.style.left = Math.max(4, Math.min(sx + 14, this.w - tw - 4)) + 'px';
      this.tip.style.top = Math.max(4, Math.min(sy + 14, this.h - th - 4)) + 'px';
    }

    bindInput() {
      const c = this.canvas;
      let drag = null;
      const local = (ev) => {
        const r = c.getBoundingClientRect();
        return [ev.clientX - r.left, ev.clientY - r.top];
      };
      c.addEventListener('pointerdown', (ev) => {
        if (ev.button !== 0) return;
        const [x, y] = local(ev);
        drag = { x, y, cx: this.cx, cy: this.cy, moved: false, id: ev.pointerId };
        c.setPointerCapture(ev.pointerId);
      });
      c.addEventListener('pointermove', (ev) => {
        const [x, y] = local(ev);
        if (drag) {
          if (Math.hypot(x - drag.x, y - drag.y) > CLICK_SLOP) drag.moved = true;
          if (drag.moved) {
            this.cx = drag.cx - (x - drag.x) / this.zoom;
            this.cy = drag.cy - (y - drag.y) / this.zoom;
            this.tip.hidden = true;
            this.draw();
            return;
          }
        }
        const n = this.nodeAt(x, y);
        c.style.cursor = n ? 'pointer' : 'grab';
        this.showTip(n, x, y);
      });
      c.addEventListener('pointerup', (ev) => {
        if (!drag) return;
        const [x, y] = local(ev);
        const moved = drag.moved;
        drag = null;
        if (moved) return;
        const n = this.nodeAt(x, y);
        if (n) {
          this.click(n);
          this.showTip(n, x, y);
        }
      });
      c.addEventListener('pointerleave', () => {
        if (!drag) this.tip.hidden = true;
      });
      c.addEventListener('contextmenu', (ev) => {
        ev.preventDefault();
        const [x, y] = local(ev);
        const n = this.nodeAt(x, y);
        if (n) {
          this.toggle(n);
          this.showTip(n, x, y);
        }
      });
      // Plain wheel scrolls the page; Ctrl and the wheel zooms, as on RustCraft's scenes.
      c.addEventListener(
        'wheel',
        (ev) => {
          if (!ev.ctrlKey) return;
          ev.preventDefault();
          const [x, y] = local(ev);
          this.zoomBy(ev.deltaY < 0 ? ZOOM_STEP : 1 / ZOOM_STEP, x, y);
        },
        { passive: false },
      );
    }
  }

  window.PCTTree = { TreeView, edges, byId };
})();
