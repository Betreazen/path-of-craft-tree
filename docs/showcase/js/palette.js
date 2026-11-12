'use strict';
// A command palette in the manner of cmdk: Ctrl+K or "/" opens it, typing searches every page and
// all 995 nodes (name, what they give, branch), arrows move, Enter opens, Esc closes.
(function () {
  const S = window.PCTS;
  const SP = window.PCTSprites;
  const P = window.PCTPages;
  const KIND = { small: 'малый', big: 'большой', upgrade: 'усилитель' };
  const MAX_PAGES = 6;
  const MAX_NODES = 60;
  const norm = (s) => String(s || '').toLowerCase().replace(/ё/g, 'е');
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

  // ---- The index, built once from data.js and pages.js ----
  const pages = P.entries.map((e) => ({ type: 'page', id: e.id, name: e.name, key: norm(e.name), extra: norm(e.sum), icon: e.icon, branch: e.branch }));
  const nodes = S.nodes
    .filter((n) => n.k !== 'start')
    .map((n) => {
      const b = S.branches[n.b];
      const extra = [n.d, ...(n.rd || []), b && b.name, KIND[n.k]].filter(Boolean).join(' ');
      return { type: 'node', node: n, name: n.n, key: norm(n.n), extra: norm(extra) };
    });

  /** How well `item` answers the words of the query; 0 when a word is missing. */
  function score(item, words) {
    let total = 0;
    for (const w of words) {
      const at = item.key.indexOf(w);
      if (at === 0) total += 120;
      else if (at > 0) total += item.key[at - 1] === ' ' ? 100 : 70;
      else if (item.extra.includes(w)) total += 20;
      else {
        const fuzzy = subsequence(item.key, w);
        if (!fuzzy) return 0;
        total += fuzzy;
      }
    }
    return total - item.key.length * 0.1;
  }

  /** cmdk's loose match: the letters of `w` in order inside `s`, worth less the more they spread. */
  function subsequence(s, w) {
    if (w.length < 2) return 0;
    let i = 0;
    let first = -1;
    let last = 0;
    for (const ch of w) {
      i = s.indexOf(ch, i);
      if (i < 0) return 0;
      if (first < 0) first = i;
      last = i++;
    }
    const spread = last - first + 1 - w.length;
    return Math.max(5, 40 - spread * 4);
  }

  /** The matched words of `text` marked; the text is escaped. */
  function mark(text, words) {
    let html = esc(text);
    for (const w of words) {
      if (w.length < 2) continue;
      const re = new RegExp('(' + w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/е/g, '[её]') + ')', 'i');
      html = html.replace(re, '<mark>$1</mark>');
    }
    return html;
  }

  // ---- The dialog ----
  let root = null;
  let input = null;
  let list = null;
  let results = [];
  let active = 0;
  let lastFocus = null;
  let handlers = {};

  function build() {
    root = document.createElement('div');
    root.className = 'pct-palette-root';
    root.hidden = true;
    root.innerHTML =
      '<div class="pct-scrim" data-close></div>' +
      '<div class="pct-palette" role="dialog" aria-modal="true" aria-label="Поиск по справочнику">' +
      '<div class="bar"><span class="glass" aria-hidden="true">⌕</span>' +
      '<input type="text" role="combobox" aria-expanded="true" aria-controls="pct-palette-list" aria-autocomplete="list" ' +
      'placeholder="Узел, эффект, ветка или страница…" spellcheck="false" autocomplete="off">' +
      '<kbd>Esc</kbd></div>' +
      '<div class="list" id="pct-palette-list" role="listbox" aria-label="Результаты"></div>' +
      '<div class="foot"><span><kbd>↑</kbd><kbd>↓</kbd> выбор</span><span><kbd>Enter</kbd> открыть</span><span class="n"></span></div></div>';
    document.body.appendChild(root);
    input = root.querySelector('input');
    list = root.querySelector('.list');
    input.addEventListener('input', () => render(input.value));
    root.addEventListener('click', (ev) => {
      if (ev.target.closest('[data-close]')) close();
      const row = ev.target.closest('[role=option]');
      if (row) choose(+row.dataset.index);
    });
    root.addEventListener('pointermove', (ev) => {
      const row = ev.target.closest('[role=option]');
      if (row && +row.dataset.index !== active) setActive(+row.dataset.index, false);
    });
    root.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') {
        ev.preventDefault();
        close();
      } else if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') {
        ev.preventDefault();
        if (results.length) setActive((active + (ev.key === 'ArrowDown' ? 1 : -1) + results.length) % results.length, true);
      } else if (ev.key === 'Enter') {
        ev.preventDefault();
        choose(active);
      } else if (ev.key === 'Tab') {
        // Only the field takes focus inside the palette; the list follows the arrows.
        ev.preventDefault();
      }
    });
  }

  function render(query) {
    const words = norm(query).split(/\s+/).filter(Boolean);
    let found;
    if (!words.length) {
      found = pages.map((item) => ({ item, s: 1 }));
    } else {
      // Nodes of one name (ten «Урон снарядов») come nearest the start first.
      const near = (item) => (item.type === 'node' ? window.PCTDrawer.steps(item.node.id) ?? 999 : 0);
      const rank = (items, cap) =>
        items
          .map((item) => ({ item, s: score(item, words) }))
          .filter((r) => r.s > 0)
          .sort((a, b) => b.s - a.s || a.item.name.localeCompare(b.item.name, 'ru') || near(a.item) - near(b.item))
          .slice(0, cap);
      found = [...rank(pages, MAX_PAGES), ...rank(nodes, MAX_NODES)];
    }
    // Groups as cmdk shows them: pages first, then nodes by branch, the best branch first.
    const groups = [];
    const pageRows = found.filter((r) => r.item.type === 'page');
    if (pageRows.length) groups.push(['Страницы', pageRows]);
    const byBranch = new Map();
    for (const r of found.filter((x) => x.item.type === 'node')) {
      const b = r.item.node.b;
      if (!byBranch.has(b)) byBranch.set(b, []);
      byBranch.get(b).push(r);
    }
    for (const [b, rows] of byBranch) groups.push(['Узлы · ' + S.branches[b].name, rows, S.branches[b].colour]);
    results = groups.flatMap(([, rows]) => rows.map((r) => r.item));
    list.innerHTML = '';
    let index = 0;
    for (const [title, rows, colour] of groups) {
      const g = document.createElement('div');
      g.className = 'group';
      g.setAttribute('role', 'presentation');
      g.innerHTML = '<div class="g-title">' + (colour ? '<i style="background:' + colour + '"></i>' : '') + esc(title) + '</div>';
      for (const { item } of rows) g.appendChild(row(item, index++, words));
      list.appendChild(g);
    }
    if (!results.length) list.innerHTML = '<div class="empty">Ничего не нашлось. Попробуйте часть слова: «жил», «урон», «рыб».</div>';
    root.querySelector('.foot .n').textContent = words.length ? 'найдено: ' + results.length : 'начните вводить';
    setActive(0, true);
  }

  function row(item, index, words) {
    const r = document.createElement('div');
    r.className = 'opt';
    r.id = 'pct-opt-' + index;
    r.setAttribute('role', 'option');
    r.dataset.index = index;
    const ic = document.createElement('span');
    ic.className = 'ic';
    const key = item.type === 'page' ? item.icon : 'icon:' + item.node.i + ':n';
    ic.appendChild(SP.canvas(key, item.type === 'page' && item.branch ? 36 : 16));
    r.appendChild(ic);
    const name = document.createElement('span');
    name.className = 'nm';
    name.innerHTML = mark(item.name, words);
    r.appendChild(name);
    const meta = document.createElement('span');
    meta.className = 'meta';
    if (item.type === 'node') {
      const n = item.node;
      const b = S.branches[n.b];
      // What tells apart nodes of one name: the small node's bonus and how far it sits.
      const gives = n.k === 'small' && (n.e || []).length ? '<span class="gives">' + window.PCTDrawer.percent(n.e[0]) + '</span>' : '';
      const step = window.PCTDrawer.steps(n.id);
      meta.innerHTML =
        gives +
        (step != null ? '<span class="step">шаг ' + step + '</span>' : '') +
        '<span class="br"><span class="dot" style="background:' + b.colour + '"></span>' + esc(b.name) + ' · ' + KIND[n.k] + '</span>';
    } else {
      meta.textContent = 'страница';
    }
    r.appendChild(meta);
    return r;
  }

  function setActive(index, scroll) {
    active = index;
    for (const o of list.querySelectorAll('[role=option]')) o.setAttribute('aria-selected', String(+o.dataset.index === index));
    const cur = list.querySelector('#pct-opt-' + index);
    input.setAttribute('aria-activedescendant', cur ? cur.id : '');
    if (cur && scroll) cur.scrollIntoView({ block: 'nearest' });
  }

  function choose(index) {
    const item = results[index];
    if (!item) return;
    close();
    if (item.type === 'page') handlers.onPage(item.id);
    else handlers.onNode(item.node);
  }

  /** Opens the palette with `query` typed in; `opts.onPage(id)` and `opts.onNode(node)` act on Enter. */
  function open(query, opts) {
    if (!root) build();
    if (opts) handlers = opts;
    if (root.hidden) lastFocus = document.activeElement;
    root.hidden = false;
    requestAnimationFrame(() => root.classList.add('open'));
    input.value = query || '';
    render(input.value);
    input.focus();
    input.select();
  }

  function close() {
    if (!root || root.hidden) return;
    root.classList.remove('open');
    root.hidden = true;
    if (lastFocus && lastFocus.focus) lastFocus.focus();
  }

  /** Ctrl+K (⌘K) anywhere and "/" outside text fields open the palette. */
  function bindKeys(opts) {
    handlers = opts;
    document.addEventListener('keydown', (ev) => {
      const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement && document.activeElement.tagName);
      // The physical keys, so the Russian layout (where K types «л») opens it too.
      if ((ev.ctrlKey || ev.metaKey) && (ev.code === 'KeyK' || ev.key.toLowerCase() === 'k')) {
        ev.preventDefault();
        if (root && !root.hidden) close();
        else open('', opts);
      } else if ((ev.key === '/' || (ev.code === 'Slash' && !ev.shiftKey)) && !typing && !(root && !root.hidden)) {
        ev.preventDefault();
        open('', opts);
      }
    });
  }

  window.PCTPalette = { open, close, bindKeys };
})();
