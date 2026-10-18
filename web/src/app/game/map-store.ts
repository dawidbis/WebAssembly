import { Injectable, computed, inject, signal } from '@angular/core';

import type { MapGenParams } from '../../generated/MapGenParams';
import type { SupplyRule } from '../../generated/SupplyRule';
import type { BuildingFilter, BuildingKind, SlotBuilding } from '../render/buildings';
import { DEFAULT_TERRITORY_STYLE, type TerritoryStyle } from '../render/map-renderer';
import { DEFAULT_LANDING_STYLE, type LandingStyle } from '../render/landings';
import type { TerrainView } from '../render/terrain';
import { YETI_PIXEL } from '../render/creatures';
import { RESOURCES } from '../render/resources';
import type { WaveSettings } from '../render/waves';
import { sameParams, type MapPayload } from '../worker/protocol';
import { WorkerBridge } from './worker-bridge';

/** Rodzaj lądu niczyjego: góry, lodowiec, enklawa (odcięta dolina z yeti). */
export type Wasteland = 'mountains' | 'glacier' | 'enclave';

/** Stan mapy dla UI: parametry, ostatni wynik, flagi widoku. */
@Injectable({ providedIn: 'root' })
export class MapStore {
  private readonly bridge = inject(WorkerBridge);
  private queued = false;
  /** Numer ostatniego generowania – prowincje starszej mapy są ignorowane. */
  private request = 0;
  /** Parametry ostatnio zleconej mapy (w trakcie generowania albo gotowej). */
  private requested: MapGenParams | null = null;
  private defaultsLoaded?: Promise<void>;

