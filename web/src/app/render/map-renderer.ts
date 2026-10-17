import { Application, BufferImageSource, Container, Graphics, Sprite, Texture } from 'pixi.js';

import type { MapPayload } from '../worker/protocol';
import { BuildingLayer, type BuildingFilter, type SlotBuilding } from './buildings';
import { CreatureLayer } from './creatures';
import { TerritoryLayer } from './territory';
import { ResourceMarkers } from './resource-markers';
import { HighlightLayer } from './highlight';
import { Painter } from './painter';
import { TERRITORY_BORDER_WIDTH } from './owners';
import { isFlatView, type TerrainView } from './terrain';
import { InlandWaterLayer } from './inland';
import { TerritoryNames } from './territory-names';
import { WaveLayer, type WaveSettings } from './waves';

/** Wygląd terytoriów państw (opcje pod zębatką; grubość granic – panel debugu). */
/** Dwa kliknięcia w tym czasie (ms) to dwuklik. */
const DOUBLE_CLICK_MS = 350;

export interface TerritoryStyle {
  /** Krycie granic państw na mapie terenu (osobno od granic prowincji). */
  borderOpacity: number;
  /** Grubość granic państw w kaflach (w głąb państwa). */
  borderWidth: number;
  /** Tło terytorium na mapie terenu: kolor państwa albo jego flaga. */
  fill: 'color' | 'flag';
  fillOpacity: number;
  flagOpacity: number;
  showNames: boolean;
  namesOpacity: number;
}

export const DEFAULT_TERRITORY_STYLE: TerritoryStyle = {
  borderOpacity: 0.85,
  borderWidth: TERRITORY_BORDER_WIDTH,
  fill: 'color',
  fillOpacity: 0.25,
  flagOpacity: 0.45,
  showNames: true,
  namesOpacity: 0.8,
};

/** Warstwy terytoriów malowane przez worker (raz na sekundę) – reszta liczy się na żywo (`TerritoryLayer`). */
const TERRITORY_LAYERS = ['flags'] as const;

