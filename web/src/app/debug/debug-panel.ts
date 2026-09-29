import { Component, inject, signal } from '@angular/core';

import type { MapGenParams } from '../../generated/MapGenParams';
import { MapStore } from '../game/map-store';
import { Transport } from '../game/transport';
import { BIOMES } from '../render/terrain';

type KeysOfType<T, V> = { [K in keyof T]: T[K] extends V ? K : never }[keyof T];
type NumberKey = KeysOfType<MapGenParams, number>;
type FlagKey = KeysOfType<MapGenParams, boolean>;

type Field =
  | { kind: 'range'; key: NumberKey; label: string; min: number; max: number; step: number; enabledBy?: FlagKey }
  | { kind: 'toggle'; key: FlagKey; label: string };

const GROUPS: { title: string; fields: Field[] }[] = [
  {
    title: 'Rozmiar i chunki',
    fields: [
      { kind: 'range', key: 'width', label: 'Szerokość', min: 400, max: 3200, step: 100 },
      { kind: 'range', key: 'height', label: 'Wysokość', min: 200, max: 1800, step: 100 },
      { kind: 'range', key: 'chunkCols', label: 'Chunki w poziomie', min: 1, max: 16, step: 1 },
      { kind: 'range', key: 'chunkRows', label: 'Chunki w pionie', min: 1, max: 10, step: 1 },
    ],
  },
  {
    title: 'Kontynenty',
    fields: [
      { kind: 'range', key: 'continents', label: 'Liczba kontynentów', min: 1, max: 8, step: 1 },
      { kind: 'range', key: 'landRatio', label: 'Udział chunków lądowych', min: 0.1, max: 1, step: 0.05 },
      { kind: 'range', key: 'sizeVariance', label: 'Różnice wielkości', min: 0, max: 1, step: 0.05 },
      { kind: 'range', key: 'minIslandArea', label: 'Najmniejsza wyspa (kafle)', min: 0, max: 5000, step: 50 },
    ],
  },
  {
    title: 'Wybrzeże',
    fields: [
      { kind: 'range', key: 'coastRoughness', label: 'Poszarpanie', min: 0, max: 1, step: 0.05 },
      { kind: 'toggle', key: 'keepOffEdges', label: 'Ląd z dala od krawędzi mapy i chunków wodnych' },
      { kind: 'range', key: 'edgeMargin', label: 'Odstęp (kafle)', min: 0, max: 60, step: 1, enabledBy: 'keepOffEdges' },
    ],
  },
  {
    title: 'Góry',
    fields: [
      { kind: 'range', key: 'mountainShare', label: 'Udział gór', min: 0, max: 0.3, step: 0.01 },
      { kind: 'range', key: 'highlandShare', label: 'Udział wyżyn', min: 0, max: 0.5, step: 0.01 },
      { kind: 'range', key: 'rangeScale', label: 'Skala pasm', min: 0.3, max: 3, step: 0.1 },
    ],
  },
  {
    title: 'Woda',
    fields: [
      { kind: 'toggle', key: 'rivers', label: 'Rzeki ze szczytów' },
      { kind: 'range', key: 'riverCount', label: 'Liczba źródeł', min: 1, max: 100, step: 1, enabledBy: 'rivers' },
      { kind: 'toggle', key: 'lakes', label: 'Jeziora' },
      { kind: 'range', key: 'lakeAmount', label: 'Ilość pojezierzy', min: 0, max: 1, step: 0.05, enabledBy: 'lakes' },
      { kind: 'range', key: 'minLakeArea', label: 'Najmniejsze jezioro', min: 10, max: 2000, step: 10, enabledBy: 'lakes' },
      { kind: 'range', key: 'maxLakeArea', label: 'Największe jezioro', min: 100, max: 20000, step: 100, enabledBy: 'lakes' },
    ],
  },
  {
    title: 'Biomy',
    fields: [
      { kind: 'toggle', key: 'biomes', label: 'Biomy kontynentów' },
      { kind: 'range', key: 'biomeTemperate', label: 'Umiarkowany – częstość', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeDesert', label: 'Pustynny – częstość', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeCold', label: 'Zimny – częstość', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeHumid', label: 'Wilgotny – częstość', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeSteppe', label: 'Step – częstość', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeLatitude', label: 'Wpływ szerokości geogr.', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeMixChance', label: 'Szansa na dwa biomy', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeSecondaryShare', label: 'Udział drugiego biomu', min: 0.05, max: 0.5, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeTransition', label: 'Szerokość przejścia (kafle)', min: 4, max: 300, step: 2, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeRoughness', label: 'Pofalowanie granicy', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
    ],
  },
];

/** Panel deweloperski. Ładowany dynamicznie tylko gdy DEV_TOOLS = true. */
@Component({
  selector: 'app-debug-panel',
  templateUrl: './debug-panel.html',
  styleUrl: './debug-panel.css',
  host: { '(window:keydown)': 'onKey($event)' },
})
export class DebugPanel {
  protected readonly store = inject(MapStore);
  protected readonly transport = inject(Transport);
  protected readonly groups = GROUPS;
  protected readonly biomes = BIOMES;
  protected readonly collapsed = signal(false);
  protected readonly autoGenerate = signal(true);

  protected setNumber(key: NumberKey, event: Event): void {
    this.store.update({ [key]: Number((event.target as HTMLInputElement).value) });
  }

  protected setFlag(key: FlagKey, event: Event): void {
    this.store.update({ [key]: (event.target as HTMLInputElement).checked });
    this.commit();
  }

  protected setSeed(event: Event): void {
    const value = Math.trunc(Number((event.target as HTMLInputElement).value)) >>> 0;
    this.store.update({ seed: value });
    this.commit();
  }

  /** Suwaki generują dopiero po puszczeniu (change), nie przy każdym ruchu (input). */
  protected commit(): void {
    if (this.autoGenerate()) void this.store.generate();
  }

  protected newSeed(): void {
    this.store.randomizeSeed();
    void this.store.generate();
  }

  protected isDisabled(params: MapGenParams, field: Field): boolean {
    return field.kind === 'range' && !!field.enabledBy && !params[field.enabledBy];
  }

  protected format(value: number, step: number): string {
    return step < 1 ? value.toFixed(2) : String(value);
  }

  protected percent(value: number): string {
    return `${(value * 100).toFixed(1)}%`;
  }

  protected swatch(color: readonly number[]): string {
    return `rgb(${color.join(' ')})`;
  }

  protected hex(value: number): string {
    return value.toString(16).padStart(8, '0');
  }

  protected checked(event: Event): boolean {
    return (event.target as HTMLInputElement).checked;
  }

  protected onKey(event: KeyboardEvent): void {
    const target = event.target as HTMLElement;
    if (target instanceof HTMLInputElement && (target.type === 'number' || target.type === 'text')) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;

    switch (event.code) {
      case 'KeyG':
        void this.store.generate();
        break;
      case 'KeyN':
        this.newSeed();
        break;
      case 'KeyF':
        this.store.requestFit();
        break;
      case 'KeyC':
        this.store.showChunkGrid.update((v) => !v);
        break;
      case 'KeyB':
        this.store.showBiomeMap.update((v) => !v);
        break;
      case 'Backquote':
        this.collapsed.update((v) => !v);
        break;
      default:
        return;
    }
    event.preventDefault();
  }
}
