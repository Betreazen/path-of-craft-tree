// Pixel helpers for the tree preview: colours, small pixel buffers, bevelled frames and crisp text.
// Everything is drawn into a low-resolution canvas that the page scales up without smoothing.
'use strict';
const PCT = (window.PCT = {});

/** '#rrggbb' to the little-endian RGBA word an ImageData's Uint32 view expects. */
PCT.rgb = (hex, alpha = 255) => {
  const n = parseInt(hex.slice(1, 7), 16);
  return ((alpha << 24) | ((n & 255) << 16) | (n & 0xff00) | (n >> 16)) >>> 0;
};

/** A colour word with its RGB multiplied by `k`. */
PCT.shade = (c, k) => {
  const ch = (v) => Math.max(0, Math.min(255, Math.round(v * k)));
  return (((c >>> 24) << 24) | (ch((c >>> 16) & 255) << 16) | (ch((c >>> 8) & 255) << 8) | ch(c & 255)) >>> 0;
};

/** A colour word with its RGB pulled towards grey by `amount` and darkened by `k`. */
PCT.mute = (c, amount, k) => {
  const [r, g, b] = [c & 255, (c >>> 8) & 255, (c >>> 16) & 255];
  const lum = 0.3 * r + 0.59 * g + 0.11 * b;
  const ch = (v) => Math.max(0, Math.min(255, Math.round((v * (1 - amount) + lum * amount) * k)));
  return (((c >>> 24) << 24) | (ch(b) << 16) | (ch(g) << 8) | ch(r)) >>> 0;
};

/** `a` moved towards `b` by `t`. */
PCT.mix = (a, b, t) => {
  const ch = (s) => Math.round(((a >>> s) & 255) * (1 - t) + ((b >>> s) & 255) * t);
  return ((255 << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)) >>> 0;
};

PCT.css = (c) => `rgba(${c & 255},${(c >>> 8) & 255},${(c >>> 16) & 255},${((c >>> 24) & 255) / 255})`;

class Pix {
  constructor(w, h) {
    this.w = w;
    this.h = h;
    this.d = new Uint32Array(w * h);
  }
  set(x, y, c) {
    if (c && x >= 0 && y >= 0 && x < this.w && y < this.h) this.d[y * this.w + x] = c;
  }
  get(x, y) {
    return x >= 0 && y >= 0 && x < this.w && y < this.h ? this.d[y * this.w + x] : 0;
  }
  rect(x, y, w, h, c) {
    for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) this.set(i, j, c);
  }
  /** Copies `src` onto this buffer at (x, y), skipping transparent pixels. */
  blit(src, x, y) {
    for (let j = 0; j < src.h; j++) for (let i = 0; i < src.w; i++) this.set(x + i, y + j, src.d[j * src.w + i]);
  }
  /** A canvas of this buffer, each pixel `scale` times bigger. */
  canvas(scale = 1) {
    const c = document.createElement('canvas');
    c.width = this.w * scale;
    c.height = this.h * scale;
    const ctx = c.getContext('2d');
    const img = ctx.createImageData(this.w, this.h);
    new Uint32Array(img.data.buffer).set(this.d);
    if (scale === 1) {
      ctx.putImageData(img, 0, 0);
      return c;
    }
    const raw = document.createElement('canvas');
    raw.width = this.w;
    raw.height = this.h;
    raw.getContext('2d').putImageData(img, 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(raw, 0, 0, c.width, c.height);
    return c;
  }
}
PCT.Pix = Pix;

/** Distance from the edge of a w×h rectangle whose corners are cut diagonally by `cut` pixels. */
PCT.rectDistance = (w, h, cut) => (x, y) => {
  const ex = Math.min(x, w - 1 - x);
  const ey = Math.min(y, h - 1 - y);
  return Math.min(ex, ey, ex + ey - cut);
};

/**
 * Paints a bevelled frame. `distance(x, y)` says how deep a pixel is inside the shape (negative is
 * outside). Ring k of `rings` is painted k pixels deep; a ring is a colour or {l, d, m}: the light,
 * dark and middle colour for the top-left half, the bottom-right half and the diagonal between.
 */
