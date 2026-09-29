import { Application, CanvasSource, Container, Graphics, Sprite, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';
import { paintTerrain } from './terrain';

/** Mapa jest cięta na tekstury tej wielkości (bezpieczny limit także dla mobilnych GPU). */
const TILE_TEXTURE = 512;

/**
 * Renderer mapy w czystym TS + Pixi – celowo poza Angularem.
 * Angular tylko podaje element-host i woła metody publiczne.
 */
export class MapRenderer {
  private readonly app = new Application();
  private readonly world = new Container();
  private readonly terrainLayer = new Container();
  private readonly chunkGrid = new Graphics();
  private readonly cleanup: (() => void)[] = [];
  private map: MapPayload | null = null;
  private ready = false;

  async init(host: HTMLElement): Promise<void> {
    await this.app.init({
      resizeTo: host,
      background: '#081521',
      antialias: false,
      autoDensity: true,
      resolution: window.devicePixelRatio || 1,
    });
    host.appendChild(this.app.canvas);
    this.world.addChild(this.terrainLayer, this.chunkGrid);
    this.app.stage.addChild(this.world);
    this.bindCamera(this.app.canvas);
    this.ready = true;
    if (this.map) this.setMap(this.map);
  }

  setMap(map: MapPayload): void {
    const sizeChanged = !this.map || this.map.width !== map.width || this.map.height !== map.height;
    this.map = map;
    if (!this.ready) return;
    this.buildTerrain(map);
    this.drawChunkGrid(map);
    if (sizeChanged) this.fit();
  }

  setChunkGridVisible(visible: boolean): void {
    this.chunkGrid.visible = visible;
  }

  fit(): void {
    if (!this.map || !this.ready) return;
    const { width, height } = this.app.screen;
    const scale = Math.min(width / this.map.width, height / this.map.height) * 0.95;
    this.world.scale.set(scale);
    this.world.position.set((width - this.map.width * scale) / 2, (height - this.map.height * scale) / 2);
  }

  destroy(): void {
    this.cleanup.forEach((fn) => fn());
    this.app.destroy(true, { children: true, texture: true, textureSource: true });
  }

  private buildTerrain(map: MapPayload): void {
    for (const old of this.terrainLayer.removeChildren()) old.destroy({ texture: true, textureSource: true });

    const rgba = paintTerrain(map);
    for (let y0 = 0; y0 < map.height; y0 += TILE_TEXTURE) {
      for (let x0 = 0; x0 < map.width; x0 += TILE_TEXTURE) {
        const w = Math.min(TILE_TEXTURE, map.width - x0);
        const h = Math.min(TILE_TEXTURE, map.height - y0);
        const image = new ImageData(w, h);
        for (let row = 0; row < h; row++) {
          const from = ((y0 + row) * map.width + x0) * 4;
          image.data.set(rgba.subarray(from, from + w * 4), row * w * 4);
        }
        const canvas = document.createElement('canvas');
        canvas.width = w;
        canvas.height = h;
        canvas.getContext('2d')!.putImageData(image, 0, 0);

        const texture = new Texture({ source: new CanvasSource({ resource: canvas, scaleMode: 'nearest' }) });
        const sprite = new Sprite(texture);
        sprite.position.set(x0, y0);
        this.terrainLayer.addChild(sprite);
      }
    }
  }

  private drawChunkGrid(map: MapPayload): void {
    // Granice liczone tak samo jak `game_mapgen::chunk_start`.
    const colX = (c: number) => Math.ceil((c * map.width) / map.chunkCols);
    const rowY = (r: number) => Math.ceil((r * map.height) / map.chunkRows);
    const g = this.chunkGrid.clear();

    for (let r = 0; r < map.chunkRows; r++) {
      for (let c = 0; c < map.chunkCols; c++) {
        if (map.waterChunks[r * map.chunkCols + c]) {
          g.rect(colX(c), rowY(r), colX(c + 1) - colX(c), rowY(r + 1) - rowY(r)).fill({ color: 0x7fc4ff, alpha: 0.08 });
        }
      }
    }
    for (let c = 0; c <= map.chunkCols; c++) g.moveTo(colX(c), 0).lineTo(colX(c), map.height);
    for (let r = 0; r <= map.chunkRows; r++) g.moveTo(0, rowY(r)).lineTo(map.width, rowY(r));
    g.stroke({ width: 1, color: 0xffffff, alpha: 0.3, pixelLine: true });
  }

  /** Przeciąganie przesuwa mapę, kółko przybliża względem kursora. */
  private bindCamera(canvas: HTMLCanvasElement): void {
    let last: { x: number; y: number } | null = null;
    const listen = <K extends keyof HTMLElementEventMap>(
      type: K,
      fn: (e: HTMLElementEventMap[K]) => void,
      options?: AddEventListenerOptions,
    ) => {
      canvas.addEventListener(type, fn, options);
      this.cleanup.push(() => canvas.removeEventListener(type, fn, options));
    };

    listen('pointerdown', (e) => {
      last = { x: e.clientX, y: e.clientY };
      canvas.setPointerCapture(e.pointerId);
    });
    listen('pointermove', (e) => {
      if (!last) return;
      this.world.x += e.clientX - last.x;
      this.world.y += e.clientY - last.y;
      last = { x: e.clientX, y: e.clientY };
    });
    listen('pointerup', () => (last = null));
    listen(
      'wheel',
      (e) => {
        e.preventDefault();
        const rect = canvas.getBoundingClientRect();
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const old = this.world.scale.x;
        const next = Math.min(32, Math.max(0.05, old * Math.exp(-e.deltaY * 0.0015)));
        this.world.x = mx - ((mx - this.world.x) * next) / old;
        this.world.y = my - ((my - this.world.y) * next) / old;
        this.world.scale.set(next);
      },
      { passive: false },
    );
  }
}
