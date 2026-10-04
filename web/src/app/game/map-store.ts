import { Injectable, computed, inject, signal } from '@angular/core';

import type { MapGenParams } from '../../generated/MapGenParams';
import type { TerrainView } from '../render/terrain';
import type { WaveSettings } from '../render/waves';
import { sameParams, type MapPayload } from '../worker/protocol';
import { WorkerBridge } from './worker-bridge';

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
  readonly busy = signal(false);
  /** Renderer maluje widok (worker) – np. po zmianie rodzaju mapy. */
  readonly painting = signal(false);
  readonly error = signal<string | null>(null);
  readonly generatorVersion = signal(0);
  /** Siatka chunków – narzędzie deweloperskie (klawisz C w panelu), domyślnie wyłączona. */
  readonly showChunkGrid = signal(false);
  /** Rodzaj mapy: teren, polityczna (same prowincje) albo biomy. */
  readonly view = signal<TerrainView>('terrain');
  /** Nakładka z granicami prowincji. */
  readonly showProvinces = signal(true);
  /** Krycie granic prowincji na mapie terenu (0..1) – teren pod granicą pozostaje widoczny. */
  readonly borderOpacity = signal(0.3);
  /** Zaznaczona prowincja (kliknięcie; numer od 1, 0 = brak). */
  readonly selectedProvince = signal(0);
  /** Prowincja pod kursorem (0 = brak). */
  readonly hoveredProvince = signal(0);
  /** Kursor nad górami (kafel lądu lub rzeki bez prowincji – nieprzechodni, niczyj). */
  readonly hoveredMountain = signal(false);
  /** Izobaty na oceanie. */
  readonly showContours = signal(true);
  /** Animacja fal – domyślnie wyłączona, gdy system prosi o ograniczenie ruchu. */
  readonly showWaves = signal(!globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches);
  readonly waves = signal<WaveSettings>({ shore: 0.8, inland: 0.8, speed: 1 });
  /** Każda zmiana = prośba o dopasowanie kamery do mapy. */
  readonly fitRequest = signal(0);

  /** Wczytuje domyślne parametry (dla panelu i trybu bez serwera). Nie generuje – mapę zleca `GameSession`. */
  init(): Promise<void> {
    this.defaultsLoaded ??= this.bridge.defaults().then(({ params, generatorVersion }) => {
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
    this.update({ seed: Math.floor(Math.random() * 2 ** 32) });
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
      this.hoveredMountain.set(false);
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