PCT.frame = (pix, ox, oy, w, h, distance, rings, fill) => {
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const d = distance(x, y);
      if (d < 0) continue;
      const ring = d < rings.length ? rings[d] : fill;
      if (typeof ring === 'number') {
        pix.set(ox + x, oy + y, ring);
        continue;
      }
      const side = x / (w - 1) + y / (h - 1);
      pix.set(ox + x, oy + y, side < 0.98 ? ring.l : side > 1.02 ? ring.d : ring.m ?? ring.l);
    }
  }
};

// 3×5 digits and a slash for the rank badges.
const DIGITS = {
  0: '111101101101111', 1: '010110010010111', 2: '111001111100111', 3: '111001011001111',
  4: '101101111001001', 5: '111100111001111', 6: '111100111101111', 7: '111001010010010',
  8: '111101111101111', 9: '111101111001111', '/': '001001010100100',
};
PCT.tinyWidth = (str) => str.length * 4 - 1;
PCT.tinyText = (pix, str, x, y, c) => {
  [...str].forEach((ch, n) => {
    const bits = DIGITS[ch];
    if (!bits) return;
    for (let i = 0; i < 15; i++) if (bits[i] === '1') pix.set(x + n * 4 + (i % 3), y + Math.floor(i / 3), c);
  });
};

// ---- Crisp text: a pixel font rendered once per string, its edges snapped to whole pixels. ----
const FONT_PX = 12;
const FONT = `${FONT_PX}px "Tiny5", "Lucida Console", monospace`;
const measureCtx = document.createElement('canvas').getContext('2d');
const textCache = new Map();
let metrics = null;

const fontMetrics = () => {
  if (metrics) return metrics;
  measureCtx.font = FONT;
  const m = measureCtx.measureText('ЙДЁgjpqy');
  const ascent = Math.ceil(m.fontBoundingBoxAscent ?? FONT_PX * 0.8);
  const descent = Math.ceil(m.fontBoundingBoxDescent ?? FONT_PX * 0.25);
  metrics = { ascent, height: ascent + descent };
  return metrics;
};

PCT.fontChanged = () => {
  textCache.clear();
  metrics = null;
};
PCT.lineHeight = () => fontMetrics().height - 3;
PCT.measure = (str) => {
  measureCtx.font = FONT;
  return Math.ceil(measureCtx.measureText(str).width) + 2;
};

/** A canvas holding `str` in `colour` with a vanilla-style drop shadow, or a full dark outline. */
PCT.text = (str, colour, outline = false) => {
  const key = `${colour}|${outline ? 1 : 0}|${str}`;
  const hit = textCache.get(key);
  if (hit) return hit;
  if (textCache.size > 800) textCache.clear();
  const { ascent, height } = fontMetrics();
  const w = PCT.measure(str) + 2;
  const raw = document.createElement('canvas');
  raw.width = w;
  raw.height = height + 2;
  const rctx = raw.getContext('2d');
  rctx.font = FONT;
  rctx.fillStyle = '#fff';
  rctx.fillText(str, 1, ascent);
  const alpha = rctx.getImageData(0, 0, w, height + 2).data;
  const glyph = new Pix(w, height + 2);
  const ink = PCT.rgb(colour);
  const shadow = outline ? PCT.rgb('#0b0b0e') : PCT.shade(ink, 0.25);
  const lit = (x, y) => x >= 0 && y >= 0 && x < w && y < height + 2 && alpha[(y * w + x) * 4 + 3] > 110;
  const around = outline ? [[-1, 0], [1, 0], [0, -1], [0, 1], [1, 1], [-1, -1], [1, -1], [-1, 1]] : [[1, 1]];
  for (let y = 0; y < height + 2; y++) {
    for (let x = 0; x < w; x++) {
      if (lit(x, y)) glyph.set(x, y, ink);
      else if (around.some(([dx, dy]) => lit(x - dx, y - dy))) glyph.set(x, y, shadow);
    }
  }
  const canvas = glyph.canvas();
  textCache.set(key, canvas);
  return canvas;
};

/** Splits `str` into lines no wider than `max` pixels. */
PCT.wrap = (str, max) => {
  const lines = [];
  let line = '';
  for (const word of str.split(/\s+/)) {
    const next = line ? `${line} ${word}` : word;
    if (line && PCT.measure(next) > max) {
      lines.push(line);
      line = word;
    } else line = next;
  }
  if (line) lines.push(line);
  return lines;
};

PCT.FONT = FONT;
