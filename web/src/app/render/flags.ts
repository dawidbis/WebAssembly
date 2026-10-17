import { NATION_COLORS } from './nations';

type Rgb = readonly [number, number, number];

/**
 * Flagi państw (na razie stała paleta – flaga `n` należy do państwa `n`, kolor dominujący to kolor
 * państwa z `NATION_COLORS`). Flaga to wzór liczony z (u, v), więc tę samą flagę maluje worker
 * malujący (bez DOM, nakładka na teren) i rysuje UI (miniaturki – `flagDataUrl`). Proporcje 3:2.
 */
type Pattern =
  | 'h2' // dwa pasy poziome
  | 'h3' // trzy pasy poziome
  | 'v3' // trzy pasy pionowe
  | 'nordic' // krzyż skandynawski
  | 'saltire' // krzyż ukośny
  | 'disc' // koło na środku
  | 'canton' // kwadrat w górnym rogu przy drzewcu
  | 'chevron' // trójkąt przy drzewcu
  | 'cross' // krzyż grecki na środku
  | 'diagonal'; // podział po przekątnej

interface FlagDef {
  pattern: Pattern;
  /** Kolory: [0] pole (zwykle kolor państwa), dalej elementy wzoru. */
  colors: readonly Rgb[];
}

const WHITE: Rgb = [240, 238, 230];
const GOLD: Rgb = [242, 196, 60];
const DARK: Rgb = [36, 34, 38];
const c = (n: number): Rgb => NATION_COLORS[n - 1];

/** Kolejność jak `NATION_COLORS` (12 państw, potem od początku). */
export const FLAGS: readonly FlagDef[] = [
  { pattern: 'h2', colors: [WHITE, c(1)] },
  { pattern: 'nordic', colors: [c(2), GOLD] },
  { pattern: 'v3', colors: [c(3), WHITE, c(9)] },
  { pattern: 'h3', colors: [c(4), DARK, c(4)] },
  { pattern: 'saltire', colors: [c(5), WHITE] },
  { pattern: 'disc', colors: [c(6), WHITE] },
  { pattern: 'canton', colors: [c(7), WHITE] },
  { pattern: 'h3', colors: [c(8), WHITE, c(8)] },
  { pattern: 'chevron', colors: [c(9), WHITE] },
  { pattern: 'cross', colors: [c(10), WHITE] },
  { pattern: 'diagonal', colors: [c(11), [40, 96, 44]] },
  { pattern: 'v3', colors: [c(12), WHITE, c(12)] },
];

export function flagOf(nation: number): FlagDef {
  return FLAGS[(nation - 1) % FLAGS.length];
}

/** Pas (0, 1, 2) w podziale na trzy równe części. */
const third = (t: number): number => Math.min(2, Math.floor(t * 3));

/**
 * Element wzoru w punkcie: (u, v) ∈ [0, 1)² i te same współrzędne na fladze 3 × 2 (x, y); wynik to numer
 * koloru – 0 pole, 1 i 2 elementy wzoru.
 */
const PATTERNS: Record<Pattern, (u: number, v: number, x: number, y: number) => number> = {
  h2: (_u, v) => (v < 0.5 ? 0 : 1),
  h3: (_u, v) => third(v),
  v3: (u) => third(u),
  nordic: (_u, _v, x, y) => Number(Math.abs(y - 1) < 0.2 || Math.abs(x - 1.1) < 0.2),
  saltire: (_u, _v, x, y) => Number(Math.abs(2 * x - 3 * y) < 0.75 || Math.abs(2 * x + 3 * y - 6) < 0.75),
  disc: (_u, _v, x, y) => Number((x - 1.5) ** 2 + (y - 1) ** 2 < 0.36),
  canton: (_u, _v, x, y) => Number(x < 1.2 && y < 1),
  chevron: (_u, _v, x, y) => Number(x < 1.3 * (1 - Math.abs(y - 1))),
  cross: (_u, _v, x, y) => {
    const [dx, dy] = [Math.abs(x - 1.5), Math.abs(y - 1)];
    return Number((dx < 0.2 && dy < 0.7) || (dy < 0.2 && dx < 0.7));
  },
  diagonal: (_u, _v, x, y) => (x / 3 + y / 2 < 1 ? 0 : 1),
};

/** Kolor flagi w punkcie (u, v) ∈ [0, 1)² – u od drzewca w prawo, v z góry w dół. */
export function flagPixel(flag: FlagDef, u: number, v: number): Rgb {
  const [field, a = WHITE, b = a] = flag.colors;
  return [field, a, b][PATTERNS[flag.pattern](u, v, u * 3, v * 2)];
}

const urlCache = new Map<string, string>();

/** Miniaturka flagi jako obraz (data URL) – tylko w wątku głównym (UI). */
export function flagDataUrl(nation: number, width = 30, height = 20): string {
  const key = `${nation}|${width}|${height}`;
  const cached = urlCache.get(key);
  if (cached) return cached;
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  if (!ctx) return '';
  const img = ctx.createImageData(width, height);
  const flag = flagOf(nation);
  // 2 × 2 próbki na piksel – gładsze skosy i koła w małej miniaturce.
  for (let py = 0; py < height; py++) {
    for (let px = 0; px < width; px++) {
      let r = 0;
      let g = 0;
      let bl = 0;
      for (const [sx, sy] of [[0.25, 0.25], [0.75, 0.25], [0.25, 0.75], [0.75, 0.75]]) {
        const col = flagPixel(flag, (px + sx) / width, (py + sy) / height);
        r += col[0];
        g += col[1];
        bl += col[2];
      }
      const o = (py * width + px) * 4;
      img.data[o] = r / 4;
      img.data[o + 1] = g / 4;
      img.data[o + 2] = bl / 4;
      img.data[o + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
  const url = canvas.toDataURL();
  urlCache.set(key, url);
  return url;
}
