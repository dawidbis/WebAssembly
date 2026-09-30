import { Component, inject, signal } from '@angular/core';

import type { MapGenParams } from '../../generated/MapGenParams';
import { MapStore } from '../game/map-store';
import { Transport } from '../game/transport';
import { BIOMES, BIOME_PAIRS } from '../render/terrain';
import type { WaveSettings } from '../render/waves';

type KeysOfType<T, V> = { [K in keyof T]: T[K] extends V ? K : never }[keyof T];
type NumberKey = KeysOfType<MapGenParams, number>;
type FlagKey = KeysOfType<MapGenParams, boolean>;

type Field =
  | {
      kind: 'range';
      key: NumberKey;
      label: string;
      min: number;
      max: number;
      step: number;
      enabledBy?: FlagKey;
      /** Własny opis wartości zamiast liczby z suwaka. */
      show?: (p: MapGenParams) => string;
    }
  | { kind: 'toggle'; key: FlagKey; label: string }
  | { kind: 'mask'; key: NumberKey; label: string; options: { bit: number; label: string }[]; enabledBy?: FlagKey };

/** Wagi szans biomów w kolejności `Biome` (ta sama co `BIOMES`). */
const CHANCE_KEYS = ['biomeTemperate', 'biomeDesert', 'biomeCold', 'biomeHumid', 'biomeSteppe'] as const satisfies readonly NumberKey[];

/** Waga biomu jako rzeczywista szansa: udział w sumie wag wszystkich biomów. */
function chance(key: (typeof CHANCE_KEYS)[number]): (p: MapGenParams) => string {
  return (p) => {
    const sum = CHANCE_KEYS.reduce((s, k) => s + p[k], 0);
    return `${sum > 0 ? Math.round((p[key] / sum) * 100) : 0}%`;
  };
}

const GROUPS: { title: string; hint?: string; fields: Field[] }[] = [
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
    title: 'Ocean',
    fields: [
      { kind: 'range', key: 'shelfWidth', label: 'Szerokość szelfu (kafle)', min: 1, max: 60, step: 1 },
      { kind: 'range', key: 'shelfVariation', label: 'Zmienność szelfu', min: 0, max: 1, step: 0.05 },
      { kind: 'range', key: 'slopeSteepness', label: 'Stromość stoku', min: 0, max: 1, step: 0.05 },
      { kind: 'range', key: 'seabedRelief', label: 'Rzeźba dna', min: 0, max: 1, step: 0.05 },
    ],
  },
  {
    title: 'Lasy i żyzność',
    hint: 'Typ lasu wynika z biomu: liściasty, tajga, dżungla, zagajniki stepowe, oazy. Udział liczony wśród kafli bez gór.',
    fields: [
      { kind: 'toggle', key: 'forest', label: 'Lasy' },
      { kind: 'range', key: 'forestTemperate', label: 'Udział lasu: Umiarkowany', min: 0, max: 1, step: 0.05, enabledBy: 'forest' },
      { kind: 'range', key: 'forestCold', label: 'Udział lasu: Zimny (tajga)', min: 0, max: 1, step: 0.05, enabledBy: 'forest' },
      { kind: 'range', key: 'forestHumid', label: 'Udział lasu: Wilgotny (dżungla)', min: 0, max: 1, step: 0.05, enabledBy: 'forest' },
      { kind: 'range', key: 'forestSteppe', label: 'Udział lasu: Step', min: 0, max: 1, step: 0.01, enabledBy: 'forest' },
      { kind: 'range', key: 'forestDesert', label: 'Udział lasu: Pustynny (oazy)', min: 0, max: 0.3, step: 0.01, enabledBy: 'forest' },
      { kind: 'range', key: 'forestClumping', label: 'Zwartość masywów', min: 0, max: 1, step: 0.05, enabledBy: 'forest' },
      { kind: 'range', key: 'forestMoisture', label: 'Przyciąganie do wody', min: 0, max: 1, step: 0.05, enabledBy: 'forest' },
      { kind: 'range', key: 'fertilityRiverBonus', label: 'Żyzność: bonus brzegów rzek i jezior', min: 0, max: 1, step: 0.05 },
      { kind: 'range', key: 'fertilityRiverReach', label: 'Żyzność: szerokość pasa brzegów (kafle)', min: 1, max: 30, step: 1 },
    ],
  },
  {
    title: 'Prowincje',
    hint: 'Każda prowincja ma podobną wartość: suma wartości kafli z żyzności (od minimum dla jałowej ziemi do 1). Góry są niczyje i nieprzechodnie.',
    fields: [
      { kind: 'toggle', key: 'provinces', label: 'Prowincje' },
      { kind: 'range', key: 'provinceValue', label: 'Średnia wartość prowincji', min: 50, max: 3000, step: 25, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceValueFloor', label: 'Wartość jałowego kafla (i rzeki)', min: 0.05, max: 1, step: 0.05, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceMinSize', label: 'Najmniejsza prowincja (kafle)', min: 10, max: 3000, step: 10, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceMaxSize', label: 'Największa prowincja (kafle)', min: 200, max: 30000, step: 100, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceNaturalBorders', label: 'Granice na rzekach i graniach', min: 0, max: 1, step: 0.05, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceRoughness', label: 'Nieregularność granic', min: 0, max: 1, step: 0.05, enabledBy: 'provinces' },
    ],
  },
  {
    title: 'Biomy',
    hint: 'Szansa = udział biomu w losowaniu dla kontynentu. Wpływ szerokości geogr. przesuwa szanse: bliżej biegunów zimniej, przy równiku cieplej.',
    fields: [
      { kind: 'toggle', key: 'biomes', label: 'Biomy kontynentów' },
      ...CHANCE_KEYS.map(
        (key, i): Field => ({
          kind: 'range',
          key,
          label: `Szansa: ${BIOMES[i].name}`,
          min: 0,
          max: 1,
          step: 0.05,
          enabledBy: 'biomes',
          show: chance(key),
        }),
      ),
      { kind: 'range', key: 'biomeLatitude', label: 'Wpływ szerokości geogr.', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeMixChance', label: 'Szansa na dwa biomy', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      {
        kind: 'mask',
        key: 'biomePairs',
        label: 'Dozwolone pary na jednym kontynencie',
        options: BIOME_PAIRS.map(([a, b], bit) => ({ bit, label: `${BIOMES[a].name} + ${BIOMES[b].name}` })),
        enabledBy: 'biomes',
      },
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

  protected setWave(key: keyof WaveSettings, event: Event): void {
    const value = Number((event.target as HTMLInputElement).value);
    this.store.waves.update((w) => ({ ...w, [key]: value }));
  }

  protected setBit(key: NumberKey, bit: number, event: Event): void {
    const params = this.store.params();
    if (!params) return;
    const on = (event.target as HTMLInputElement).checked;
    this.store.update({ [key]: on ? params[key] | (1 << bit) : params[key] & ~(1 << bit) });
    this.commit();
  }

  protected hasBit(value: number, bit: number): boolean {
    return (value & (1 << bit)) !== 0;
  }

  protected isDisabled(params: MapGenParams, field: Field): boolean {
    return field.kind !== 'toggle' && !!field.enabledBy && !params[field.enabledBy];
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
      case 'KeyC':
        this.store.showChunkGrid.update((v) => !v);
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
