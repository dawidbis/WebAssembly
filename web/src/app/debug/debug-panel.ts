import { Component, inject, signal } from '@angular/core';

import type { MapGenParams } from '../../generated/MapGenParams';
import { GameSession } from '../game/game-session';
import { MapStore } from '../game/map-store';
import { Transport } from '../game/transport';
import { BIOMES, BIOME_PAIRS, BIOME_TYPES, VARIANTS } from '../render/terrain';
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
  /** Waga wariantu typu – element tablicy `biomeVariants`. */
  | { kind: 'variant'; key: 'biomeVariants'; index: number; label: string; enabledBy?: FlagKey }
  | { kind: 'mask'; key: NumberKey; label: string; options: { bit: number; label: string }[]; enabledBy?: FlagKey };

/** Wagi szans typów klimatu w kolejności `BiomeType` (ta sama co `BIOME_TYPES`). */
const CHANCE_KEYS = ['biomeTropical', 'biomeDry', 'biomeTemperate', 'biomeContinental', 'biomePolar'] as const satisfies readonly NumberKey[];

/** Udziały lasu w kolejności `Biome` (ta sama co `BIOMES`). */
const FOREST_KEYS = [
  'forestRainforest',
  'forestSavanna',
  'forestDesert',
  'forestSteppe',
  'forestMediterranean',
  'forestSubtropical',
  'forestOceanic',
  'forestHotSummer',
  'forestWarmSummer',
  'forestBoreal',
  'forestTaiga',
  'forestTundra',
  'forestIceSheet',
] as const satisfies readonly NumberKey[];

