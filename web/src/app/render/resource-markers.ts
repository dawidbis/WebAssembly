import { CanvasSource, Container, Sprite, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';
import { ICONS, RESOURCES } from './resources';

/** Rozmiar tekstury znaku (px) – ostry do przybliżenia ok. 10 px na kafel. */
const TEXTURE_PX = 128;
/** Wielkość znaku w kaflach: ułamek „średnicy” prowincji (√ liczby kafli), w granicach. */
const MARK_SHARE = 0.42;
const MARK_MIN = 6;
const MARK_MAX = 14;

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

/**
 * Znaki zasobów na środkach prowincji (mapa zasobów): ikony energii, piktogramy (rolnictwo, hodowla, las)
 * i symbole (metale, chemia).
 * Tekstura na zasób powstaje raz (canvas 2D, te same ścieżki SVG co w legendzie), znaki to sprite'y
 * w kaflach mapy – skalują się razem z nią.
 */
export class ResourceMarkers {
  readonly view = new Container();
  private readonly textures = new Map<number, Texture>();

  /** Rysuje znaki zasobów zaznaczonych w legendzie (`enabled`); przed fazą 3 – nic. */
  setMap(map: MapPayload, enabled: readonly boolean[]): void {
    this.clear();
    if (!map.polished) return;
    for (const p of map.provinces) {
      const info = RESOURCES[p.resource];
      if (!info || (!info.icon && !info.symbol && !info.glyph) || enabled[p.resource] === false) continue;
      const sprite = new Sprite(this.texture(p.resource));
      const size = clamp(Math.sqrt(p.area) * MARK_SHARE, MARK_MIN, MARK_MAX);
      sprite.anchor.set(0.5);
      sprite.setSize(size, size);
      sprite.position.set(p.centerX + 0.5, p.centerY + 0.5);
      this.view.addChild(sprite);
    }
  }

  clear(): void {
    for (const s of this.view.removeChildren()) s.destroy();
  }

  destroy(): void {
    this.clear();
    for (const t of this.textures.values()) t.destroy(true);
    this.textures.clear();
    this.view.destroy();
  }

  private texture(resource: number): Texture {
    let texture = this.textures.get(resource);
    if (!texture) {
      texture = new Texture({ source: new CanvasSource({ resource: drawMark(resource), autoGenerateMipmaps: true }) });
      this.textures.set(resource, texture);
    }
    return texture;
  }
}

/** Znak zasobu na kwadratowym canvasie: ikona ze ścieżek SVG albo symbol pierwiastka (biały z czarnym obrysem). */
function drawMark(resource: number): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = TEXTURE_PX;
  const ctx = canvas.getContext('2d')!;
  const { icon, symbol, glyph } = RESOURCES[resource];
  if (glyph) {
    // Biała linia z czarnym obrysem – jak symbole pierwiastków.
    const k = TEXTURE_PX / 24;
    ctx.scale(k, k);
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    const path = new Path2D(glyph);
    ctx.strokeStyle = '#000000';
    ctx.lineWidth = 4.2;
    ctx.stroke(path);
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 2;
    ctx.stroke(path);
  } else if (icon) {
    const k = TEXTURE_PX / 24;
    ctx.scale(k, k);
    ctx.lineJoin = 'round';
    for (const shape of ICONS[icon]) {
      const path = new Path2D(shape.d);
      ctx.fillStyle = shape.fill;
      ctx.fill(path);
      if (shape.stroke) {
        ctx.strokeStyle = shape.stroke;
        ctx.lineWidth = shape.strokeWidth ?? 1;
        ctx.stroke(path);
      }
    }
  } else if (symbol) {
    ctx.font = `700 ${TEXTURE_PX * 0.56}px system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.lineJoin = 'round';
    ctx.lineWidth = TEXTURE_PX * 0.1;
    ctx.strokeStyle = '#000000';
    const y = TEXTURE_PX * 0.54;
    ctx.strokeText(symbol, TEXTURE_PX / 2, y, TEXTURE_PX * 0.9);
    ctx.fillStyle = '#ffffff';
    ctx.fillText(symbol, TEXTURE_PX / 2, y, TEXTURE_PX * 0.9);
  }
  return canvas;
}
