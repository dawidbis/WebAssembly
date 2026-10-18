import { Container, Graphics, Sprite, Text, TextStyle, Texture } from 'pixi.js';

import { MIN_SCALE } from './buildings';

/** Jednostka rysunku znaczników = tyle kafli (skaluje się z mapą jak ikony budynków). */
const TILES_PER_UNIT = 0.35;

/** Wygląd zasięgu portu (suwaki w panelu debugu). */
export interface PortReachStyle {
  /** Krycie zasięgu desantu na morzu. */
  fill: number;
  /** Promień znacznika portu (px ekranu). */
  ring: number;
}

export const DEFAULT_PORT_REACH_STYLE: PortReachStyle = { fill: 0.8, ring: 9 };

/** Kolor zasięgu desantu (RGB; środek półprzezroczysty, krawędź pełna) i znaczników portów: nasz – jasny,
 * partnerzy handlu – złoty. */
const REACH_RGB = [255, 244, 200] as const;
const REACH_INSIDE = 70;
const REACH_EDGE = 255;
/** Ocean (`Terrain::Ocean`). */
const OCEAN = 0;
const PORT_COLOR = 0xe8f6ff;
const PARTNER_COLOR = 0xf2d47a;

const STYLE = new TextStyle({
  fontFamily: 'system-ui, sans-serif',
  fontWeight: '700',
  fontSize: 11,
  fill: '#ffffff',
  stroke: { color: '#10161d', width: 3 },
});

/** Port do pokazania: kafel portu, zasięg (kafle morza) i porty partnerów z podpisem (koszt dostawy). */
export interface PortView {
  x: number;
  y: number;
  reach: Uint32Array;
  partners: readonly { x: number; y: number; label: string }[];
}

interface Mark {
  view: Container;
  x: number;
  y: number;
}

/**
 * Zasięg portu po kliknięciu ikony portu: kafle morza w zasięgu (rozjaśnione – desant tu jest połączony z
 * krajem, dalej tworzy przyczółek), znacznik
 * portu i porty partnerów handlu morskiego nad tym samym morzem z kosztem dostawy. Znaczniki mają
 * wielkość w kaflach (jak ikony budynków); z daleka warstwa znika.
 */
export class PortReachLayer {
  readonly view = new Container();
  private area: Sprite | null = null;
  private marks: Mark[] = [];
  private style: PortReachStyle = DEFAULT_PORT_REACH_STYLE;
  private current: PortView | null = null;
  private terrain: Uint8Array | undefined;

  /** Port do pokazania (null – nic). `width` – szerokość mapy w kaflach, `terrain` – typy kafli (krawędź
   * zasięgu tylko na morzu, nie wzdłuż brzegu). */
  set(port: PortView | null, width: number, terrain?: Uint8Array): void {
    this.terrain = terrain;
    this.clear();
    this.current = port;
    if (!port) return;
    this.area = reachSprite(port.reach, width, this.terrain);
    if (this.area) {
      this.area.alpha = this.style.fill;
      this.view.addChild(this.area);
    }
    this.marks.push(this.mark(port.x, port.y, PORT_COLOR, ''));
    for (const p of port.partners) this.marks.push(this.mark(p.x, p.y, PARTNER_COLOR, p.label));
  }

  setStyle(style: PortReachStyle, width: number): void {
    this.style = style;
    this.set(this.current, width, this.terrain);
  }

  /** Co klatkę: z daleka (`scale` – px ekranu na kafel) warstwa znika jak ikony budynków. */
  update(scale: number): void {
    this.view.visible = scale >= MIN_SCALE;
  }

  destroy(): void {
    this.clear();
    this.view.destroy({ children: true });
  }

  private clear(): void {
    for (const c of this.view.removeChildren()) c.destroy({ children: true, texture: true, textureSource: true });
    this.area = null;
    this.marks = [];
  }

  private mark(x: number, y: number, color: number, label: string): Mark {
    const view = new Container();
    const r = this.style.ring;
    view.addChild(new Graphics().circle(0, 0, r).stroke({ width: 2.5, color: 0x10161d }).circle(0, 0, r).stroke({ width: 1.5, color }));
    if (label) {
      const text = new Text({ text: label, style: STYLE, resolution: 6 });
      text.anchor.set(0.5, 1);
      text.position.set(0, -r - 2);
      view.addChild(text);
    }
    view.position.set(x + 0.5, y + 0.5);
    view.scale.set(TILES_PER_UNIT);
    this.view.addChild(view);
    return { view, x, y };
  }
}

/** Kafle zasięgu jako półprzezroczysty obraz w prostokącie, który je obejmuje (null – pusto). */
function reachSprite(reach: Uint32Array, width: number, terrain?: Uint8Array): Sprite | null {
  if (reach.length === 0) return null;
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const t of reach) {
    const x = t % width;
    const y = (t - x) / width;
    x0 = Math.min(x0, x);
    y0 = Math.min(y0, y);
    x1 = Math.max(x1, x);
    y1 = Math.max(y1, y);
  }
  const w = x1 - x0 + 1;
  const h = y1 - y0 + 1;
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;
  const img = ctx.createImageData(w, h);
  const inside = new Uint8Array(w * h);
  for (const t of reach) inside[(Math.floor(t / width) - y0) * w + (t % width) - x0] = 1;
  const at = (x: number, y: number) => x >= 0 && y >= 0 && x < w && y < h && inside[y * w + x] === 1;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (!at(x, y)) continue;
      // Krawędź zasięgu: sąsiad bokiem poza nim, ale na morzu (koniec zasięgu, nie brzeg) – wyraźna linia.
      const out = (dx: number, dy: number) => !at(x + dx, y + dy) && (terrain?.[(y + y0 + dy) * width + x + x0 + dx] ?? 0) === OCEAN;
      const edge = out(-1, 0) || out(1, 0) || out(0, -1) || out(0, 1);
      const o = (y * w + x) * 4;
      img.data[o] = REACH_RGB[0];
      img.data[o + 1] = REACH_RGB[1];
      img.data[o + 2] = REACH_RGB[2];
      img.data[o + 3] = edge ? REACH_EDGE : REACH_INSIDE;
    }
  }
  ctx.putImageData(img, 0, 0);
  const texture = Texture.from(canvas);
  texture.source.scaleMode = 'nearest';
  const sprite = new Sprite(texture);
  sprite.position.set(x0, y0);
  return sprite;
}
