// The tree model of the preview: nodes and links, branch colours and zones, and the learning rules
// of the game window (adjacency, ranks, switching big nodes off, reset), with unlimited points.
'use strict';
(() => {
  const TREE = window.PCT_TREE;
  const nodes = TREE.nodes;
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const start = byId.get(TREE.start) ?? nodes.find((n) => n.kind === 'start');
  const ORDER = { small: 0, upgrade: 1, big: 2, start: 3 };

  // ---- Links, made symmetric like the game does ----
  for (const n of nodes) {
    n.adj = [];
    n.ranks = Math.max(1, Math.floor(n.ranks) || 1);
  }
  const edges = [];
  for (const n of nodes) {
    for (const id of n.links || []) {
      const m = byId.get(id);
      if (!m || m === n || n.adj.includes(m)) continue;
      n.adj.push(m);
      m.adj.push(n);
      edges.push([n, m]);
    }
  }

  const length = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);

  /** Shortest distance from the start along links; it decides which way the energy flows. */
  const distances = () => {
    const dist = new Map(nodes.map((n) => [n, Infinity]));
    const done = new Set();
    dist.set(start, 0);
    for (;;) {
      let best = null;
      for (const [n, d] of dist) if (!done.has(n) && d < Infinity && (!best || d < dist.get(best))) best = n;
      if (!best) break;
      done.add(best);
      for (const m of best.adj) dist.set(m, Math.min(dist.get(m), dist.get(best) + length(best, m)));
    }
    return dist;
  };
  const dist = distances();
  for (const n of nodes) n.dist = dist.get(n);

  const median = (values) => values.sort((a, b) => a - b)[Math.floor(values.length / 2)] ?? 40;
  const spacing = median(edges.filter(([a, b]) => a !== start && b !== start).map(([a, b]) => length(a, b)));

  // ---- Branches: angular order, colour wheel, banner anchor and label spot ----
  const hsl = (h, s, l) => {
    const f = (k) => {
      const t = (k + h / 30) % 12;
      const v = l - s * Math.min(l, 1 - l) * Math.max(-1, Math.min(t - 3, 9 - t, 1));
      return Math.round(v * 255).toString(16).padStart(2, '0');
    };
    return `#${f(0)}${f(8)}${f(4)}`;
  };

  /** Where the zoomed-out name goes: towards the branch's centroid, three quarters of the way out. */
  const labelSpot = (own, centroid) => {
    const radii = own.map((n) => Math.hypot(n.x - start.x, n.y - start.y)).sort((a, b) => a - b);
    const r = radii[Math.floor(radii.length * 0.75)] ?? 0;
    const d = Math.hypot(centroid.x - start.x, centroid.y - start.y) || 1;
    return { x: start.x + ((centroid.x - start.x) / d) * r, y: start.y + ((centroid.y - start.y) / d) * r };
  };

  const branches = Object.entries(TREE.branches).map(([id, b]) => {
    const own = nodes.filter((n) => n.branch === id);
    const roots = own.filter((n) => n.adj.includes(start));
    const mean = (list, key) => list.reduce((s, n) => s + n[key], 0) / Math.max(1, list.length);
    const centroid = { x: mean(own, 'x'), y: mean(own, 'y') };
    const toward = roots.length ? { x: mean(roots, 'x'), y: mean(roots, 'y') } : centroid;
    const reach = Math.hypot(toward.x - start.x, toward.y - start.y) || 1;
    const along = Math.min(reach * 0.72, Math.max(reach - 1.5 * spacing, reach * 0.4));
    return {
      id,
      name: b.name,
      icon: b.icon || `branch:${id}`,
      given: b.colour,
      nodes: own,
      reach,
      angle: Math.atan2(centroid.y - start.y, centroid.x - start.x),
      label: labelSpot(own, centroid),
      anchor: { x: start.x + ((toward.x - start.x) / reach) * along, y: start.y + ((toward.y - start.y) / reach) * along },
    };
  });
  // Clockwise from the top, so neighbouring branches get neighbouring hues.
  const fromTop = (a) => (a + Math.PI / 2 + 2 * Math.PI) % (2 * Math.PI);
  branches.sort((a, b) => fromTop(a.angle) - fromTop(b.angle));
  branches.forEach((b, i) => {
    b.index = i;
    b.colour = /^#[0-9a-f]{6}$/i.test(b.given || '') ? b.given : hsl((i * 360) / branches.length, 0.75, 0.5);
  });
  const branchOf = new Map(branches.map((b) => [b.id, b]));
  const firstRing = median(branches.map((b) => b.reach));

  // ---- Colour zones: a coarse world grid tinted by the nearest branch node ----
  const zones = () => {
    const cell = Math.max(12, Math.round(spacing * 0.6));
    const fadeNear = spacing * 1.2;
    const fadeFar = spacing * 4;
    const placed = nodes.filter((n) => branchOf.has(n.branch));
    const xs = placed.map((n) => n.x);
    const ys = placed.map((n) => n.y);
    const x0 = Math.min(...xs) - fadeFar;
    const y0 = Math.min(...ys) - fadeFar;
    const cols = Math.ceil((Math.max(...xs) + fadeFar - x0) / cell);
    const rows = Math.ceil((Math.max(...ys) + fadeFar - y0) / cell);
    const bucket = fadeFar;
    const grid = new Map();
    for (const n of placed) {
      const key = `${Math.floor(n.x / bucket)},${Math.floor(n.y / bucket)}`;
      if (!grid.has(key)) grid.set(key, []);
      grid.get(key).push(n);
    }
    const branch = new Uint8Array(cols * rows);
    const level = new Uint8Array(cols * rows);
    const inner = firstRing * 0.25;
    const full = Math.max(firstRing * 0.85, inner + spacing);
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const [x, y] = [x0 + (c + 0.5) * cell, y0 + (r + 0.5) * cell];
        const [bx, by] = [Math.floor(x / bucket), Math.floor(y / bucket)];
        let best = null;
        let bestD = fadeFar;
        for (let j = -1; j <= 1; j++) {
          for (let i = -1; i <= 1; i++) {
            for (const n of grid.get(`${bx + i},${by + j}`) || []) {
              const d = Math.hypot(n.x - x, n.y - y);
              if (d < bestD) [best, bestD] = [n, d];
            }
          }
        }
        if (!best) continue;
        const outer = 1 - Math.max(0, Math.min(1, (bestD - fadeNear) / (fadeFar - fadeNear)));
        const centre = Math.max(0, Math.min(1, (Math.hypot(x - start.x, y - start.y) - inner) / (full - inner)));
        const k = Math.round(outer * centre * 4);
        if (!k) continue;
        branch[r * cols + c] = branchOf.get(best.branch).index;
        level[r * cols + c] = k;
      }
    }
    return { x0, y0, cell, cols, rows, branch, level };
  };

  // ---- Learning state ----
  let ranks = new Map([[start, 1]]);
  let disabled = new Set();
  let off = new Set();

  const rank = (n) => ranks.get(n) || 0;
  const isFull = (n) => rank(n) >= n.ranks;
  const isLearned = (n) => rank(n) > 0;
  const canLearn = (n) => !isFull(n) && n.adj.some(isFull);

  /** Big nodes switched off and every upgrade behind them, as the game computes it. */
  const switchedOff = () => {
    const out = new Set(disabled);
    const frontier = [...disabled];
    while (frontier.length) {
      for (const m of frontier.pop().adj) {
        if (m.kind === 'upgrade' && !out.has(m)) {
          out.add(m);
          frontier.push(m);
        }
      }
    }
    return out;
  };

  const changed = () => {
    off = switchedOff();
    model.version++;
  };

  const state = (n) => {
    if (isLearned(n) && off.has(n)) return 'off';
    if (isFull(n)) return 'learned';
    if (isLearned(n)) return 'partial';
    return canLearn(n) ? 'available' : 'locked';
  };

  const learn = (n) => {
    if (!canLearn(n)) return false;
    ranks = new Map(ranks).set(n, rank(n) + 1);
    changed();
    return true;
  };

  /** Right click: only an allocated big node switches, like in the game. */
  const toggle = (n) => {
    if (n.kind !== 'big' || !isLearned(n)) return false;
    disabled = new Set(disabled);
    if (!disabled.delete(n)) disabled.add(n);
    changed();
    return true;
  };

  const reset = () => {
    ranks = new Map([[start, 1]]);
    disabled = new Set();
    changed();
  };

  // ---- Demo build: a few branches taken to their first big nodes, with some upgrades ----
  const pathTo = (target) => {
    const parent = new Map();
    const queue = nodes.filter(isFull);
    queue.forEach((n) => parent.set(n, null));
    while (queue.length) {
      const n = queue.shift();
      if (n === target) break;
      for (const m of n.adj) {
        if (!parent.has(m)) {
          parent.set(m, n);
          queue.push(m);
        }
      }
    }
    const path = [];
    for (let n = target; n && !isFull(n); n = parent.get(n)) path.unshift(n);
    return parent.has(target) ? path : [];
  };
  const learnFully = (n) => {
    while (canLearn(n)) learn(n);
  };

  const demo = () => {
    reset();
    const wanted = ['mining', 'woodcutting', 'farming', 'melee', 'enchanting'].filter((id) => branchOf.has(id));
    const picked = wanted.length >= 3 ? wanted : branches.slice(0, 5).map((b) => b.id);
    for (const id of picked) {
      const bigs = nodes.filter((n) => n.branch === id && n.kind === 'big').sort((a, b) => a.dist - b.dist).slice(0, 3);
      for (const big of bigs) {
        pathTo(big).forEach(learnFully);
        big.adj.filter((m) => m.kind === 'upgrade').slice(0, 2).forEach(learnFully);
      }
    }
    const ranked = nodes.find((n) => n.ranks > 1 && canLearn(n) && !isLearned(n));
    if (ranked) learn(ranked);
    const last = nodes.filter((n) => n.kind === 'big' && isFull(n) && n.branch === picked[picked.length - 1]);
    if (last.length) toggle(last[last.length - 1]);
  };

  /** Why a locked node cannot be learned. */
  const refusal = (n) => (n.adj.some((m) => isLearned(m) && !isFull(m)) ? 'partial' : 'unlinked');

  const model = (PCT.model = {
    nodes: [...nodes].sort((a, b) => ORDER[a.kind] - ORDER[b.kind]),
    byId, edges, start, spacing, firstRing, branches, branchOf,
    zones: zones(),
    version: 0,
    rank, isFull, isLearned, canLearn, state, learn, toggle, reset, demo, refusal,
    isOff: (n) => off.has(n),
    learnedCount: (branch) => nodes.filter((n) => n !== start && isLearned(n) && (!branch || n.branch === branch)).length,
  });
})();
