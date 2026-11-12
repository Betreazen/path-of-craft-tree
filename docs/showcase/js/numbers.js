'use strict';
// Numbers that roll to their new value digit by digit, in the manner of NumberFlow. Without motion
// (prefers-reduced-motion) the text simply changes.
(function () {
  const still = () => window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  /** A column 0..9 for one digit, moved so that `digit` shows. */
  function column(digit) {
    const col = document.createElement('span');
    col.className = 'roll-col';
    const strip = document.createElement('span');
    strip.className = 'roll-strip';
    strip.textContent = '0123456789'.split('').join('\n');
    strip.style.transform = 'translateY(' + -digit + 'em)';
    col.appendChild(strip);
    return col;
  }

  /** Puts `text` (digits and anything else) into `el`, rolling the digits that changed. */
  function set(el, text) {
    text = String(text);
    const old = el.dataset.rollValue;
    el.dataset.rollValue = text;
    el.setAttribute('aria-label', text);
    if (still() || old == null || old.length !== text.length) {
      el.classList.add('roll');
      el.textContent = '';
      for (const ch of text) {
        if (/\d/.test(ch)) el.appendChild(column(+ch));
        else el.appendChild(document.createTextNode(ch));
      }
      for (const c of el.children) c.setAttribute('aria-hidden', 'true');
      return;
    }
    const parts = [...el.childNodes];
    [...text].forEach((ch, i) => {
      const part = parts[i];
      if (/\d/.test(ch) && part && part.classList && part.classList.contains('roll-col')) {
        part.firstChild.style.transform = 'translateY(' + -ch + 'em)';
      } else if (part) {
        part.textContent = ch;
      }
    });
  }

  window.PCTNumber = { set };
})();
