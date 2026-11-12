'use strict';
// Toasts in the manner of Sonner: a short stack at the bottom right that fades out by itself.
(function () {
  const LIFETIME_MS = 3200;
  const MAX_SHOWN = 3;
  let box = null;
  // Kept on screen until clicked: the page's ?toast=demo, so a screenshot can show them.
  let sticky = false;

  function container() {
    if (box) return box;
    box = document.createElement('ol');
    box.className = 'pct-toasts';
    box.setAttribute('aria-live', 'polite');
    box.setAttribute('aria-label', 'Уведомления');
    document.body.appendChild(box);
    return box;
  }

  function dismiss(item) {
    if (!item.isConnected || item.classList.contains('out')) return;
    item.classList.add('out');
    const gone = () => item.remove();
    item.addEventListener('animationend', gone, { once: true });
    // Without animations (reduced motion) animationend never comes.
    setTimeout(gone, 400);
  }

  /** Shows `text`; `kind` is ok, bad or info and colours the mark; `icon` is an optional element. */
  function show(text, kind, icon) {
    const list = container();
    const item = document.createElement('li');
    item.className = 'pct-toast ' + (kind || 'info');
    const mark = document.createElement('span');
    mark.className = 'mark';
    if (icon) mark.appendChild(icon);
    item.appendChild(mark);
    const words = document.createElement('span');
    words.className = 'words';
    words.textContent = text;
    item.appendChild(words);
    list.appendChild(item);
    while (list.children.length > MAX_SHOWN) list.firstElementChild.remove();
    item.addEventListener('click', () => dismiss(item));
    if (sticky) return item;
    let timer = setTimeout(() => dismiss(item), LIFETIME_MS);
    // Reading a toast holds it, as Sonner does on hover.
    item.addEventListener('pointerenter', () => clearTimeout(timer));
    item.addEventListener('pointerleave', () => {
      timer = setTimeout(() => dismiss(item), LIFETIME_MS / 2);
    });
    return item;
  }

  window.PCTToast = { show, stay: (on) => (sticky = !!on) };
})();