/** Mapa jest cięta na tekstury tej wielkości (bezpieczny limit także dla mobilnych GPU). */
const TILE_TEXTURE = 512;
/** Ile ostatnio oglądanych widoków trzymać gotowych (powrót do nich jest natychmiastowy). */
const VIEW_CACHE = 3;
/** Czas przenikania widoków i pojawiania się granic prowincji (ms). */
const CROSSFADE_MS = 350;
/** Przenikanie po zmianie filtra zasobów – krótkie, bo klika się w legendę seriami. */
const FILTER_FADE_MS = 120;
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
  /** Ostatnie kliknięcie (dwuklik). */
  private lastClick: { at: number; x: number; y: number } | null = null;
  private provincesFade = 1;
  /**
   * Terytoria państw malowane przez worker: flagi jako półprzezroczyste tło na mapie terenu. Kolory
   * (mapa polityczna), tło w kolorze państwa i granice liczy na żywo `live` – co turę.
   */
  private readonly territory = {
    flags: new Container(),
  };
  /** Nazwy państw (terytoriów) nad mapą – na mapie terenu i politycznej. */
  private readonly names = new TerritoryNames();
  private territoryStyle: TerritoryStyle = DEFAULT_TERRITORY_STYLE;
  /** Państwo na kafel z bieżącej gry (null = brak gry albo nikt nic nie ma). */
  private owners: Uint8Array | null = null;
  private nationNames = new Map<number, string>();
  /** Numer malowania właścicieli – starszy wynik jest wyrzucany. */
  private ownersRequest = 0;
  /** Terytoria na żywo (GPU, co turę): kolory polityczne, tło i granice państw, przejście nowych kafli. */
  private readonly live = new TerritoryLayer();
  /** Granice prowincji jako kafle (nakładka nad terenem, pod falami). */
  private readonly provinceLayer = new Container();
  private readonly highlight = new HighlightLayer();
  /** Stworki w prowincjach zablokowanych (po ostatnich szlifach). */
  private readonly creatures = new CreatureLayer();
  /** Budynki na mapie: ikony z poziomem w stałych miejscach prowincji, gwiazdki centrum dowodzenia. */
  private readonly buildings = new BuildingLayer();
  /** Ikony energii i symbole metali na mapie zasobów. */
  private readonly markers = new ResourceMarkers();
  private readonly chunkGrid = new Graphics();
  private readonly waves = new WaveLayer();
  private readonly inland = new InlandWaterLayer();
  private readonly cleanup: (() => void)[] = [];
  private map: MapPayload | null = null;
  private view: TerrainView = 'terrain';
  private contours = true;
  /** Zasoby zaznaczone w legendzie mapy zasobów (pusta lista = wszystkie). */
  private resourceFilter: readonly boolean[] = [];
  private provinces = true;
  private wavesVisible = true;
  private ready = false;
  /** Kafel pod kursorem (null = poza mapą) – dla panelu debugu. */
  onHover: ((tile: { x: number; y: number } | null) => void) | null = null;
  /** Kliknięcie w kafel bez przeciągania mapy (null = poza mapą). */
  onTileClick: ((tile: { x: number; y: number } | null, mods: { ctrl: boolean }) => void) | null = null;
  /** Dwuklik (dwa kliknięcia w `DOUBLE_CLICK_MS` blisko siebie) – po zwykłym kliknięciu. */
  onTileDoubleClick: ((tile: { x: number; y: number } | null) => void) | null = null;
  /** Kliknięcie ikony budynku na mapie: prowincja, budynek i punkt ekranu. */
  onBuildingClick: ((hit: { province: number; building: SlotBuilding }, screen: { x: number; y: number }) => void) | null = null;
  /** Kliknięcie prawym przyciskiem w kafel (bez przeciągania): kafel i punkt ekranu – menu kołowe. */
  onContextClick: ((tile: { x: number; y: number } | null, screen: { x: number; y: number }) => void) | null = null;
  /** Narysowano yeti w enklawach (liczba) – etap „chowanie easter eggów”. */
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
      this.live.view,
      this.territory.flags,
      this.inland.view,
      this.provinceLayer,
      this.creatures.view,
      this.highlight.view,
      this.markers.view,
      this.buildings.view,
      this.waves.view,
      this.names.view,
      this.chunkGrid,
    );
    this.app.ticker.add((ticker) => {
      this.runTweens(ticker.deltaMS);
      if (this.live.view.visible) this.live.update();
      this.buildings.update(this.world.scale.x);
      this.provinceLayer.alpha = this.borderOpacity * this.provincesFade;
      if (this.names.view.visible) this.names.update(this.world.scale.x);
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
    const sizeChanged = this.map?.width !== map.width || this.map?.height !== map.height;
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
      this.markers.setMap(this.map, this.resourceFilter);
      this.waves.setMap(map);
      this.inland.setMap(map);
      this.drawChunkGrid(map);
      this.live.setMap(map);
      this.live.setOwners(this.owners, false);
      void this.paintOwners();
      if (sizeChanged) this.fit();
      // Znacznik do pomiarów czasu wczytania (DevTools → Performance, testy obciążeniowe).
      performance.mark('map-rendered', { detail: { generateMs: map.ms, width: map.width, height: map.height } });
    }, first ? 600 : CROSSFADE_MS);
  }

  /** Faza 2 tej samej mapy: przychodzą prowincje – przebudowa tylko zależnych od nich warstw. */
  private addProvinces(map: MapPayload): void {
    // Ostatnie szlify tej samej mapy bez enklaw: prowincje bez zmian, dochodzą tylko tunele.
    // Enklawy zmieniają `province` – wtedy (albo gdy obie fazy przyszły naraz, jedną zmianą
    // sygnału) granice budują się od nowa.
    const sameProvinces = map.province === this.map?.province;
    this.map = map;
    this.painter.setProvinces(map);
    if (this.mapShown) {
      this.showCreatures(map);
      this.markers.setMap(map, this.resourceFilter);
      this.live.setProvinces(map.province);
    }
    if (!sameProvinces && this.mapShown) void this.buildProvinces(map);
    // Widoki płaskie zależą od prowincji: polityczny od numeracji, zasobów także od zasobów
    // (przydziela je faza 3, nawet gdy numeracja się nie zmienia) – namaluj je od nowa.
    this.repaintViews(sameProvinces ? ['resources'] : ['political', 'resources']);
  }

  /** Wyrzuca z pamięci podane widoki (poza właśnie pokazanym) i maluje bieżący, jeśli jest wśród nich. */
  private repaintViews(views: TerrainView[]): void {
    for (const [key, layers] of this.cache) {
      if (views.some((v) => key.startsWith(`${v}|`)) && layers !== this.shown) this.dropCached(key);
    }
    if (views.includes(this.view)) {
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

  /** Zasoby zaznaczone w legendzie; mapa zasobów maluje się od nowa (albo bierze z pamięci). */
  setResourceFilter(enabled: readonly boolean[]): void {
    this.resourceFilter = enabled;
    if (this.mapShown && this.map) this.markers.setMap(this.map, enabled);
    if (this.view === 'resources' && this.ready && this.map) void this.showView(undefined, FILTER_FADE_MS);
  }

  /**
   * Właściciele kafli z gry (państwo na kafel; null = brak gry albo nikt nic nie ma). Nakładkę
   * maluje worker malujący – tylko po zmianie właścicieli, widać ją na mapie politycznej.
   */
  setOwners(tiles: Uint8Array | null): void {
    if (tiles === this.owners) return;
    this.owners = tiles;
    if (this.mapShown && (!tiles || tiles.length === (this.map?.width ?? 0) * (this.map?.height ?? 0))) this.live.setOwners(tiles);
    void this.paintOwners();
  }

  /** Zmiany kafli z tury (`kafel << 8 | państwo`) – front rysuje się od razu, płynnie. */
  applyOwnerChanges(changes: Uint32Array): void {
    if (this.mapShown && this.owners) this.live.apply(changes);
  }

  /** Przewidywany start gracza (od razu po kliknięciu, zanim wróci tura) – `TerritoryLayer.preview`. */
  previewStart(province: number, previous: number, nation: number): void {
    // Także zanim ktokolwiek ma prowincję (`owners` jeszcze puste) – warstwa zna już mapę.
    if (this.mapShown) this.live.preview(province, previous, nation);
  }

  /** Trwające budowy na mapie (`BuildingLayer.setConstructions`). */
  setConstructions(slots: Uint16Array | null, data: Uint8Array | null): void {
    this.buildings.setConstructions(slots, data);
  }

  /** Widoczność budynków na mapie: wszystkie (`show`) i rodzaje (`filter`). */
  setBuildingFilter(show: boolean, filter: BuildingFilter): void {
    this.buildings.setFilter(show ? filter : { city: false, port: false, extraction: false, factory: false, university: false, command: false, construction: false });
  }

  /** Budynki na mapie: stałe miejsca i poziomy z gry (null – bez gry). */
  setBuildings(slots: Uint16Array | null, levels: Uint8Array | null): void {
    this.buildings.set(slots, levels);
  }

  /** Nazwy państw (numer państwa → nazwa gracza). */
  setNationNames(names: Map<number, string>): void {
    this.nationNames = names;
    this.names.setNames(names);
  }

  /** Wygląd terytoriów (opcje pod zębatką i w panelu debugu). */
  setTerritoryStyle(style: TerritoryStyle): void {
    const repaint = style.borderWidth !== this.territoryStyle.borderWidth;
    this.territoryStyle = style;
    this.names.setOpacity(style.namesOpacity);
    this.applyVisibility();
    if (repaint) void this.paintOwners();
  }

  private async paintOwners(): Promise<void> {
    const request = ++this.ownersRequest;
    const { map, owners } = this;
    if (!this.ready || !this.mapShown || !map || owners?.length !== map.width * map.height) {
      this.clearOwners();
      return;
    }
    const epoch = this.paintEpoch;
    const r = await this.painter.territories(owners, this.territoryStyle.borderWidth);
    if (request !== this.ownersRequest || epoch !== this.paintEpoch || this.map !== map) return;
    this.clearOwners();
    for (const key of TERRITORY_LAYERS) this.territory[key].addChild(this.textureLayer(map, r[key]));
    this.names.set(r.labels, this.nationNames);
  }

  private clearOwners(): void {
    for (const key of TERRITORY_LAYERS) {
      for (const old of this.territory[key].removeChildren()) old.destroy({ children: true, texture: true, textureSource: true });
    }
    this.names.set([], this.nationNames);
  }

  /** Podświetlenie prowincji: zaznaczonej (mocniej, z wyraźnym skrajem) i pod kursorem (lekko). */
  setHighlight(selected: number, hovered: number): void {
    this.highlight.set(selected, hovered);
  }

  /** Prowincje zaznaczone na jeden front (Ctrl + klik) – czerwonawe podświetlenie. */
  setTargets(provinces: readonly number[]): void {
    this.highlight.setTargets(provinces);
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

  /** Mapa polityczna i mapa zasobów pokazują tylko prowincje: bez animacji wody. */
  private applyVisibility(): void {
    const flat = isFlatView(this.view);
    this.waves.view.visible = this.wavesVisible && !flat;
    this.inland.view.visible = this.wavesVisible && !flat;
    this.provinceLayer.visible = this.provinces && !flat;
    this.markers.view.visible = this.view === 'resources';
    // Terytoria: na mapie politycznej pełne kolory, na mapie terenu tło (kolor albo flaga) i granice.
    const s = this.territoryStyle;
    const terrain = this.view === 'terrain';
    const t = this.territory;
    t.flags.visible = terrain && s.fill === 'flag' && s.flagOpacity > 0;
    t.flags.alpha = s.flagOpacity;
    this.names.view.visible = s.showNames && (terrain || this.view === 'political');
    this.live.view.visible = terrain || this.view === 'political';
    this.live.setLook({
      political: this.view === 'political',
      fill: s.fill === 'color' ? s.fillOpacity : 0,
      border: s.borderOpacity,
      borderWidth: s.borderWidth,
    });
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

  /**
   * Płynny przelot kamery na kafel (x, y) – środek ekranu; `scale` – docelowe przybliżenie (bez –
   * obecne). Środek i skala zmieniają się razem (skala logarytmicznie, bez „skoku”).
   */
  flyTo(x: number, y: number, scale?: number, ms = 900): void {
    if (!this.ready) return;
    const { width, height } = this.app.screen;
    const s0 = this.world.scale.x;
    const s1 = Math.min(32, Math.max(0.05, scale ?? s0));
    const c0 = { x: (width / 2 - this.world.x) / s0, y: (height / 2 - this.world.y) / s0 };
    this.tween(ms, (k) => {
      const s = Math.exp(Math.log(s0) + (Math.log(s1) - Math.log(s0)) * k);
      const cx = c0.x + (x + 0.5 - c0.x) * k;
      const cy = c0.y + (y + 0.5 - c0.y) * k;
      this.world.scale.set(s);
      this.world.position.set(width / 2 - cx * s, height / 2 - cy * s);
    });
  }

  /** Skala, przy której prowincja wielkości `tiles` kafli zajmuje mniej więcej `share` wysokości ekranu. */
  scaleFor(tiles: number, share = 0.35): number {
    return (this.app.screen.height * share) / Math.max(1, Math.sqrt(tiles));
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
    this.live.destroy();
    this.buildings.destroy();
    this.names.destroy();
    this.creatures.destroy();
    this.markers.destroy();
    this.painter.destroy();
    this.app.destroy(true, { children: true, texture: true, textureSource: true });
  }

  private viewKey(): string {
    // Mapa zasobów zależy od filtra – każdy filtr to osobny widok w pamięci (szybki powrót).
    const filter = this.view === 'resources' ? `|${this.resourceFilter.map(Number).join('')}` : '';
    return `${this.view}|${this.contours}${filter}`;
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
      const r = await this.painter.terrain(this.view, this.contours, this.resourceFilter);
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

  /** Rozmiar „piksela” yeti w kaflach – przerysowuje yeti na bieżącej mapie. */
  setCreaturePixel(pixel: number): void {
    if (pixel === this.creatures.pixel) return;
    this.creatures.pixel = pixel;
    if (this.mapShown && this.map?.polished) this.creatures.setMap(this.map);
  }

  /** Yeti w enklawach – dopiero gdy ostatnie szlify są gotowe. */
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

  /** Przybliża do skali `next` (w granicach), trzymając punkt ekranu (mx, my) nad tym samym miejscem mapy. */
  private zoomAt(mx: number, my: number, next: number): void {
    const old = this.world.scale.x;
    next = Math.min(32, Math.max(0.05, next));
    this.world.x = mx - ((mx - this.world.x) * next) / old;
    this.world.y = my - ((my - this.world.y) * next) / old;
    this.world.scale.set(next);
  }

  /**
   * Przeciąganie przesuwa mapę, kółko przybliża względem kursora.
   * Dotyk: jeden palec przesuwa, dwa palce przybliżają (pinch) i przesuwają środkiem.
   */
  private bindCamera(canvas: HTMLCanvasElement): void {
    // Wciśnięte wskaźniki (mysz, palce) – ostatnia pozycja każdego.
    const pointers = new Map<number, { x: number; y: number }>();
    // Łączne przesunięcie od wciśnięcia – do odróżnienia kliknięcia od przeciągania.
    let dragged = 0;
    // Gest dwoma palcami nie kończy się kliknięciem, nawet gdy palce prawie stały.
    let multi = false;
    /** Środek i rozstaw wciśniętych wskaźników (rozstaw 0 dla jednego). */
    const gesture = () => {
      const [a, b] = [...pointers.values()];
      if (!b) return { x: a.x, y: a.y, span: 0 };
      return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2, span: Math.hypot(a.x - b.x, a.y - b.y) };
    };
    const listen = <K extends keyof HTMLElementEventMap>(
      type: K,
      fn: (e: HTMLElementEventMap[K]) => void,
      options?: AddEventListenerOptions,
    ) => {
      canvas.addEventListener(type, fn, options);
      this.cleanup.push(() => canvas.removeEventListener(type, fn, options));
    };

    listen('pointerdown', (e) => {
      if (pointers.size === 0) {
        dragged = 0;
        multi = false;
      }
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (pointers.size > 1) multi = true;
      canvas.setPointerCapture(e.pointerId);
    });
    listen('pointermove', (e) => {
      if (!pointers.has(e.pointerId)) {
        if (e.pointerType === 'mouse') this.hover(e, canvas);
        return;
      }
      const before = gesture();
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      const after = gesture();
      const rect = canvas.getBoundingClientRect();
      this.world.x += after.x - before.x;
      this.world.y += after.y - before.y;
      if (before.span > 0 && after.span > 0) {
        this.zoomAt(after.x - rect.left, after.y - rect.top, (this.world.scale.x * after.span) / before.span);
      }
      dragged += Math.abs(after.x - before.x) + Math.abs(after.y - before.y);
    });
    const release = (e: PointerEvent) => {
      if (!pointers.delete(e.pointerId)) return;
      // Dotyk jest grubszy niż kursor – większy próg przeciągnięcia.
      const threshold = e.pointerType === 'mouse' ? 5 : 12;
      if (e.type === 'pointerup' && pointers.size === 0 && !multi && dragged < threshold) {
        const tile = this.tileAt(e, canvas);
        if (e.button === 2) {
          this.onContextClick?.(tile, { x: e.clientX, y: e.clientY });
          return;
        }
        // Ikona budynku ma pierwszeństwo przed prowincją pod nią.
        const rect = canvas.getBoundingClientRect();
        const wx = (e.clientX - rect.left - this.world.x) / this.world.scale.x;
        const wy = (e.clientY - rect.top - this.world.y) / this.world.scale.y;
        const building = this.buildings.hit(wx, wy);
        if (building) {
          this.onBuildingClick?.(building, { x: e.clientX, y: e.clientY });
          return;
        }
        // Na dotyku nie ma najechania – ramka prowincji pokazuje się po stuknięciu.
        if (e.pointerType !== 'mouse') this.onHover?.(tile);
        this.onTileClick?.(tile, { ctrl: e.ctrlKey || e.metaKey });
        const now = performance.now();
        const last = this.lastClick;
        if (last && now - last.at < DOUBLE_CLICK_MS && Math.abs(e.clientX - last.x) + Math.abs(e.clientY - last.y) < 8) {
          this.lastClick = null;
          this.onTileDoubleClick?.(tile);
        } else {
          this.lastClick = { at: now, x: e.clientX, y: e.clientY };
        }
      }
    };
    listen('pointerup', release);
    // Prawy przycisk – menu kołowe gry zamiast menu przeglądarki.
    listen('contextmenu', (e) => e.preventDefault());
    listen('pointercancel', release);
    listen('pointerleave', (e) => {
      if (e.pointerType === 'mouse') this.onHover?.(null);
    });
    listen(
      'wheel',
      (e) => {
        e.preventDefault();
        const rect = canvas.getBoundingClientRect();
        this.zoomAt(e.clientX - rect.left, e.clientY - rect.top, this.world.scale.x * Math.exp(-e.deltaY * 0.0015));
      },
      { passive: false },
    );
  }
}
