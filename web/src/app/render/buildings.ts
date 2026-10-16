import { CanvasSource, Container, Graphics, Sprite, Texture } from 'pixi.js';

/**
 * Budynki z miejscem na mapie (kolejność `slots::SLOT_BUILDINGS` w rdzeniu) i ich numery w tablicy
 * poziomów (`Building as usize`).
 */
export const SLOT_BUILDINGS = [
  { building: 'city', index: 2 },
  { building: 'port', index: 5 },
  { building: 'extraction', index: 0 },
  { building: 'factory', index: 1 },
  { building: 'university', index: 3 },
] as const;
export type SlotBuilding = (typeof SLOT_BUILDINGS)[number]['building'];
/** Liczba rodzajów budynków w tablicy poziomów (`buildings::BUILDING_COUNT`) i numer centrum dowodzenia. */
const BUILDING_COUNT = 8;
const COMMAND_CENTER = 7;
const NO_SLOT = 0xffff;
/** Liczb w tablicy miejsc na prowincję (x, y każdego budynku z miejscem). */
const STRIDE = SLOT_BUILDINGS.length * 2;

/** Ikony budynków (24 × 24, linia) – te same co w oknie budynków. */
export const BUILDING_ICONS: Record<SlotBuilding, string> = {
  extraction: 'M4 20h16M7 20V10l5-5 5 5v10M10 20v-5h4v5',
  factory: 'M3 20V10l5 3V10l5 3V6h3v14zM17 6h3v14',
  city: 'M3 20V9h5v11M8 20V4h7v16M15 20v-8h6v8M10 7h3M10 10h3M10 13h3',
  university: 'M2 9l10-5 10 5-10 5zM6 11v5c3 2 9 2 12 0v-5M22 9v6',
  port: 'M12 3v15M8 7h8M5 13c0 4 3 6 7 6s7-2 7-6M3 14l2-2 2 2M17 14l2-2 2 2',
};

/** Tło ikony – inny kolor dla każdego rodzaju budynku (rozróżnienie na mapie). */
export const BUILDING_COLORS: Record<SlotBuilding, string> = {
  city: '#7b5ea8',
  port: '#2f74a8',
  extraction: '#8c6430',
  factory: '#a8483a',
  university: '#2e8a6e',
};

/**
 * Budowy: ikona i kolor każdego rodzaju w kolejności `Building as usize` (wydobycie, fabryka, miasto,
 * uniwersytet, umocnienia, port, infrastruktura, centrum dowodzenia). Budynki bez miejsca na mapie
 * (umocnienia, infrastruktura, centrum dowodzenia) – budowa obok miasta.
 */
const SITES: readonly { path: string; color: string; slot: SlotBuilding | null }[] = [
  { path: BUILDING_ICONS.extraction, color: BUILDING_COLORS.extraction, slot: 'extraction' },
  { path: BUILDING_ICONS.factory, color: BUILDING_COLORS.factory, slot: 'factory' },
  { path: BUILDING_ICONS.city, color: BUILDING_COLORS.city, slot: 'city' },
  { path: BUILDING_ICONS.university, color: BUILDING_COLORS.university, slot: 'university' },
  { path: 'M4 20V8h3v3h3V8h4v3h3V8h3v12z', color: '#6b6f78', slot: null },
  { path: BUILDING_ICONS.port, color: BUILDING_COLORS.port, slot: 'port' },
  { path: 'M7 20 10 4M17 20 14 4M12 6v2M12 11v2M12 16v2', color: '#5d6b4a', slot: null },
  { path: 'M5 21V4M5 4h12l-3 4 3 4H5', color: '#8a7330', slot: null },
];
/** Pierścień postępu budowy: kolor, tło i grubość (kafle). */
const RING_COLOR = 0xf2d47a;
const RING_BACK = 0x0c1824;
const RING_WIDTH = 0.7;

/** Rozmiar ikony na mapie (kafle – stały, skaluje się z mapą) i tekstury (px). */
const ICON_TILES = 6;
const TEXTURE_PX = 64;
/** Gwiazdki centrum dowodzenia względem ikony (jedna gwiazdka ≈ 0,55 × 0,9 ikony). */
const STAR_SCALE = 0.55;
/** Ikony widać od takiego przybliżenia (px ekranu na kafel) – z daleka byłyby nieczytelne. */
const MIN_SCALE = 2;
/** Kolory gwiazdek centrum dowodzenia: 1–2 brąz, 3–4 srebro, 5–6 złoto. */
const STAR_COLORS = ['#c47a3a', '#c47a3a', '#c9d1d9', '#c9d1d9', '#f2c94c', '#f2c94c'];

interface Icon {
  sprite: Sprite;
  x: number;
  y: number;
  province: number;
  building: SlotBuilding;
}

