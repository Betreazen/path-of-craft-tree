'use strict';
// Page shell of the guide, laid out like RustCraft's: sidebar, search, entry pages.
(function () {
  const S = window.PCTS;
  const SP = window.PCTSprites;
  const P = window.PCTPages;
  const $ = (sel, el) => (el || document).querySelector(sel);
  const el = (tag, cls, html) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html != null) e.innerHTML = html;
    return e;
  };
  const byId = Object.fromEntries(P.entries.map((e) => [e.id, e]));
  const nodeById = new Map(S.nodes.map((n) => [n.id, n]));
  const num = (v) => String(v).replace('.', ',');
  const SECTOR_NAME = Object.fromEntries(S.sectors.map(([key, title]) => [key, title]));

  // Related work paid to a second branch at the cross share, as the mod's code does it.
  const CROSS = {
    mining: [['smithing', 'руда, упавшая слитком (Переплавка)']],
    woodcutting: [['crafting', 'брёвна, упавшие досками (Плотник)']],
    farming: [['alchemy', 'сбор незерского нароста']],
    smithing: [['crafting', 'крафт инструмента или брони']],
    cooking: [['crafting', 'крафт еды']],
    melee: [['husbandry', 'убийство животного']],
    enchanting: [['smithing', 'книга на наковальне']],
  };

  window.onerror = (msg, src, line) => {
    $('#err').textContent = 'Ошибка страницы: ' + msg + ' (' + String(src).split('/').pop() + ':' + line + ')';
  };

  function icon(key, box) {
    const wrap = el('span', 'icon');
    wrap.appendChild(SP.canvas(key, box || 16));
    return wrap;
  }
  const nodeIcon = (n) => icon('icon:' + n.i + ':n', 16);
  const branchLink = (id) => '<a href="#' + id + '">' + S.branches[id].name + '</a>';

  // ---------------------------------------------------------------- sidebar
  function buildNav() {
    const nav = $('#nav');
    for (const [cat, title] of P.cats) {
      nav.appendChild(el('h3', null, title));
      const ul = el('ul');
      for (const e of P.entries.filter((x) => x.cat === cat)) {
        const li = el('li');
        const a = el('a');
        a.href = '#' + e.id;
        a.dataset.id = e.id;
        a.dataset.find = (e.name + ' ' + e.sum).toLowerCase();
        a.appendChild(icon(e.icon, e.branch ? 36 : 16));
        a.appendChild(el('span', 'nm', e.name));
        if (e.branch) {
          const b = S.branches[e.branch];
          const total = b.kinds.small + b.kinds.big + b.kinds.upgrade;
          const badge = el('span', 'tier', String(total));
          badge.style.background = b.colour;
          badge.title = 'Узлов в ветви: ' + total;
          a.appendChild(badge);
        }
        li.appendChild(a);
        ul.appendChild(li);
      }
      nav.appendChild(ul);
    }
    $('#search').addEventListener('input', (ev) => {
      const q = ev.target.value.trim().toLowerCase();
      for (const a of nav.querySelectorAll('a')) a.parentNode.hidden = q && !a.dataset.find.includes(q);
      for (const h of nav.querySelectorAll('h3')) h.hidden = ![...h.nextElementSibling.children].some((li) => !li.hidden);
    });
  }

  // ---------------------------------------------------------------- the tree stage
  let view = null;
  function legendRow() {
    const row = el('div', 'legend-row');
    for (const [state, label] of [['available', 'можно изучить'], ['learned', 'изучено'], ['locked', 'закрыто'], ['off', 'выключено ПКМ']]) {
      const s = el('span');
      s.appendChild(SP.canvas('cframe:small:' + state, 12));
      s.appendChild(document.createTextNode(label));
      row.appendChild(s);
    }
    row.appendChild(el('span', 'hint', 'Наведите на узел — подсказка, как в игре. Клик изучает, правая кнопка выключает большой узел. Дерево двигается мышью или пальцем, приближают Ctrl и колесо или кнопки ниже.'));
    return row;
  }

  function treeStage(main, focus) {
    const sc = el('section', 'scene');
    sc.innerHTML = '<div class="stage"><div class="view"></div></div>';
    main.appendChild(sc);
    sc.appendChild(legendRow());
    if (!focus) {
      sc.appendChild(el('div', 'onstage', 'Ветви: ' + S.sectors.flatMap(([, , ids]) => ids).map(branchLink).join(' · ')));
    }
    const controls = el('div', 'controls');
    const button = (label, title, fn, cls) => {
      const b = el('button', cls || null, label);
      b.type = 'button';
      b.title = title;
      b.setAttribute('aria-label', title);
      b.addEventListener('click', fn);
      controls.appendChild(b);
    };
    const count = el('span', 'count');
    const update = (v) => {
      count.textContent = 'изучено рангов: ' + v.learnedCount();
    };
    button('−', 'Отдалить', () => view.zoomBy(1 / 1.25));
    button('+', 'Приблизить', () => view.zoomBy(1.25));
    button('Вид по умолчанию', 'Вид по умолчанию', () => view.home(), 'reset');
    button('Сбросить изученное', 'Сбросить изученное', () => view.resetLearned(), 'reset');
    controls.appendChild(count);
    sc.appendChild(controls);
    view = new PCTTree.TreeView(sc, { focus, onChange: update });
    // Kept reachable for checks from the browser console.
    window.PCTView = view;
    update(view);
  }

  function shotStage(main, shot) {
    const sc = el('section', 'scene');
    sc.innerHTML = '<div class="stage shot"><img src="' + shot.src + '" alt="' + shot.alt + '"></div>';
    sc.appendChild(el('div', 'caption', 'Так дерево выглядит в игре. <a href="#tree">Покрутить его здесь →</a>'));
    main.appendChild(sc);
  }

  // ---------------------------------------------------------------- page parts
  const md = (s) => s.replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>');

  function statsAside(rows) {
    const side = el('aside');
    side.appendChild(el('h2', null, 'Характеристики'));
    side.appendChild(el('table', 'stats', rows.map(([k, v]) => '<tr><td>' + k + '</td><td>' + v + '</td></tr>').join('')));
    return side;
  }

  function tableEl(t) {
    const wrap = el('div', 'wide');
    wrap.appendChild(
      el(
        'table',
        'pct-table' + (t.numbers ? ' num' : ''),
        '<tr>' + t.head.map((h) => '<th>' + h + '</th>').join('') + '</tr>' + t.rows.map((r) => '<tr>' + r.map((c) => '<td>' + c + '</td>').join('') + '</tr>').join(''),
      ),
    );
    return wrap;
  }

  function branchText(id) {
    const b = S.branches[id];
    const text = b.prose.map(md);
    text.push('Каждый уровень ветви даёт +' + num(b.bonus.perLevel) + '% к свойству «' + b.bonus.name + '».');
    if (b.sums.length) {
      text.push('Малые узлы ветви, изученные все, дают в сумме: ' + b.sums.map(([name, v]) => name.toLowerCase() + ' +' + num(v) + '%').join(', ') + '.');
    }
    return text;
  }

  function branchStats(id) {
    const b = S.branches[id];
    const rows = [
      ['Сектор', SECTOR_NAME[b.sector]],
      ['Узлов', b.kinds.small + b.kinds.big + b.kinds.upgrade],
      ['Больших', b.kinds.big],
      ['Усилителей', b.kinds.upgrade],
      ['Малых', b.kinds.small],
      ['С двумя рангами', b.ranked],
    ];
    if (b.tool) rows.push(['Инструмент', b.tool]);
    rows.push(['За уровень', '+' + num(b.bonus.perLevel) + '% ' + b.bonus.name.toLowerCase()]);
    for (const [other, what] of CROSS[id] || []) rows.push(['Смежный опыт', branchLink(other) + ' ×' + num(S.consts.crossShare) + ': ' + what]);
    for (const [other, list] of Object.entries(CROSS)) {
      for (const [target, what] of list) if (target === id) rows.push(['Получает от', branchLink(other) + ': ' + what]);
    }
    return rows;
  }

  function xpTable(id) {
    const rows = S.branches[id].xp.map(([label, value, kind]) => [label, kind === 'x' ? '×' + num(value) : num(value)]);
    return tableEl({ head: ['За что', 'Опыт'], rows, numbers: true });
  }

  function bigNodes(id) {
    const list = el('div', 'bigs');
    for (const { id: bigId, ups } of S.branches[id].bigs) {
      const big = nodeById.get(bigId);
      const card = el('div', 'big');
      const head = el('div', 'big-head');
      head.appendChild(nodeIcon(big));
      head.appendChild(el('b', null, big.n));
      card.appendChild(head);
      if (big.d) card.appendChild(el('p', null, big.d));
      if (ups.length) {
        const ul = el('ul');
        for (const upId of ups) {
          const u = nodeById.get(upId);
          const what = u.r > 1 && u.rd && u.rd.length ? u.rd.join(' → ') : u.d || '';
          let label = u.n.includes(': ') ? u.n.slice(u.n.indexOf(': ') + 2) : u.n;
          if (label === big.n || /^\d+$/.test(label)) label = 'Усилитель';
          if (u.r > 1) label += ', ' + u.r + ' ранга';
          ul.appendChild(el('li', null, '<span class="up">' + label + '</span>' + (what ? ' — ' + what : '')));
        }
        card.appendChild(ul);
      }
      list.appendChild(card);
    }
    return list;
  }

  function seeAlso(main, ids) {
    const list = (ids || []).filter((s) => byId[s]);
    if (!list.length) return;
    main.appendChild(el('h2', null, 'См. также'));
    const see = el('div', 'see');
    for (const sid of list) {
      const a = el('a');
      a.href = '#' + sid;
      a.appendChild(icon(byId[sid].icon, byId[sid].branch ? 36 : 16));
      a.appendChild(el('span', null, byId[sid].name));
      see.appendChild(a);
    }
    main.appendChild(see);
  }

  // ---------------------------------------------------------------- entry page
  function show(address) {
    const e = byId[address] || byId.overview;
    if (view) view.stop();
    view = null;
    const main = $('#main');
    main.innerHTML = '';
    window.scrollTo(0, 0);
    document.title = e.name + ' · Path-of-craft-tree';
    for (const a of document.querySelectorAll('#nav a')) {
      a.classList.toggle('on', a.dataset.id === e.id);
      if (a.dataset.id === e.id) a.scrollIntoView({ block: 'nearest' });
    }

    const head = el('header', 'head');
    head.appendChild(icon(e.icon, e.branch ? 36 : 16));
    const ht = el('div');
    ht.appendChild(el('h1', null, e.name));
    if (e.branch) {
      const b = S.branches[e.branch];
      const tag = el('span', 'tier wide', 'Сектор «' + SECTOR_NAME[b.sector] + '» · ' + (b.kinds.small + b.kinds.big + b.kinds.upgrade) + ' узлов');
      tag.style.background = b.colour;
      ht.appendChild(tag);
    }
    head.appendChild(ht);
    main.appendChild(head);
    main.appendChild(el('p', 'sum', e.sum));

    if (e.stage === 'tree') treeStage(main, null);
    if (e.stage === 'branch') treeStage(main, e.branch);
    if (e.shot) shotStage(main, e.shot);

    const cols = el('div', 'cols');
    const text = el('div', 'text');
    for (const p of e.branch ? branchText(e.branch) : e.text || []) text.appendChild(el('p', null, p));
    cols.appendChild(text);
    const stats = e.branch ? branchStats(e.branch) : e.stats;
    if (stats) cols.appendChild(statsAside(stats));
    main.appendChild(cols);

    if (e.table) {
      main.appendChild(el('h2', null, e.table.title));
      main.appendChild(tableEl(e.table));
    }
    if (e.branch) {
      main.appendChild(el('h2', null, 'За что даётся опыт'));
      main.appendChild(xpTable(e.branch));
      main.appendChild(el('h2', null, 'Большие узлы и их усилители'));
      main.appendChild(bigNodes(e.branch));
    }
    seeAlso(main, e.see);
  }

  SP.ready
    .then(() => {
      buildNav();
      const route = () => show(decodeURIComponent(location.hash.slice(1)) || 'overview');
      window.addEventListener('hashchange', route);
      route();
    })
    .catch((e) => setTimeout(() => { throw e; }));
})();
