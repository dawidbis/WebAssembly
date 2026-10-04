import { Application, BufferImageSource, Container, Graphics, Sprite, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';
import { CreatureLayer } from './creatures';
import { HighlightLayer } from './highlight';
import { Painter } from './painter';
import type { TerrainView } from './terrain';
import { InlandWaterLayer } from './inland';
import { WaveLayer, type WaveSettings } from './waves';

/** Mapa jest cięta na tekstury tej wielkości (bezpieczny limit także dla mobilnych GPU). */
const TILE_TEXTURE = 512;
/** Ile ostatnio oglądanych widoków trzymać gotowych (powrót do nich jest natychmiastowy). */
const VIEW_CACHE = 3;
/** Czas przenikania widoków i pojawiania się granic prowincji (ms). */
const CROSSFADE_MS = 350;
const PROVINCES_FADE_MS = 900;

/** Warstwy jednego widoku. */
interface ViewLayers {
  terrain: Container;
}

/** Prosta animacja wartości 0..1 z łagodnym wyjściem (ease-out). */
interface Tween {
  elapsed: number;
  duration: number;
  step: (k: number) => void;
  done?: () => void;
}

/**
 * Renderer mapy w czystym TS + Pixi – celowo poza Angularem.
 * Angular tylko podaje element-host i woła metody publiczne.
 */
export class MapRenderer {
  private readonly app = new Application();
  private readonly world = new Container();
  /** Warstwy terenu kolejnych widoków (przenikają się przy zmianie widoku). */
  private readonly terrainGroup = new Container();
  private readonly painter = new Painter();
  /** Gotowe widoki bieżącej mapy (klucz: widok + izobaty), od najstarszego. */
  private readonly cache = new Map<string, ViewLayers>();
  private shown: ViewLayers | null = null;
  /** Numer malowania – wynik starszej mapy jest wyrzucany. */
  private paintEpoch = 0;
  private readonly tweens: Tween[] = [];
  private borderOpacity = 0.3;
  /** Teren bieżącej mapy jest już na ekranie (do tego czasu warstwy prowincji czekają). */
  private mapShown = false;
  private provincesFade = 1;
  /** Granice prowincji jako kafle (nakładka nad terenem, pod falami). */
  private readonly provinceLayer = new Container();
  private readonly highlight = new HighlightLayer();
  /** Stworki w prowincjach zablokowanych (po ostatnich szlifach). */
  private readonly creatures = new CreatureLayer();
  private readonly chunkGrid = new Graphics();
  private readonly waves = new WaveLayer();
  private readonly inland = new InlandWaterLayer();
  private readonly cleanup: (() => void)[] = [];
  private map: MapPayload | null = null;
  private view: TerrainView = 'terrain';
  private contours = true;
  private provinces = true;
  private wavesVisible = true;
  private ready = false;
  /** Kafel pod kursorem (null = poza mapą) – dla panelu debugu. */
  onHover: ((tile: { x: number; y: number } | null) => void) | null = null;
  /** Kliknięcie w kafel bez przeciągania mapy (null = poza mapą). */
  onTileClick: ((tile: { x: number; y: number } | null) => void) | null = null;
  /** Narysowano stworki w prowincjach zablokowanych (liczba) – etap „chowanie easter eggów”. */
  onCreatures: ((count: number) => void) | null = null;
  /** Trwa malowanie widoku (worker) – UI pokazuje wtedy „Rysowanie mapy…”. */
  onPainting: ((painting: boolean) => void) | null = null;

  async init(host: HTMLElement): Promise<void> {
    await this.app.init({
      resizeTo: host,
      background: '#081521',
      antialias: false,
      // Fale, rzeki i jeziora mają tylko shadery GLSL.
      preference: 'webgl',
      autoDensity: true,
      resolution: window.devicePixelRatio || 1,
    });
    host.appendChild(this.app.canvas);
    this.world.addChild(
      this.terrainGroup,
      this.inland.view,
      this.provinceLayer,
      this.creatures.view,
      this.highlight.view,
      this.waves.view,
      this.chunkGrid,
    );
    this.app.ticker.add((ticker) => {
      this.runTweens(ticker.deltaMS);
      this.provinceLayer.alpha = this.borderOpacity * this.provincesFade;
      if (this.waves.view.visible) {
        this.waves.tick(ticker.deltaMS / 1000);
        this.inland.update(ticker.deltaMS / 1000, 1 / (this.world.scale.x * this.app.renderer.resolution));
      }
    });
    this.app.stage.addChild(this.world);
    this.bindCamera(this.app.canvas);
    this.ready = true;
    if (this.map) this.setMap(this.map);
  }

  setMap(map: MapPayload): void {
    // Ta sama mapa, doszły prowincje (faza 2): przebuduj tylko warstwy prowincji.
    if (this.ready && this.map?.terrain === map.terrain) {
      this.addProvinces(map);
      return;
    }
    const first = !this.map;
    const sizeChanged = !this.map || this.map.width !== map.width || this.map.height !== map.height;
    this.map = map;
    if (!this.ready) return;
    const epoch = ++this.paintEpoch;
    this.mapShown = false;
    this.painter.setMap(map);
    // Stare widoki (poza wyświetlanym, który zniknie w przenikaniu) nie pasują do nowej mapy.
    for (const key of this.cache.keys()) {
      if (this.cache.get(key) !== this.shown) this.dropCached(key);
    }
    this.cache.clear();
    void this.showView(() => {
      if (epoch !== this.paintEpoch || !this.map) return;
      this.mapShown = true;
      // Warstwy zależne od mapy zmieniają się razem z terenem, nie przed nim. Prowincje mogły
      // dojść w trakcie malowania – `this.map` ma wtedy już je.
      this.clearProvinces();
      this.creatures.clear();
      void this.buildProvinces(this.map);
      this.showCreatures(this.map);
      this.waves.setMap(map);
      this.inland.setMap(map);
      this.drawChunkGrid(map);
      if (sizeChanged) this.fit();
      // Znacznik do pomiarów czasu wczytania (DevTools → Performance, testy obciążeniowe).
      performance.mark('map-rendered', { detail: { generateMs: map.ms, width: map.width, height: map.height } });
    }, first ? 600 : CROSSFADE_MS);
  }

  /** Faza 2 tej samej mapy: przychodzą prowincje – przebudowa tylko zależnych od nich warstw. */
  private addProvinces(map: MapPayload): void {
    // Ostatnie szlify tej samej mapy: prowincje bez zmian, dochodzą tylko stworki. Obie fazy mogą
    // też przyjść naraz (jedna zmiana sygnału) – wtedy stworki rysuje gałąź prowincji.
    const sameProvinces = map.province === this.map?.province;
    this.map = map;
    this.painter.setProvinces(map);
    if (this.mapShown) this.showCreatures(map);
    if (sameProvinces) return;
    if (this.mapShown) void this.buildProvinces(map);
    // Widok polityczny zależy od prowincji – namaluj go od nowa.
    for (const [key, layers] of this.cache) {
      if (key.startsWith('political') && layers !== this.shown) this.dropCached(key);
    }
    if (this.view === 'political') {
      this.cache.delete(this.viewKey());
      void this.showView();
    }
  }

  /** Styl terenu: pełne palety biomów albo płaska „mapa biomów” (debug). */
  setView(view: TerrainView): void {
    if (view === this.view) return;
    this.view = view;
    this.applyVisibility();
    if (this.ready && this.map) void this.showView();
  }

  /** Podświetlenie prowincji: zaznaczonej (mocniej, z wyraźnym skrajem) i pod kursorem (lekko). */
  setHighlight(selected: number, hovered: number): void {
    this.highlight.set(selected, hovered);
  }

  /** Nakładka granic prowincji (na mapie politycznej granice są zawsze wrysowane w kolory). */
  setProvinces(visible: boolean, opacity: number): void {
    this.provinces = visible;
    // Krycie całej warstwy: kafle granic są nieprzezroczyste, więc to jest krycie granicy.
    this.borderOpacity = opacity;
    this.applyVisibility();
  }

  /** Animacja fal brzegowych: przybój i piana przy linii brzegu. */
  setWaves(visible: boolean, settings: WaveSettings): void {
    this.wavesVisible = visible;
    this.waves.configure(settings);
    this.inland.configure(settings.inland, settings.speed);
    this.applyVisibility();
  }

  /** Mapa polityczna pokazuje tylko prowincje: bez animacji wody. */
  private applyVisibility(): void {
    const political = this.view === 'political';
    this.waves.view.visible = this.wavesVisible && !political;
    this.inland.view.visible = this.wavesVisible && !political;
    this.provinceLayer.visible = this.provinces && !political;
  }

  /** Izobaty – linie jednakowej głębokości oceanu. */
  setContours(visible: boolean): void {
    if (visible === this.contours) return;
    this.contours = visible;
    if (this.ready && this.map) void this.showView();
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
    this.waves.destroy();
    this.inland.destroy();
    this.highlight.destroy();
    this.creatures.destroy();
    this.painter.destroy();
    this.app.destroy(true, { children: true, texture: true, textureSource: true });
  }

  private viewKey(): string {
    return `${this.view}|${this.contours}`;
  }

  /**
   * Pokazuje bieżący widok: z pamięci od razu, inaczej maluje go worker (strona nie zamiera).
   * Nowy widok przenika stary; `ready` woła się tuż przed przenikaniem.
   */
  private async showView(ready?: () => void, fadeMs = CROSSFADE_MS): Promise<void> {
    const map = this.map;
    if (!map) return;
    const epoch = this.paintEpoch;
    const key = this.viewKey();
    let layers = this.cache.get(key);
    if (!layers) {
      this.onPainting?.(true);
      const r = await this.painter.terrain(this.view, this.contours);
      // Inna mapa w międzyczasie – wynik do kosza.
      if (epoch !== this.paintEpoch) return;
      layers = this.cache.get(key) ?? { terrain: this.textureLayer(map, r.terrain) };
      this.remember(key, layers);
      if (this.viewKey() !== key) return; // użytkownik przełączył dalej – zostaje w pamięci
    } else {
      this.remember(key, layers);
    }
    this.onPainting?.(false);
    ready?.();
    this.crossfade(layers, fadeMs);
  }

  /** Dopisuje widok do pamięci (najnowszy na końcu) i wyrzuca najstarsze ponad limit. */
  private remember(key: string, layers: ViewLayers): void {
    this.cache.delete(key);
    this.cache.set(key, layers);
    for (const old of this.cache.keys()) {
      if (this.cache.size <= VIEW_CACHE) break;
      if (this.cache.get(old) !== this.shown && old !== key) this.dropCached(old);
    }
  }

  /** Czy widok jest w pamięci widoków. */
  private cachedLayers(layers: ViewLayers): boolean {
    for (const cached of this.cache.values()) if (cached === layers) return true;
    return false;
  }

  private dropCached(key: string): void {
    const layers = this.cache.get(key);
    this.cache.delete(key);
    if (!layers || layers === this.shown) return;
    layers.terrain.destroy({ children: true, texture: true, textureSource: true });
  }

  /** Nowy widok nad starym, alfa 0 → 1; potem stary znika (zostaje w pamięci, jeśli tam jest). */
  private crossfade(next: ViewLayers, ms: number): void {
    const prev = this.shown;
    if (prev === next) return;
    this.shown = next;
    this.terrainGroup.addChild(next.terrain); // na wierzch
    next.terrain.visible = true;
    next.terrain.alpha = 0;
    this.tween(ms, (k) => {
      next.terrain.alpha = k;
    }, () => {
      if (!prev || this.shown === prev) return;
      const c = prev.terrain;
      if (this.cachedLayers(prev)) {
        c.visible = false;
        c.removeFromParent();
      } else {
        c.destroy({ children: true, texture: true, textureSource: true });
      }
    });
  }

  private tween(duration: number, step: (k: number) => void, done?: () => void): void {
    step(0);
    this.tweens.push({ elapsed: 0, duration, step, done });
  }

  private runTweens(deltaMs: number): void {
    for (let i = this.tweens.length - 1; i >= 0; i--) {
      const t = this.tweens[i];
      t.elapsed += deltaMs;
      const x = Math.min(1, t.elapsed / t.duration);
      t.step(1 - (1 - x) * (1 - x) * (1 - x));
      if (x >= 1) {
        this.tweens.splice(i, 1);
        t.done?.();
      }
    }
  }

  /** Stworki w prowincjach zablokowanych – dopiero gdy ostatnie szlify są gotowe. */
  private showCreatures(map: MapPayload): void {
    if (!map.polished) return;
    this.onCreatures?.(this.creatures.setMap(map));
  }

  private clearProvinces(): void {
    for (const old of this.provinceLayer.removeChildren()) old.destroy({ texture: true, textureSource: true });
  }

  /** Granice i podświetlenie prowincji – puste, dopóki prowincje się liczą; pojawiają się łagodnie. */
  private async buildProvinces(map: MapPayload): Promise<void> {
    if (!map.provincesReady) return;
    const epoch = this.paintEpoch;
    const r = await this.painter.borders();
    if (epoch !== this.paintEpoch || this.map !== map) return;
    this.clearProvinces();
    this.provinceLayer.addChild(this.textureLayer(map, r.borders));
    this.highlight.setMap(map);
    this.tween(PROVINCES_FADE_MS, (k) => (this.provincesFade = k));
    performance.mark('provinces-rendered', { detail: { provincesMs: map.provincesMs } });
  }

  /** Warstwa z bufora RGBA całej mapy, pocięta na tekstury `TILE_TEXTURE` × `TILE_TEXTURE`. */
  private textureLayer(map: MapPayload, rgba: Uint8ClampedArray): Container {
    const layer = new Container();
    for (let y0 = 0; y0 < map.height; y0 += TILE_TEXTURE) {
      for (let x0 = 0; x0 < map.width; x0 += TILE_TEXTURE) {
        const w = Math.min(TILE_TEXTURE, map.width - x0);
        const h = Math.min(TILE_TEXTURE, map.height - y0);
        const data = new Uint8Array(w * h * 4);
        for (let row = 0; row < h; row++) {
          const from = ((y0 + row) * map.width + x0) * 4;
          data.set(rgba.subarray(from, from + w * 4), row * w * 4);
        }
        const texture = new Texture({ source: new BufferImageSource({ resource: data, width: w, height: h, scaleMode: 'nearest' }) });
        const sprite = new Sprite(texture);
        sprite.position.set(x0, y0);
        layer.addChild(sprite);
      }
    }
    return layer;
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

  private hover(e: PointerEvent, canvas: HTMLCanvasElement): void {
    this.onHover?.(this.tileAt(e, canvas));
  }

  /** Kafel pod kursorem albo null poza mapą. */
  private tileAt(e: PointerEvent, canvas: HTMLCanvasElement): { x: number; y: number } | null {
    if (!this.map) return null;
    const rect = canvas.getBoundingClientRect();
    const x = Math.floor((e.clientX - rect.left - this.world.x) / this.world.scale.x);
    const y = Math.floor((e.clientY - rect.top - this.world.y) / this.world.scale.y);
    return x >= 0 && y >= 0 && x < this.map.width && y < this.map.height ? { x, y } : null;
  }

  /** Przeciąganie przesuwa mapę, kółko przybliża względem kursora. */
  private bindCamera(canvas: HTMLCanvasElement): void {
    let last: { x: number; y: number } | null = null;
    // Łączne przesunięcie od wciśnięcia – do odróżnienia kliknięcia od przeciągania.
    let dragged = 0;
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
      dragged = 0;
      canvas.setPointerCapture(e.pointerId);
    });
    listen('pointermove', (e) => {
      if (!last) {
        this.hover(e, canvas);
        return;
      }
      this.world.x += e.clientX - last.x;
      this.world.y += e.clientY - last.y;
      dragged += Math.abs(e.clientX - last.x) + Math.abs(e.clientY - last.y);
      last = { x: e.clientX, y: e.clientY };
    });
    listen('pointerup', (e) => {
      if (last && dragged < 5) this.onTileClick?.(this.tileAt(e, canvas));
      last = null;
    });
    listen('pointerleave', () => this.onHover?.(null));
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