/**
 * Budynki na mapie: okrągłe ikony z poziomem w stałych miejscach prowincji (tylko zbudowane), nad
 * miastem stolicy gwiazdki centrum dowodzenia. Wielkość stała na mapie (`ICON_TILES` kafli – rośnie
 * i maleje z przybliżeniem); z daleka ukryte. `hit` – który budynek jest pod kursorem (kliknięcie otwiera okienko budynku).
 */
export class BuildingLayer {
  readonly view = new Container();
  /** Zbudowane (ikony i gwiazdki) i budowy (ikona z pierścieniem postępu). */
  private readonly built = new Container();
  private readonly sites = new Container();
  private readonly rings = new Graphics();
  private icons: Icon[] = [];
  private siteIcons: Icon[] = [];
  private stars: Sprite[] = [];
  private readonly textures = new Map<string, Texture>();

  constructor() {
    this.sites.addChild(this.rings);
    this.view.addChild(this.built, this.sites);
  }

  /** Miejsca i poziomy z gry (null – bez gry). */
  set(slots: Uint16Array | null, levels: Uint8Array | null): void {
    this.clear();
    if (!slots || !levels) return;
    const provinces = Math.floor(levels.length / BUILDING_COUNT);
    for (let p = 0; p < provinces; p++) {
      const at = (k: number) => [slots[p * STRIDE + 2 * k], slots[p * STRIDE + 2 * k + 1]] as const;
      SLOT_BUILDINGS.forEach(({ building, index }, k) => {
        const level = levels[p * BUILDING_COUNT + index];
        const [x, y] = at(k);
        if (level === 0 || x === NO_SLOT) return;
        const sprite = new Sprite(
          this.texture(`${building}:${level}`, () => drawIcon(BUILDING_ICONS[building], BUILDING_COLORS[building], level)),
        );
        sprite.anchor.set(0.5);
        sprite.scale.set(ICON_TILES / TEXTURE_PX);
        sprite.position.set(x + 0.5, y + 0.5);
        this.built.addChild(sprite);
        this.icons.push({ sprite, x: x + 0.5, y: y + 0.5, province: p + 1, building });
      });
      const command = levels[p * BUILDING_COUNT + COMMAND_CENTER];
      const [cx, cy] = at(0);
      if (command > 0 && cx !== NO_SLOT) {
        const star = new Sprite(this.texture(`stars:${command}`, () => drawStars(command)));
        star.anchor.set(0.5, 1);
        star.scale.set((ICON_TILES / TEXTURE_PX) * STAR_SCALE);
        // Nad ikoną miasta (pivot w pikselach tekstury gwiazdek).
        star.pivot.set(0, (TEXTURE_PX * 0.42) / STAR_SCALE);
        star.position.set(cx + 0.5, cy + 0.5);
        this.built.addChild(star);
        this.stars.push(star);
      }
    }
  }

  /** Co klatkę: widoczność przy skali świata `scale` (px na kafel). */
  update(scale: number): void {
    this.view.visible = scale >= MIN_SCALE;
  }

  /** Budynek pod punktem świata (x, y), albo null. */
  hit(x: number, y: number): { province: number; building: SlotBuilding } | null {
    if (!this.view.visible) return null;
    const r = ICON_TILES * 0.42;
    let best: Icon | null = null;
    let bestD = r * r;
    for (const icon of [...this.icons, ...this.siteIcons]) {
      const d = (icon.x - x) ** 2 + (icon.y - y) ** 2;
      if (d <= bestD) {
        best = icon;
        bestD = d;
      }
    }
    return best && { province: best.province, building: best.building };
  }

  destroy(): void {
    this.clear();
    for (const t of this.textures.values()) t.destroy(true);
    this.textures.clear();
    this.view.destroy();
  }

  private clear(): void {
    for (const s of this.built.removeChildren()) s.destroy();
    this.icons = [];
    this.stars = [];
  }