  readonly params = signal<MapGenParams | null>(null);
  readonly map = signal<MapPayload | null>(null);
  /** Teren jest już na ekranie, prowincje jeszcze się liczą. */
  readonly provincesPending = computed(() => {
    const map = this.map();
    return !!map && !map.provincesReady;
  });
  /** Prowincje są, trwają ostatnie szlify (tunele do odciętych prowincji, enklawy). */
  readonly polishPending = computed(() => {
    const map = this.map();
    return !!map?.provincesReady && !map.polished;
  });
  /** Renderer rysuje yeti w enklawach („chowanie easter eggów”). */
  readonly eggsPending = signal(false);
  /** Prowincje można zaznaczać dopiero po ostatnich szlifach (enklawy mogą zmienić numerację). */
  readonly selectable = computed(() => !!this.map()?.polished);
  readonly busy = signal(false);
  /** Renderer maluje widok (worker) – np. po zmianie rodzaju mapy. */
  readonly painting = signal(false);
  readonly error = signal<string | null>(null);
  readonly generatorVersion = signal(0);
  /** Receptury zaopatrzenia z rdzenia (drzewko w oknie zapasów). */
  readonly supplyRules = signal<SupplyRule[]>([]);
  /** Siatka chunków – narzędzie deweloperskie (klawisz C w panelu), domyślnie wyłączona. */
  readonly showChunkGrid = signal(false);
  /** Rodzaj mapy: teren, polityczna (same prowincje), biomy albo zasoby. */
  readonly view = signal<TerrainView>('terrain');
  /** Zasoby pokazane na mapie zasobów (indeks = numer zasobu); odznaczone są szare, bez obwódki. */
  readonly resourceFilter = signal<readonly boolean[]>(RESOURCES.map(() => true));
  /** Nakładka z granicami prowincji. */
  readonly showProvinces = signal(true);
  /** Krycie granic prowincji na mapie terenu (0..1) – teren pod granicą pozostaje widoczny. */
  readonly borderOpacity = signal(0.3);
  /** Zaznaczona prowincja (kliknięcie; numer od 1, 0 = brak). */
  readonly selectedProvince = signal(0);
  /** Prowincja pod kursorem (0 = brak). */
  readonly hoveredProvince = signal(0);
  /** Ląd niczyj pod kursorem (nieprzechodni, bez prowincji): góry, lodowiec albo enklawa. */
  readonly hoveredWaste = signal<Wasteland | null>(null);
  /** Okno państwa (kliknięcie złota albo populacji w pasku zasobów): zamknięte albo zakładka. */
  readonly nationWindow = signal<'closed' | 'finance' | 'population'>('closed');
  /** Okno zapasów surowców: zamknięte, otwarte albo zminimalizowane (sam pasek tytułu). */
  readonly stockWindow = signal<'closed' | 'open' | 'min'>('closed');
  /** Okno rynku: zamknięte, otwarte albo zminimalizowane. */
  readonly marketWindow = signal<'closed' | 'open' | 'min'>('closed');
  /** Wygląd terytoriów państw: granice, tło (kolor albo flaga), nazwy – opcje pod zębatką. */
  readonly territory = signal<TerritoryStyle>(DEFAULT_TERRITORY_STYLE);
  /** Wygląd płynących desantów na mapie (suwaki w panelu debugu). */
  readonly landingStyle = signal<LandingStyle>(DEFAULT_LANDING_STYLE);
  /** Izobaty na oceanie. */
  readonly showContours = signal(true);
  /** Animacja fal – domyślnie wyłączona, gdy system prosi o ograniczenie ruchu. */
  readonly showWaves = signal(!globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches);
  readonly waves = signal<WaveSettings>({ shore: 0.8, inland: 0.8, speed: 1 });
  /** Rozmiar „piksela” yeti w enklawach (kafle). */
  readonly yetiPixel = signal(YETI_PIXEL);
  /** Każda zmiana = prośba o dopasowanie kamery do mapy. */
  readonly fitRequest = signal(0);
  /** Prośba o płynny przelot kamery na kafel (x, y), `tiles` – wielkość celu do przybliżenia (bez – bez zmiany). */
  readonly flyRequest = signal<{ x: number; y: number; tiles?: number } | null>(null);
  /** Okienko budynku z mapy (kliknięcie ikony): punkt ekranu, prowincja, budynek. */
  readonly buildingPopup = signal<{ x: number; y: number; province: number; building: SlotBuilding } | null>(null);
  /** Menu kołowe (prawy przycisk na prowincji): punkt ekranu i prowincja. */
  readonly contextMenu = signal<{ x: number; y: number; province: number } | null>(null);
  /** Okno budynków zamknięte ręcznie dla tej prowincji (otworzy się znowu po zaznaczeniu innej). */
  readonly buildingsClosedFor = signal(0);
  /** Panel ataku przypięty – widać go także bez zaznaczonej cudzej prowincji. */
  readonly attackPinned = signal(loadFlag('mapa.attackPinned'));
  /** Prowincje na jeden front (Ctrl + klik; zwykły klik – tylko kliknięta). */
  readonly attackTargets = signal<number[]>([]);
  /** Udział wojska w ataku (%) – suwak panelu ataku, też przy dwukliku; zapamiętany w przeglądarce. */
  readonly attackShare = signal(loadShare());

  setAttackShare(percent: number): void {
    this.attackShare.set(percent);
    try {
      localStorage.setItem(SHARE_KEY, String(percent));
    } catch {
      // Bez storage – tylko na tę sesję.
    }
  }
  /** Budynki na mapie (klawisz B) i widoczność rodzajów (Shift + 1…7) – zapamiętane w przeglądarce. */
  readonly showBuildings = signal(loadFlag(SHOW_BUILDINGS_KEY, true));
  readonly buildingFilter = signal<BuildingFilter>(loadJson(BUILDING_FILTER_KEY));

  setShowBuildings(show: boolean): void {
    this.showBuildings.set(show);
    save(SHOW_BUILDINGS_KEY, show ? '1' : '0');
  }

  setBuildingKind(kind: BuildingKind, shown: boolean): void {
    this.buildingFilter.update((f) => ({ ...f, [kind]: shown }));
    save(BUILDING_FILTER_KEY, JSON.stringify(this.buildingFilter()));
  }

  /** Panel frontów: otwarty albo zminimalizowany (sam nagłówek). */
  readonly frontsPanel = signal<'open' | 'min'>('open');

