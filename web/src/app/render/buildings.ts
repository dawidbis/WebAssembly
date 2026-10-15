import { CanvasSource, Container, Sprite, Texture } from 'pixi.js';

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

/** Rozmiar ikony na ekranie (px) i tekstury (px). */
const ICON_PX = 26;
const TEXTURE_PX = 64;
/** Ikony widać od takiego przybliżenia (px ekranu na kafel) – z daleka zasłaniałyby mapę. */
const MIN_SCALE = 2.5;
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
 * miastem stolicy gwiazdki centrum dowodzenia. Wielkość na ekranie stała (niezależna od przybliżenia);
 * z daleka ukryte. `hit` – który budynek jest pod kursorem (kliknięcie otwiera okienko budynku).
 */
export class BuildingLayer {
  readonly view = new Container();
  private icons: Icon[] = [];
  private stars: Sprite[] = [];
  private readonly textures = new Map<string, Texture>();

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
        const sprite = new Sprite(this.texture(`${building}:${level}`, () => drawIcon(building, level)));
        sprite.anchor.set(0.5);
        sprite.position.set(x + 0.5, y + 0.5);
        this.view.addChild(sprite);
        this.icons.push({ sprite, x: x + 0.5, y: y + 0.5, province: p + 1, building });
      });
      const command = levels[p * BUILDING_COUNT + COMMAND_CENTER];
      const [cx, cy] = at(0);
      if (command > 0 && cx !== NO_SLOT) {
        const star = new Sprite(this.texture(`stars:${command}`, () => drawStars(command)));
        star.anchor.set(0.5, 1);
        star.position.set(cx + 0.5, cy + 0.5);
        this.view.addChild(star);
        this.stars.push(star);
      }
    }
  }

  /** Co klatkę: stała wielkość na ekranie przy skali świata `scale` (px na kafel). */
  update(scale: number): void {
    this.view.visible = scale >= MIN_SCALE;
    if (!this.view.visible) return;
    const k = ICON_PX / TEXTURE_PX / scale;
    for (const icon of this.icons) icon.sprite.scale.set(k);
    for (const star of this.stars) {
      star.scale.set(k);
      // Gwiazdki nad ikoną miasta.
      star.pivot.set(0, TEXTURE_PX * 0.45);
    }
  }

  /** Budynek pod punktem świata (x, y) przy skali `scale`, albo null. */
  hit(x: number, y: number, scale: number): { province: number; building: SlotBuilding } | null {
    if (!this.view.visible) return null;
    const r = ICON_PX / 2 / scale;
    let best: Icon | null = null;
    let bestD = r * r;
    for (const icon of this.icons) {
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
    for (const s of this.view.removeChildren()) s.destroy();
    this.icons = [];
    this.stars = [];
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

/** Ikona: ciemne kółko z obwódką, piktogram budynku, plakietka z poziomem. */
function drawIcon(building: SlotBuilding, level: number): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = TEXTURE_PX;
  const ctx = canvas.getContext('2d')!;
  const c = TEXTURE_PX / 2;
  const r = TEXTURE_PX * 0.4;
  ctx.beginPath();
  ctx.arc(c, c, r, 0, Math.PI * 2);
  ctx.fillStyle = 'rgba(12, 24, 36, 0.92)';
  ctx.fill();
  ctx.lineWidth = TEXTURE_PX * 0.05;
  ctx.strokeStyle = '#d8e2ea';
  ctx.stroke();
  ctx.save();
  const k = (r * 1.15) / 24;
  ctx.translate(c - 12 * k, c - 12 * k);
  ctx.scale(k, k);
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  ctx.lineWidth = 1.9;
  ctx.strokeStyle = '#ffffff';
  ctx.stroke(new Path2D(BUILDING_ICONS[building]));
  ctx.restore();
  // Plakietka z poziomem (prawy dolny róg).
  const bx = TEXTURE_PX * 0.76;
  const by = TEXTURE_PX * 0.76;
  ctx.beginPath();
  ctx.arc(bx, by, TEXTURE_PX * 0.2, 0, Math.PI * 2);
  ctx.fillStyle = '#2f6f55';
  ctx.fill();
  ctx.lineWidth = TEXTURE_PX * 0.035;
  ctx.strokeStyle = '#0c1824';
  ctx.stroke();
  ctx.fillStyle = '#ffffff';
  ctx.font = `700 ${TEXTURE_PX * 0.26}px system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(String(level), bx, by + TEXTURE_PX * 0.01);
  return canvas;
}

/** Gwiazdki centrum dowodzenia (1–6) w rzędzie, kolor wg poziomu. */
function drawStars(level: number): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = TEXTURE_PX * 5.6;
  canvas.height = TEXTURE_PX;
  const ctx = canvas.getContext('2d')!;
  const size = TEXTURE_PX * 0.9;
  const gap = size * 0.98;
  const x0 = canvas.width / 2 - ((level - 1) * gap) / 2;
  for (let i = 0; i < level; i++) {
    const cx = x0 + i * gap;
    const cy = canvas.height / 2;
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
