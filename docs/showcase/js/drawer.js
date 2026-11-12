'use strict';
// A node's details in a panel, in the manner of Vaul: from the right on wide screens, a bottom
// sheet on phones that a swipe down closes. Esc closes it, Tab stays inside, focus comes back.
(function () {
  const S = window.PCTS;
  const SP = window.PCTSprites;
  const byId = new Map(S.nodes.map((n) => [n.id, n]));
  const KIND = { small: 'Малый узел', big: 'Большой узел', upgrade: 'Усилитель', start: 'Начало дерева' };
  const SHEET = '(max-width: 860px)';
  const SWIPE_CLOSE_PX = 90;
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
  const percent = (v) => (v >= 0 ? '+' : '') + String(Math.round(v * 1000) / 10).replace('.', ',') + '%';

  // Steps from the start to every node, through the links (tree.js filled `adj`).
  let steps = null;
  function stepsFromStart() {
    if (steps) return steps;
    steps = new Map([[S.start, 0]]);
    const queue = [byId.get(S.start)];
    while (queue.length) {
      const n = queue.shift();
      for (const m of n.adj || []) {
        if (steps.has(m.id)) continue;
        steps.set(m.id, steps.get(n.id) + 1);
        queue.push(m);
      }
    }
    return steps;
  }

  /** The big node this node upgrades, and the upgrades of a big node. */
  function family(n) {
    const bigs = (S.branches[n.b] && S.branches[n.b].bigs) || [];
    if (n.k === 'big') return { ups: (bigs.find((x) => x.id === n.id) || { ups: [] }).ups.map((id) => byId.get(id)) };
    if (n.k === 'upgrade') {
      const owner = bigs.find((x) => x.ups.includes(n.id));
      return { big: owner && byId.get(owner.id) };
    }
    return {};
  }

  let root = null;
  let lastFocus = null;
  let current = null;
  let options = {};

  function build() {
    root = document.createElement('div');
    root.className = 'pct-drawer-root';
    root.hidden = true;
    root.innerHTML =
      '<div class="pct-scrim" data-close></div>' +
      '<aside class="pct-drawer" role="dialog" aria-modal="true" aria-labelledby="pct-drawer-title">' +
      '<div class="grip" aria-hidden="true"></div>' +
      '<button type="button" class="x" data-close aria-label="Закрыть">×</button>' +
      '<div class="body"></div></aside>';
    document.body.appendChild(root);
    root.addEventListener('click', (ev) => {
      if (ev.target.closest('[data-close]')) close();
      const go = ev.target.closest('[data-node]');
      if (go) open(+go.dataset.node, options);
      if (ev.target.closest('[data-show]') && options.onShow) {
        options.onShow(current);
        close();
      }
    });
    root.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') {
        ev.preventDefault();
        close();
      } else if (ev.key === 'Tab') trap(ev);
    });
    swipe(root.querySelector('.pct-drawer'));
  }

  function trap(ev) {
    const items = [...root.querySelectorAll('a[href], button')].filter((e) => !e.disabled && e.offsetParent);
    if (!items.length) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (ev.shiftKey && document.activeElement === first) {
      ev.preventDefault();
      last.focus();
    } else if (!ev.shiftKey && document.activeElement === last) {
      ev.preventDefault();
      first.focus();
    }
  }

  /** A sheet follows the finger down from its grip and top, and closes past a short distance. */
  function swipe(panel) {
    let start = null;
    panel.addEventListener('pointerdown', (ev) => {
      if (!matchMedia(SHEET).matches || ev.pointerType === 'mouse') return;
      if (!ev.target.closest('.grip, .head') && panel.querySelector('.body').scrollTop > 0) return;
      start = { y: ev.clientY, id: ev.pointerId };
      panel.classList.add('dragging');
    });
    panel.addEventListener('pointermove', (ev) => {
      if (!start || ev.pointerId !== start.id) return;
      const dy = Math.max(0, ev.clientY - start.y);
      panel.style.transform = 'translateY(' + dy + 'px)';
    });
    const end = (ev) => {
      if (!start || ev.pointerId !== start.id) return;
      const dy = ev.clientY - start.y;
      start = null;
      panel.classList.remove('dragging');
      panel.style.transform = '';
      if (dy > SWIPE_CLOSE_PX) close();
    };
    panel.addEventListener('pointerup', end);
    panel.addEventListener('pointercancel', end);
  }

  function icon(n, side) {
    const c = SP.canvas('icon:' + n.i + ':n', 16);
    c.style.width = c.style.height = side + 'px';
    return c;
  }

  // Icons are canvases, which markup cannot carry: a placeholder names the node, filled after.
  const nodeRow = (n) =>
    '<button type="button" class="node-link" data-node="' + n.id + '"><span data-icon-node="' + n.id + '"></span>' + esc(n.n) + '</button>';

  function fill(n) {
    const b = S.branches[n.b];
    const body = root.querySelector('.body');
    const ranks = n.r || 1;
    const rows = [];
    rows.push('<div class="head"><span class="big-icon"></span><div><h2 id="pct-drawer-title">' + esc(n.n) + '</h2>');
    rows.push('<div class="chips">');
    if (b) rows.push('<a class="chip-branch" href="#' + n.b + '"><i style="background:' + b.colour + '"></i>' + esc(b.name) + '</a>');
    rows.push('<span class="chip-kind kind-' + n.k + '">' + KIND[n.k] + '</span>');
    if (ranks > 1) rows.push('<span class="chip-kind">' + ranks + ' ранга</span>');
    rows.push('</div></div></div>');
    if (n.d) rows.push('<p class="what">' + esc(n.d) + '</p>');
    if (ranks > 1 && (n.rd || []).length) {
      rows.push('<h3>По рангам</h3><ol class="ranks">' + n.rd.map((d) => '<li>' + esc(d) + '</li>').join('') + '</ol>');
    }
    if (n.k === 'small' && (n.e || []).length && !(n.rd || []).length) {
      rows.push('<h3>Прибавка</h3><p class="fx">' + n.e.map(percent).join(', ') + (ranks > 1 ? ' за ранг' : '') + '</p>');
    }
    const far = stepsFromStart().get(n.id);
    if (far != null && n.k !== 'start') {
      rows.push('<h3>Путь</h3><p>' + far + ' ' + plural(far, 'узел', 'узла', 'узлов') + ' от начала дерева, включая этот.</p>');
    }
    const fam = family(n);
    if (fam.big) rows.push('<h3>Усиливает</h3>' + nodeRow(fam.big));
    if (fam.ups && fam.ups.length) rows.push('<h3>Усилители за ним</h3><div class="ups">' + fam.ups.map(nodeRow).join('') + '</div>');
    rows.push('<div class="acts">');
    if (options.onShow) rows.push('<button type="button" class="act" data-show>Показать на дереве</button>');
    if (b && location.hash.split('?')[0] !== '#' + n.b) rows.push('<a class="act" href="#' + n.b + '?node=' + n.id + '">Страница ветви «' + esc(b.name) + '»</a>');
    rows.push('</div>');
    body.innerHTML = rows.join('');
    body.querySelector('.big-icon').appendChild(icon(n, 48));
    for (const slot of body.querySelectorAll('[data-icon-node]')) slot.appendChild(icon(byId.get(+slot.dataset.iconNode), 20));
    body.scrollTop = 0;
  }

  function plural(v, one, few, many) {
    const d = v % 10;
    const t = v % 100;
    if (d === 1 && t !== 11) return one;
    if (d >= 2 && d <= 4 && (t < 12 || t > 14)) return few;
    return many;
  }

  /** Opens node `id`'s details; `opts.onShow(node)` adds a button that shows it on the page's tree. */
  function open(id, opts) {
    const n = byId.get(id);
    if (!n) return;
    if (!root) build();
    options = opts || {};
    current = n;
    if (root.hidden) lastFocus = document.activeElement;
    fill(n);
    root.hidden = false;
    // The next frame, so the slide-in transition starts from the closed position.
    requestAnimationFrame(() => root.classList.add('open'));
    root.querySelector('.x').focus();
  }

  function close() {
    if (!root || root.hidden) return;
    root.classList.remove('open');
    const hide = () => {
      root.hidden = true;
    };
    setTimeout(hide, matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : 260);
    if (lastFocus && lastFocus.focus) lastFocus.focus();
  }

  window.PCTDrawer = { open, close, isOpen: () => !!root && !root.hidden, steps: (id) => stepsFromStart().get(id), percent };
})();