  /** Wczytuje domyślne parametry (dla panelu i trybu bez serwera). Nie generuje – mapę zleca `GameSession`. */
  init(): Promise<void> {
    this.defaultsLoaded ??= this.bridge.defaults().then(({ params, generatorVersion, supplyRules }) => {
      this.supplyRules.set(supplyRules);
      // Parametry z serwera (`show`) mogły przyjść wcześniej – mają pierwszeństwo.
      this.params.update((p) => p ?? params);
      this.generatorVersion.set(generatorVersion);
    });
    return this.defaultsLoaded;
  }

  /**
   * Mapa gry z serwera. Generuje tylko wtedy, gdy ostatnio zlecona mapa (gotowa albo w trakcie
   * generowania) ma inne parametry – ta sama mapa nie jest generowana drugi raz.
   */
  show(params: MapGenParams): void {
    this.params.set(params);
    if (!this.queued && this.requested && sameParams(this.requested, params)) return;
    void this.generate();
  }

  /** Mapa bez serwera: z domyślnych parametrów, o ile żadna mapa nie została jeszcze zlecona. */
  async showDefault(): Promise<void> {
    await this.init();
    if (!this.requested) await this.generate();
  }

  update(patch: Partial<MapGenParams>): void {
    this.params.update((p) => (p ? { ...p, ...patch } : p));
  }

  randomizeSeed(): void {
    this.update({ seed: crypto.getRandomValues(new Uint32Array(1))[0] });
  }

  /** Płynny przelot kamery na kafel; z `tiles` – przybliżenie do obiektu tej wielkości (kafle). */
  flyTo(x: number, y: number, tiles?: number): void {
    this.flyRequest.set({ x, y, tiles });
  }

  requestFit(): void {
    this.fitRequest.update((n) => n + 1);
  }

  /** Generuje mapę. Wywołanie w trakcie generowania zostaje zapamiętane i wykonane po nim. */
  async generate(): Promise<void> {
    const params = this.params();
    if (!params) return;
    if (this.busy()) {
      this.queued = true;
      return;
    }
    this.busy.set(true);
    this.error.set(null);
    this.requested = params;
    try {
      const request = ++this.request;
      this.map.set(
        await this.bridge.generateMap(params, (r) => {
          if (request !== this.request) return;
          if (r instanceof Error) {
            this.error.set(r.message);
            return;
          }
          if (r.type === 'polished') {
            this.map.update(
              (m) =>
                m && {
                  ...m,
                  province: r.province ?? m.province,
                  provinceHash: r.provinceHash ?? m.provinceHash,
                  provinces: r.provinces,
                  enclaves: r.enclaves,
                  stats: r.stats,
                  polished: true,
                },
            );
            return;
          }
          this.map.update((m) =>
            m && {
              ...m,
              province: r.province,
              provinces: r.provinces,
              stats: r.stats,
              provinceHash: r.provinceHash,
              provincesReady: true,
              provincesMs: r.ms,
            },
          );
        }),
      );
      this.selectedProvince.set(0);
      this.hoveredProvince.set(0);
      this.hoveredWaste.set(null);
    } catch (e) {
      this.error.set(e instanceof Error ? e.message : String(e));
    } finally {
      this.busy.set(false);
    }
    if (this.queued) {
      this.queued = false;
      await this.generate();
    }
  }
}

/** Flaga z localStorage (wygoda – brak dostępu = wyłączona). */
/** Zapamiętany udział wojska w ataku. */
const SHARE_KEY = 'mapa.attackShare';
const DEFAULT_SHARE = 50;

function loadShare(): number {
  try {
    const v = Number(localStorage.getItem(SHARE_KEY));
    return v >= 1 && v <= 100 ? v : DEFAULT_SHARE;
  } catch {
    return DEFAULT_SHARE;
  }
}

function loadFlag(key: string, fallback = false): boolean {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : v === '1';
  } catch {
    return fallback;
  }
}

const SHOW_BUILDINGS_KEY = 'mapa.showBuildings';
const BUILDING_FILTER_KEY = 'mapa.buildingFilter';

function loadJson<T extends object>(key: string): T {
  try {
    const v = JSON.parse(localStorage.getItem(key) ?? '{}');
    return v && typeof v === 'object' ? v : ({} as T);
  } catch {
    return {} as T;
  }
}

function save(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Bez storage – tylko na tę sesję.
  }
}
