import type { Catchup } from '../../generated/Catchup';
import type { Enclave } from '../../generated/Enclave';
import type { GameConfig } from '../../generated/GameConfig';
import type { MapGenParams } from '../../generated/MapGenParams';
import type { MapStats } from '../../generated/MapStats';
import type { Province } from '../../generated/Province';
import type { Turn } from '../../generated/Turn';

/** Wątek główny → worker: zapytania z odpowiedzią (po `id`). */
export type WorkerCall =
  | { type: 'defaults'; id: number }
  | { type: 'generateMap'; id: number; params: MapGenParams };

/**
 * Wątek główny → worker: pętla gry (bez odpowiedzi, odpowiedzią są zdarzenia `GameEvent`).
 * `startGame` – nowe `Welcome`: gra rusza, gdy worker ma mapę z `config.map` z policzonymi prowincjami
 * (nie generuje jej sam – mapę na ekran zleca wątek główny, a worker buduje z niej `WasmGame`).
 * `turn` – tura z serwera; do czasu zbudowania gry czeka w kolejce.
 */
export type GameRequest = { type: 'startGame'; config: GameConfig; catchup: Catchup } | { type: 'turn'; turn: Turn };

export type WorkerRequest = WorkerCall | GameRequest;

/** Gotowa mapa. Bufory są przenoszone (transfer), a nie kopiowane. */
export interface MapPayload {
  params: MapGenParams;
  width: number;
  height: number;
  chunkCols: number;
  chunkRows: number;
  /** 1 = chunk wodny, 0 = lądowy. */
  waterChunks: Uint8Array;
  /** Wartości `Terrain` z render/terrain.ts. */
  terrain: Uint8Array;
  /** Ląd: wysokość 0..255, ocean: głębokość 0..255. */
  shade: Uint8Array;
  /** Biom dominujący kafla (rodzaj o największej wadze, wartości `Biome` z render/terrain.ts). */
  biome: Uint8Array;
  /** 6 bajtów na kafel: [typ, σ1, σ2] typu głównego i drugiego kontynentu – wagi rodzajów liczy `kindWeights`. */
  biomeLayers: Uint8Array;
  /** Udział drugiego typu w kaflu: 0..255. */
  biomeMix: Uint8Array;
  /** 1 = lód morski (zamarznięty ocean przy lądolodzie; nadal ocean). */
  seaIce: Uint8Array;
  /** 1 = kafel lądu będący lodowcem (mapa z `glacier`: lądolód i kieszenie lądu zamknięte w lodzie). */
  glacier: Uint8Array;
  /** Gęstość lasu 0..255 (≥ 128 = las). Typ lasu wynika z biomu kafla. */
  forest: Uint8Array;
  /** Kafle rzek: odległość do ujścia wzdłuż nurtu (maleje z prądem), 0 = nie rzeka. */
  riverFlow: Uint16Array;
  /** Odległość kafla jeziora od brzegu jeziora w kaflach (0..255), poza jeziorami 0. */
  lakeDist: Uint8Array;
  /** Odległość kafla oceanu od lądu w kaflach (0..255, ląd = 0) – dla animacji fal. */
  coastDist: Uint8Array;
  /** Numer prowincji kafla (od 1), 0 = brak (woda, góry, lodowiec, enklawy). Do czasu wiadomości `provinces` same zera. */
  province: Uint16Array;
  /** Prowincje w kolejności numerów: `provinces[id - 1]`. Puste, dopóki prowincje się liczą. */
  provinces: Province[];
  /** Czy prowincje są już policzone (przychodzą osobno, po terenie). */
  provincesReady?: boolean;
  /** Czas liczenia prowincji w workerze (ms). */
  provincesMs?: number;
  /** Enklawy – odcięty ląd bez prowincji, z yeti (z ostatnich szlifów). */
  enclaves: Enclave[];
  /** Ostatnie szlify (faza 3) gotowe: znane tunele (`Province.tunnel`) i enklawy. */
  polished?: boolean;
  stats: MapStats;
  /** FNV-1a terenu – ten sam co w CLI `mapgen`, do porównań native vs wasm. */
  hash: number;
  /** FNV-1a biomów (`biome`, `biomeLayers`, `biomeMix`) – jak „hash biomów” w CLI. */
  biomeHash: number;
  /** FNV-1a roślinności (`forest`) – jak „hash roślinności” w CLI. */
  vegetationHash: number;
  /** FNV-1a prowincji (bajty `province`, little endian) – jak „hash prowincji” w CLI. */
  provinceHash: number;
  ms: number;
}

/** Worker → wątek główny: odpowiedzi na `WorkerCall` (to samo `id`). */
export type WorkerResponse =
  | { type: 'defaults'; id: number; params: MapGenParams; generatorVersion: number }
  | { type: 'map'; id: number; map: MapPayload }
  /** Druga faza tej samej mapy (to samo `id`): prowincje i pełne statystyki. */
  | {
      type: 'provinces';
      id: number;
      province: Uint16Array;
      provinces: Province[];
      stats: MapStats;
      provinceHash: number;
      ms: number;
    }
  /**
   * Trzecia faza tej samej mapy: ostatnie szlify – prowincje z tunelami i enklawy. `province`
   * (i jej hash) tylko wtedy, gdy enklawy zmieniły numerację prowincji.
   */
  | {
      type: 'polished';
      id: number;
      provinces: Province[];
      enclaves: Enclave[];
      stats: MapStats;
      province?: Uint16Array;
      provinceHash?: number;
      ms: number;
    }
  | { type: 'error'; id: number; message: string };

/** Hash stanu gry po wykonaniu tury `tick` – do odesłania serwerowi (`ClientMsg::Hash`). */
export interface TickHash {
  tick: number;
  hash: number;
}

/** Worker → wątek główny: zdarzenia pętli gry (bez `id`). */
export type GameEvent =
  /** Po wykonaniu tur: `tick` = liczba wykonanych tur, `hashes` – hashe do odesłania serwerowi. */
  | { type: 'game'; tick: number; hash: number; hashes: TickHash[] }
  /** Gra stanęła (np. tura spoza kolejki, inna wersja generatora); wznowi ją dopiero nowe `Welcome`. */
  | { type: 'gameError'; message: string };

/** Czy dwa zestawy parametrów dają tę samą mapę (te same pola i wartości). */
export function sameParams(a: MapGenParams, b: MapGenParams): boolean {
  const keys = Object.keys(a) as (keyof MapGenParams)[];
  return keys.length === Object.keys(b).length && keys.every((k) => sameValue(a[k], b[k]));
}

/** Równość pola parametrów: liczby i flagi wprost, tablice (np. `biomeVariants`) element po elemencie. */
function sameValue(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((v, i) => v === b[i]);
  return a === b;
}

/** Czy dwie konfiguracje opisują tę samą grę (ta sama mapa i wersja generatora). */
export function sameConfig(a: GameConfig, b: GameConfig): boolean {
  return a.generatorVersion === b.generatorVersion && sameParams(a.map, b.map);
}
