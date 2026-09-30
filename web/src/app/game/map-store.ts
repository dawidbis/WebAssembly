import { Injectable, inject, signal } from '@angular/core';

import type { MapGenParams } from '../../generated/MapGenParams';
import type { TerrainView } from '../render/terrain';
import type { WaveSettings } from '../render/waves';
import type { MapPayload } from '../worker/protocol';
import { WorkerBridge } from './worker-bridge';

/** Stan mapy dla UI: parametry, ostatni wynik, flagi widoku. */
@Injectable({ providedIn: 'root' })
export class MapStore {
  private readonly bridge = inject(WorkerBridge);
  private queued = false;

  readonly params = signal<MapGenParams | null>(null);
  readonly map = signal<MapPayload | null>(null);
  readonly busy = signal(false);
  readonly error = signal<string | null>(null);
  readonly generatorVersion = signal(0);
  readonly showChunkGrid = signal(true);
  /** Rodzaj mapy: teren, polityczna (same prowincje), biomy albo żyzność. */
  readonly view = signal<TerrainView>('terrain');
  /** Nakładka z granicami prowincji. */
  readonly showProvinces = signal(true);
  /** Krycie granic prowincji na mapie terenu (0..1) – teren pod granicą pozostaje widoczny. */
  readonly borderOpacity = signal(0.3);
  /** Zaznaczona prowincja (kliknięcie; numer od 1, 0 = brak). */
  readonly selectedProvince = signal(0);
  /** Prowincja pod kursorem (0 = brak). */
  readonly hoveredProvince = signal(0);
  /** Symbole drzew przy przybliżeniu. */
  readonly showTrees = signal(true);
  /** Izobaty na oceanie. */
  readonly showContours = signal(true);
  /** Animacja fal – domyślnie wyłączona, gdy system prosi o ograniczenie ruchu. */
  readonly showWaves = signal(!globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches);
  readonly waves = signal<WaveSettings>({ shore: 0.8, inland: 0.8, speed: 1 });
  /** Każda zmiana = prośba o dopasowanie kamery do mapy. */
  readonly fitRequest = signal(0);

  async init(): Promise<void> {
    const { params, generatorVersion } = await this.bridge.defaults();
    this.params.set(params);
    this.generatorVersion.set(generatorVersion);
    await this.generate();
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
    try {
      this.map.set(await this.bridge.generateMap(params));
      this.selectedProvince.set(0);
      this.hoveredProvince.set(0);
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