/** Waga typu jako rzeczywista szansa: udział w sumie wag wszystkich typów. */
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
      { kind: 'range', key: 'height', label: 'Wysokość', min: 400, max: 3200, step: 100 },
      { kind: 'range', key: 'chunkCols', label: 'Chunki w poziomie', min: 1, max: 16, step: 1 },
      { kind: 'range', key: 'chunkRows', label: 'Chunki w pionie', min: 1, max: 16, step: 1 },
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
    title: 'Lasy',
    hint: 'Typ lasu wynika z rodzaju biomu (np. dżungla, tajga, zagajniki stepowe, oazy). Udział liczony wśród kafli bez gór.',
    fields: [
      { kind: 'toggle', key: 'forest', label: 'Lasy' },
      ...FOREST_KEYS.map(
        (key, i): Field => ({ kind: 'range', key, label: `Udział lasu: ${BIOMES[i].name}`, min: 0, max: 1, step: 0.01, enabledBy: 'forest' }),
      ),
      { kind: 'range', key: 'forestClumping', label: 'Zwartość masywów', min: 0, max: 1, step: 0.05, enabledBy: 'forest' },
      { kind: 'range', key: 'forestMoisture', label: 'Przyciąganie do wody', min: 0, max: 1, step: 0.05, enabledBy: 'forest' },
    ],
  },
  {
    title: 'Prowincje',
    hint: 'Każda prowincja ma podobną wielkość (liczbę kafli lądu i rzek). Góry są niczyje i nieprzechodnie.',
    fields: [
      { kind: 'toggle', key: 'provinces', label: 'Prowincje' },
      { kind: 'range', key: 'provinceSize', label: 'Średnia wielkość prowincji (kafle)', min: 50, max: 5000, step: 25, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceMinSize', label: 'Najmniejsza prowincja (kafle)', min: 10, max: 3000, step: 10, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceMaxSize', label: 'Największa prowincja (kafle)', min: 200, max: 30000, step: 100, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceNaturalBorders', label: 'Granice na rzekach i graniach', min: 0, max: 1, step: 0.05, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceRoughness', label: 'Nieregularność granic', min: 0, max: 1, step: 0.05, enabledBy: 'provinces' },
      { kind: 'range', key: 'provinceRounds', label: 'Dokładność wyrównania (rundy; więcej = wolniej)', min: 4, max: 40, step: 1, enabledBy: 'provinces' },
    ],
  },
  {
    title: 'Biomy',
    hint: 'Kontynent dostaje typ klimatu (albo dwa); rodzaj biomu wewnątrz typu wynika z chłodu (położenie między biegunami) i suchości (odległość od morza). Szansa = udział typu w losowaniu. Wpływ biegunów: przy biegunie zimna (górna lub dolna krawędź) polarny i kontynentalny, przy biegunie ciepła naprzeciwko tropikalny i suchy. Suchość to odległość od morza (waga – wpływ morza) i wielkoskalowe strefy wilgotności.',
    fields: [
      { kind: 'toggle', key: 'biomes', label: 'Biomy kontynentów' },
      ...CHANCE_KEYS.map(
        (key, i): Field => ({
          kind: 'range',
          key,
          label: `Szansa: ${BIOME_TYPES[i]}`,
          min: 0,
          max: 1,
          step: 0.05,
          enabledBy: 'biomes',
          show: chance(key),
        }),
      ),
      { kind: 'range', key: 'biomeLatitude', label: 'Wpływ biegunów klimatu', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeMixChance', label: 'Szansa na dwa typy', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      {
        kind: 'mask',
        key: 'biomePairs',
        label: 'Dozwolone pary na jednym kontynencie',
        options: BIOME_PAIRS.map(([a, b], bit) => ({ bit, label: `${BIOME_TYPES[a]} + ${BIOME_TYPES[b]}` })),
        enabledBy: 'biomes',
      },
      { kind: 'range', key: 'biomeSecondaryShare', label: 'Udział drugiego typu', min: 0.05, max: 0.5, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeTransition', label: 'Szerokość przejścia typów (kafle)', min: 4, max: 300, step: 2, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeRoughness', label: 'Pofalowanie granicy typów', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeRainforestShare', label: 'Tropikalny: las deszczowy (reszta sawanna)', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeDesertShare', label: 'Suchy: pustynia (reszta step)', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeOceanicShare', label: 'Umiarkowany: oceaniczny', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeMediterraneanShare', label: 'Umiarkowany: śródziemnomorski (z reszty; dalej subtropikalny)', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeHotSummerShare', label: 'Kontynentalny: gorące lato', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeBorealShare', label: 'Kontynentalny: borealny (środek – ciepłe lato)', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomePolarTaigaShare', label: 'Polarny: tajga', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeIceShare', label: 'Polarny: lądolód (środek – tundra)', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeCoastInfluence', label: 'Wpływ odległości od morza na suchość', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeKindTransition', label: 'Szerokość przejścia rodzajów (kafle)', min: 2, max: 200, step: 2, enabledBy: 'biomes' },
      { kind: 'range', key: 'biomeKindRoughness', label: 'Pofalowanie granic rodzajów', min: 0, max: 1, step: 0.05, enabledBy: 'biomes' },
      { kind: 'toggle', key: 'glacier', label: 'Lądolód jako lodowiec (nieprzechodni, bez prowincji)' },
      { kind: 'range', key: 'iceShelfWidth', label: 'Lód morski przy lądolodzie (kafle)', min: 0, max: 40, step: 1, enabledBy: 'biomes' },
    ],
  },
  {
    title: 'Warianty biomów',
    hint: 'Wariant = które rodzaje typu występują w jego obszarze na kontynencie (od jednego do wszystkich). Szansa = udział wariantu w losowaniu w obrębie typu.',
    fields: VARIANTS.map(
      (v, index): Field => ({
        kind: 'variant',
        key: 'biomeVariants',
        index,
        label: `${BIOME_TYPES[v.type]}: ${v.kinds.map((k) => BIOMES[k].name.split(' – ')[1]).join(' + ')}`,
        enabledBy: 'biomes',
      }),
    ),
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
  protected readonly session = inject(GameSession);
  protected readonly groups = GROUPS;
  protected readonly biomes = BIOMES;
  /** Ustawienia generatora są schowane; Esc je otwiera i zamyka. */
  protected readonly collapsed = signal(true);
  protected readonly autoGenerate = signal(true);

  protected setNumber(key: NumberKey, event: Event): void {
    this.store.update({ [key]: Number((event.target as HTMLInputElement).value) });
  }

  protected setVariant(index: number, event: Event): void {
    const params = this.store.params();
    if (!params) return;
    const next = [...params.biomeVariants];
    next[index] = Number((event.target as HTMLInputElement).value);
    this.store.update({ biomeVariants: next });
  }

  /** Waga wariantu jako szansa w obrębie typu. */
  protected variantChance(params: MapGenParams, index: number): string {
    const type = VARIANTS[index].type;
    const sum = VARIANTS.reduce((s, v, j) => s + (v.type === type ? (params.biomeVariants[j] ?? 0) : 0), 0);
    return `${sum > 0 ? Math.round(((params.biomeVariants[index] ?? 0) / sum) * 100) : 0}%`;
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
    // Esc otwiera i zamyka ustawienia zawsze, także z pola seeda.
    if (event.code === 'Escape') {
      this.collapsed.update((v) => !v);
      event.preventDefault();
      return;
    }
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
      default:
        return;
    }
    event.preventDefault();
  }
}
