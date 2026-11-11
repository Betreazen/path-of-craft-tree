// The window around the tree: header, reset button, preview controls, tooltips, mouse input and
// the URL options used for screenshots (#demo&zoom=1.5&focus=12&hover=12&noflow&fit&px=2).
'use strict';
(() => {
  const { model, screen } = PCT;
  const MIN_ZOOM = 0.08;
  const MAX_ZOOM = 3;
  const ZOOM_PER_STEP = 1.15;
  const CLICK_SLOP = 3;
  const RESPEC_LEVELS = 5;
  const C = {
    white: '#ffffff', grey: '#aaaaaa', dark: '#777777', green: '#55ff55', red: '#ff5555',
    gold: '#ffaa00', aqua: '#55ffff', yellow: '#ffff55',
  };

  const options = Object.fromEntries(
    `${location.search.slice(1)}&${location.hash.slice(1)}`
      .split('&')
      .filter(Boolean)
      .map((part) => {
        const [k, v = ''] = part.split('=');
        return [decodeURIComponent(k), decodeURIComponent(v)];
      }),
  );

  const view = document.getElementById('view');
  const ctx = view.getContext('2d');
  const ui = { mouse: [-99, -99], press: null, travelled: 0, armed: false, flow: !('noflow' in options), pinned: null };
  let staticLayer = null;
  let dirty = true;
  let frameQueued = false;

  const resize = () => {
    const auto = Math.floor(Math.min(innerWidth / 640, innerHeight / 400));
    screen.PX = Math.max(1, Math.min(4, Number(options.px) || auto || 1));
    screen.W = Math.ceil(innerWidth / screen.PX);
    screen.H = Math.ceil(innerHeight / screen.PX);
    view.width = screen.W;
    view.height = screen.H;
    view.style.width = `${screen.W * screen.PX}px`;
    view.style.height = `${screen.H * screen.PX}px`;
    ctx.imageSmoothingEnabled = false;
    PCT.resizeStatic(screen.W, screen.H);
    invalidate();
  };

  const invalidate = () => {
    dirty = true;
    request();
  };
  const request = () => {
    if (frameQueued) return;
    frameQueued = true;
    requestAnimationFrame(frame);
  };

  // ---- Camera, as in view.rs ----
  const cam = screen.cam;
  const zoomAt = (cursor, steps) => {
    const zoom = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, cam.zoom * ZOOM_PER_STEP ** steps));
    const world = PCT.toWorld(cursor[0], cursor[1]);
    cam.px = (cursor[0] - screen.W / 2) / zoom - world[0];
    cam.py = (cursor[1] - screen.H / 2) / zoom - world[1];
    cam.zoom = zoom;
    invalidate();
  };
  const lookAt = (x, y, zoom) => {
    cam.zoom = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, zoom ?? cam.zoom));
    cam.px = -x;
    cam.py = -y;
    invalidate();
  };
  const fitTo = (list, maxZoom = MAX_ZOOM) => {
    const xs = list.map((n) => n.x);
    const ys = list.map((n) => n.y);
    const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
    const zoom = Math.min((screen.W - 40) / Math.max(1, x1 - x0), (screen.H - 70) / Math.max(1, y1 - y0), maxZoom);
    lookAt((x0 + x1) / 2, (y0 + y1) / 2, zoom);
  };
  const fitAll = () => fitTo(model.nodes);

  // ---- Hit tests ----
  const halfOf = (n, L) => {
    if (L > 0) return PCT.spriteOf(n, L).half;
    return n.kind === 'big' || n.kind === 'start' ? 4 : 3;
  };
  const nodeAt = (m) => {
    const L = PCT.level();
    for (let i = model.nodes.length - 1; i >= 0; i--) {
      const n = model.nodes[i];
      const [x, y] = PCT.toScreen(n.x, n.y);
      const half = halfOf(n, L);
      if (Math.abs(m[0] - x) <= half && Math.abs(m[1] - y) <= half) return n;
    }
    return null;
  };
  const inside = (r, m) => m[0] >= r.x && m[0] < r.x + r.w && m[1] >= r.y && m[1] < r.y + r.h;

  // ---- Buttons ----
  const buttons = () => {
    const { W, H } = screen;
    const h = 18;
    const reset = { id: 'reset', x: W - 136, y: 6, w: 130, h, label: ui.armed ? 'Точно сбросить?' : `Сброс: ${RESPEC_LEVELS} ур. опыта`, armed: ui.armed };
    const row = [
      { id: 'demo', label: 'Демо-сборка' },
      { id: 'fit', label: 'Показать всё' },
      { id: 'flow', label: ui.flow ? 'Поток: вкл' : 'Поток: выкл' },
    ];
    let x = 6;
    for (const b of row) {
      Object.assign(b, { x, y: H - h - 6, w: PCT.measure(b.label) + 12, h });
      x += b.w + 4;
    }
    return [reset, ...row];
  };

  const drawButton = (b, hot) => {
    const fillColour = b.armed ? '#8a2a2a' : hot ? '#7c7c86' : '#5e5e66';
    ctx.fillStyle = '#08080a';
    ctx.fillRect(b.x, b.y, b.w, b.h);
    ctx.fillStyle = fillColour;
    ctx.fillRect(b.x + 1, b.y + 1, b.w - 2, b.h - 2);
    ctx.fillStyle = b.armed ? '#c04848' : '#9a9aa4';
    ctx.fillRect(b.x + 1, b.y + 1, b.w - 2, 1);
    ctx.fillRect(b.x + 1, b.y + 1, 1, b.h - 3);
    ctx.fillStyle = b.armed ? '#5a1414' : '#3c3c44';
    ctx.fillRect(b.x + 1, b.y + b.h - 3, b.w - 2, 2);
    if (hot) {
      ctx.strokeStyle = '#ffffff';
      ctx.lineWidth = 1;
      ctx.strokeRect(b.x + 0.5, b.y + 0.5, b.w - 1, b.h - 1);
    }
    const text = PCT.text(b.label, hot ? C.yellow : C.white);
    ctx.drawImage(text, Math.round(b.x + (b.w - text.width) / 2), b.y + Math.round((b.h - 2 - PCT.lineHeight()) / 2) - 1);
  };

  // ---- Header: levels and points, with the hovered node's branch tinted ----
  const drawHeader = (hovered) => {
    let x = 6;
    const y = 7;
    const put = (str, colour) => {
      const t = PCT.text(str, colour);
      ctx.drawImage(t, x, y);
      x += t.width - 4;
      return t;
    };
    const branch = hovered && model.branchOf.get(hovered.branch);
    if (branch) {
      const startX = x;
      put(branch.name, PCT.tint(branch.colour, 0.45));
      put(`: ур. ${model.learnedCount(branch.id)} (0%), очки ∞`, C.white);
      ctx.fillStyle = branch.colour;
      ctx.fillRect(startX, y + PCT.lineHeight() + 2, x - startX, 2);
      x += 14;
    }
    put(`Общие: ур. ${model.learnedCount()}, очки ∞`, C.white);
    const note = PCT.text('Превью окна дерева: очки не ограничены', C.grey, true);
    ctx.drawImage(note, 6, screen.H - 18 - 8 - PCT.lineHeight());
  };

  // ---- Legend of frame colours ----
  const drawLegend = () => {
    const items = [['available', 'можно изучить'], ['learned', 'изучено'], ['locked', 'закрыто'], ['off', 'выключено ПКМ']];
    let x = screen.W - 6;
    const y = screen.H - 20;
    for (const [state, label] of items.reverse()) {
      const t = PCT.text(label, C.grey, true);
      x -= t.width;
      ctx.drawImage(t, x, y + 2);
      const s = PCT.nodeSprite('small', state, 'small:legend', 0, 1, 0.5);
      x -= s.canvas.width + 2;
      ctx.drawImage(s.canvas, x, y + 1);
      x -= 8;
    }
  };

  // ---- Tooltip, styled like a vanilla item tooltip ----
  const percent = (v) => `${v >= 0 ? '+' : ''}${(v * 100).toFixed(1)}%`;
  const tooltipLines = (n) => {
    const lines = [{ text: n.name, colour: n.kind === 'big' ? C.gold : C.white, gap: 2 }];
    const branch = model.branchOf.get(n.branch);
    if (branch) lines.push({ text: branch.name, colour: PCT.tint(branch.colour, 0.55) });
    const repeated = n.ranks > 1 && (n.rank_descriptions || []).includes(n.description);
    if (n.description && !repeated) PCT.wrap(n.description, 210).forEach((text) => lines.push({ text, colour: C.grey }));
    const rank = model.rank(n);
    if (n.ranks > 1) {
      lines.push({ text: `Ранг ${rank}/${n.ranks}`, colour: C.aqua });
      (n.rank_descriptions || []).forEach((d, i) => {
        const colour = i < rank ? C.white : i === rank ? C.green : C.dark;
        PCT.wrap(`${i + 1}: ${d}`, 210).forEach((text) => lines.push({ text, colour }));
      });
    }
    if (n.kind === 'small' && !(n.ranks > 1 && n.rank_descriptions)) {
      Object.values(n.effects || {}).forEach((v) => lines.push({ text: percent(v), colour: C.green }));
    }
    if (n.todo) lines.push({ text: 'Эффект ещё не реализован', colour: C.red });
    lines.push(statusLine(n));
    return lines;
  };

  const statusLine = (n) => {
    const state = model.state(n);
    if (state === 'off') return { text: 'Выключено. ПКМ: включить', colour: C.red };
    if (state === 'learned') return { text: n.kind === 'big' ? 'Изучено. ПКМ: выключить' : 'Изучено', colour: C.grey };
    if (state === 'partial') return { text: `Клик: ранг ${model.rank(n) + 1}/${n.ranks}`, colour: C.green };
    if (state === 'available') return { text: n.ranks > 1 ? `Клик: изучить, ранг 1/${n.ranks}` : 'Клик: изучить', colour: C.green };
    return model.refusal(n) === 'partial'
      ? { text: 'Сначала изучите соседний узел до конца', colour: C.red }
      : { text: 'Нужен соседний узел', colour: C.red };
  };

  const drawTooltip = (n) => {
    const lines = tooltipLines(n).map((l) => ({ ...l, canvas: PCT.text(l.text, l.colour) }));
    const lh = PCT.lineHeight();
    const w = Math.max(...lines.map((l) => l.canvas.width)) - 2;
    const h = lines.reduce((s, l) => s + lh + (l.gap || 0), 0) - 2;
    const x = Math.min(ui.mouse[0] + 12, screen.W - w - 6);
    const y = Math.max(6, Math.min(ui.mouse[1] - 12, screen.H - h - 6));
    ctx.fillStyle = 'rgba(16, 0, 16, 0.94)';
    ctx.fillRect(x - 3, y - 4, w + 6, 1);
    ctx.fillRect(x - 3, y + h + 3, w + 6, 1);
    ctx.fillRect(x - 3, y - 3, w + 6, h + 6);
    ctx.fillRect(x - 4, y - 3, 1, h + 6);
    ctx.fillRect(x + w + 3, y - 3, 1, h + 6);
    for (let j = 0; j < h + 6; j++) {
      const t = j / (h + 5);
      ctx.fillStyle = `rgb(${Math.round(80 - 40 * t)}, ${Math.round(20 - 10 * t)}, ${Math.round(190 - 90 * t)})`;
      ctx.fillRect(x - 3, y - 3 + j, 1, 1);
      ctx.fillRect(x + w + 2, y - 3 + j, 1, 1);
    }
    ctx.fillStyle = 'rgb(80, 20, 190)';
    ctx.fillRect(x - 2, y - 3, w + 4, 1);
    ctx.fillStyle = 'rgb(40, 10, 100)';
    ctx.fillRect(x - 2, y + h + 2, w + 4, 1);
    let ty = y - 1;
    for (const l of lines) {
      ctx.drawImage(l.canvas, x - 1, ty);
      ty += lh + (l.gap || 0);
    }
  };

  // ---- Frame loop ----
  const started = performance.now();
  const hoveredNode = () => {
    if (ui.press && ui.travelled > CLICK_SLOP) return null;
    if (buttons().some((b) => inside(b, ui.mouse))) return null;
    return ui.pinned ?? nodeAt(ui.mouse);
  };

  function frame(now) {
    frameQueued = false;
    if (dirty) {
      staticLayer = PCT.renderStatic();
      dirty = false;
    }
    ctx.drawImage(staticLayer, 0, 0);
    const animate = ui.flow && PCT.flowCount() > 0;
    if (animate) PCT.drawFlow(ctx, ((now ?? performance.now()) - started) / 1000 + Number(options.t || 0));
    const hovered = hoveredNode();
    if (hovered) {
      const L = PCT.level();
      if (L > 0) {
        const sprite = PCT.spriteOf(hovered, L);
        const [x, y] = PCT.toScreen(hovered.x, hovered.y);
        const u = L >= 1 ? L : 1;
        ctx.globalCompositeOperation = 'lighter';
        ctx.globalAlpha = 0.35;
        ctx.drawImage(sprite.canvas, Math.round(x / u) * u - sprite.cx, Math.round(y / u) * u - sprite.cy);
        ctx.globalAlpha = 1;
        ctx.globalCompositeOperation = 'source-over';
      }
    }
    drawHeader(hovered);
    for (const b of buttons()) drawButton(b, !ui.press && inside(b, ui.mouse));
    if (!('nolegend' in options)) drawLegend();
    if (hovered) drawTooltip(hovered);
    view.style.cursor = ui.press && ui.travelled > CLICK_SLOP ? 'grabbing' : hovered ? 'pointer' : 'grab';
    if (animate) request();
  }

  // ---- Input ----
  const toLow = (e) => {
    const r = view.getBoundingClientRect();
    return [Math.floor((e.clientX - r.left) / screen.PX), Math.floor((e.clientY - r.top) / screen.PX)];
  };

  const click = (m) => {
    const b = buttons().find((it) => inside(it, m));
    if (b && b.id === 'reset') {
      if (ui.armed) model.reset();
      ui.armed = !ui.armed;
      return invalidate();
    }
    ui.armed = false;
    if (b && b.id === 'demo') return showDemo();
    if (b && b.id === 'fit') return fitAll();
    if (b && b.id === 'flow') {
      ui.flow = !ui.flow;
      return request();
    }
    const n = nodeAt(m);
    if (n) model.learn(n);
    invalidate();
  };

  view.addEventListener('pointerdown', (e) => {
    ui.mouse = toLow(e);
    ui.pinned = null;
    if (e.button === 2) {
      const n = nodeAt(ui.mouse);
      if (n && model.toggle(n)) invalidate();
      return;
    }
    if (e.button !== 0) return;
    view.setPointerCapture(e.pointerId);
    ui.press = ui.mouse;
    ui.travelled = 0;
  });
  view.addEventListener('pointermove', (e) => {
    const m = toLow(e);
    const [dx, dy] = [(m[0] - ui.mouse[0]), (m[1] - ui.mouse[1])];
    ui.mouse = m;
    ui.pinned = null;
    if (ui.press && (dx || dy)) {
      ui.travelled += Math.abs(dx) + Math.abs(dy);
      cam.px += dx / cam.zoom;
      cam.py += dy / cam.zoom;
      return invalidate();
    }
    request();
  });
  view.addEventListener('pointerup', (e) => {
    if (e.button !== 0 || !ui.press) return;
    const wasClick = ui.travelled <= CLICK_SLOP;
    ui.press = null;
    if (wasClick) click(toLow(e));
    request();
  });
  view.addEventListener('pointerleave', () => {
    ui.mouse = [-99, -99];
    request();
  });
  view.addEventListener('wheel', (e) => {
    e.preventDefault();
    const lines = e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY;
    zoomAt(toLow(e), Math.max(-3, Math.min(3, -lines / 100)));
  }, { passive: false });
  view.addEventListener('contextmenu', (e) => e.preventDefault());
  addEventListener('resize', resize);
  addEventListener('hashchange', () => location.reload());

  const showDemo = () => {
    model.demo();
    // Close enough that frames and banners show, centred between the start and the build.
    const learned = model.nodes.filter(model.isLearned);
    fitTo(learned, 1);
    if (PCT.level() === 0) lookAt(-cam.px, -cam.py, 11 / (model.spacing * 0.8) + 0.02);
  };

  // ---- Start ----
  resize();
  const defaultZoom = Math.max(0.36, Math.min(1, (Math.min(screen.W, screen.H) * 0.45) / (model.firstRing + 60)));
  lookAt(model.start.x, model.start.y, defaultZoom);
  if ('fakeranks' in options && !model.nodes.some((n) => n.ranks > 1)) {
    // Only for checking the badge design before the real data has ranks.
    model.nodes.filter((n) => n.kind === 'small').forEach((n, i) => {
      if (i % 3) return;
      n.ranks = 2;
      const v = Object.values(n.effects || {})[0] || 0.01;
      n.rank_descriptions = [percent(v), percent(v * 2)];
    });
  }
  if ('demo' in options) showDemo();
  if ('fit' in options) fitAll();
  const focusId = options.focus ?? options.hover;
  const focus = focusId !== undefined && model.byId.get(Number(focusId));
  if (options.at) {
    const [x, y] = options.at.split(',').map(Number);
    lookAt(x, y);
  } else if (focus) lookAt(focus.x, focus.y);
  if (options.zoom) lookAt(-cam.px, -cam.py, Number(options.zoom));
  const pinned = options.hover !== undefined && model.byId.get(Number(options.hover));
  if (pinned) {
    ui.pinned = pinned;
    const [x, y] = PCT.toScreen(pinned.x, pinned.y);
    ui.mouse = [Math.round(x) + 4, Math.round(y) + 4];
  }
  frame(performance.now());
  if (document.fonts) {
    const sample = 'Сброс Общие ур. очки abc 0123';
    Promise.race([document.fonts.load(PCT.FONT, sample), new Promise((done) => setTimeout(done, 2500))])
      .catch(() => null)
      .then(() => {
        PCT.fontChanged();
        invalidate();
      });
  }
})();