  /**
   * Trwające budowy (`[(id - 1) * 3]` = rodzaj + 1 (0 – brak), poziom, postęp 0–255): ikona budynku
   * (nowy – półprzezroczysta z docelowym poziomem; rozbudowa – na istniejącej ikonie) i pierścień postępu.
   */
  setConstructions(slots: Uint16Array | null, data: Uint8Array | null): void {
    for (const s of this.sites.removeChildren()) if (s !== this.rings) s.destroy();
    this.sites.addChild(this.rings);
    this.rings.clear();
    this.siteIcons = [];
    if (!slots || !data) return;
    const r = ICON_TILES * 0.47;
    for (let p = 0; p * 3 < data.length; p++) {
      const kind = data[p * 3] - 1;
      if (kind < 0) continue;
      const level = data[p * 3 + 1];
      const progress = data[p * 3 + 2] / 255;
      const site = SITES[kind];
      if (!site) continue;
      const k = site.slot ? SLOT_BUILDINGS.findIndex((s) => s.building === site.slot) : 0;
      const sx = slots[p * STRIDE + 2 * k];
      const sy = slots[p * STRIDE + 2 * k + 1];
      if (sx === NO_SLOT) continue;
      // Bez miejsca – obok miasta (na lewo od jego ikony).
      const x = sx + 0.5 - (site.slot ? 0 : ICON_TILES * 1.1);
      const y = sy + 0.5;
      if (level <= 1 || !site.slot) {
        const sprite = new Sprite(this.texture(`site:${kind}:${level}`, () => drawIcon(site.path, site.color, level)));
        sprite.anchor.set(0.5);
        sprite.scale.set(ICON_TILES / TEXTURE_PX);
        sprite.alpha = 0.75;
        sprite.position.set(x, y);
        this.sites.addChildAt(sprite, 0);
        if (site.slot) this.siteIcons.push({ sprite, x, y, province: p + 1, building: site.slot });
      }
      this.rings.circle(x, y, r).stroke({ width: RING_WIDTH, color: RING_BACK, alpha: 0.85 });
      if (progress > 0) {
        const a0 = -Math.PI / 2;
        const a1 = a0 + progress * Math.PI * 2;
        this.rings.moveTo(x + Math.cos(a0) * r, y + Math.sin(a0) * r).arc(x, y, r, a0, a1).stroke({ width: RING_WIDTH * 0.7, color: RING_COLOR });
      }
    }
  }

  private texture(key: string, draw: () => HTMLCanvasElement): Texture {
    let t = this.textures.get(key);
    if (!t) {
      t = new Texture({ source: new CanvasSource({ resource: draw(), autoGenerateMipmaps: true }) });
      this.textures.set(key, t);
    }
    return t;
  }
}

/** Ikona: kółko w kolorze rodzaju z obwódką, piktogram budynku (`path`), plakietka z poziomem. */
function drawIcon(path: string, color: string, level: number): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = TEXTURE_PX;
  const ctx = canvas.getContext('2d')!;
  const c = TEXTURE_PX / 2;
  const r = TEXTURE_PX * 0.4;
  ctx.beginPath();
  ctx.arc(c, c, r, 0, Math.PI * 2);
  ctx.fillStyle = color;
  ctx.fill();
  ctx.lineWidth = TEXTURE_PX * 0.05;
  ctx.strokeStyle = '#0c1824';
  ctx.stroke();
  ctx.save();
  const k = (r * 1.15) / 24;
  ctx.translate(c - 12 * k, c - 12 * k);
  ctx.scale(k, k);
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  ctx.lineWidth = 1.9;
  ctx.strokeStyle = '#ffffff';
  ctx.stroke(new Path2D(path));
  ctx.restore();
  // Plakietka z poziomem (prawy dolny róg).
  const bx = TEXTURE_PX * 0.76;
  const by = TEXTURE_PX * 0.76;
  ctx.beginPath();
  ctx.arc(bx, by, TEXTURE_PX * 0.2, 0, Math.PI * 2);
  ctx.fillStyle = '#0c1824';
  ctx.fill();
  ctx.lineWidth = TEXTURE_PX * 0.035;
  ctx.strokeStyle = '#d8e2ea';
  ctx.stroke();
  ctx.fillStyle = '#ffffff';
  ctx.font = `700 ${TEXTURE_PX * 0.26}px system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(String(level), bx, by + TEXTURE_PX * 0.01);
  return canvas;
}

/** Gwiazdki w rzędach (od góry) dla poziomu 1–6 – zwarte jak kropki na kostce. */
const STAR_ROWS: readonly (readonly number[])[] = [[1], [2], [2, 1], [2, 2], [3, 2], [3, 3]];

/** Gwiazdki centrum dowodzenia (1–6) w rzędach `STAR_ROWS`, kolor wg poziomu; dół tekstury – nad miastem. */
function drawStars(level: number): HTMLCanvasElement {
  const rows = STAR_ROWS[level - 1];
  const size = TEXTURE_PX * 0.9;
  const gap = size * 0.98;
  const canvas = document.createElement('canvas');
  canvas.width = Math.ceil(gap * 3 + size * 0.1);
  canvas.height = Math.ceil(gap * 0.9 * (rows.length - 1) + size * 1.05);
  const ctx = canvas.getContext('2d')!;
  const centers = rows.flatMap((n, row) =>
    Array.from({ length: n }, (_, i) => [canvas.width / 2 + (i - (n - 1) / 2) * gap, size * 0.53 + row * gap * 0.9]),
  );
  for (const [cx, cy] of centers) {
    ctx.beginPath();
    for (let k = 0; k < 10; k++) {
      const rr = k % 2 === 0 ? size / 2 : size / 4.6;
      const a = -Math.PI / 2 + (k * Math.PI) / 5;
      ctx.lineTo(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
    }
    ctx.closePath();
    ctx.fillStyle = STAR_COLORS[level - 1];
    ctx.fill();
    ctx.lineWidth = TEXTURE_PX * 0.07;
    ctx.strokeStyle = '#0c1824';
    ctx.stroke();
  }
  return canvas;
}
