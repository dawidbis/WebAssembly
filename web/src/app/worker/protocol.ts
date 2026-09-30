import type { MapGenParams } from '../../generated/MapGenParams';
import type { MapStats } from '../../generated/MapStats';
import type { Province } from '../../generated/Province';

/** Wątek główny → worker. */
export type WorkerRequest =
  | { type: 'defaults'; id: number }
  | { type: 'generateMap'; id: number; params: MapGenParams };

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
  /** Biom dominujący kafla (wartości `Biome` z render/terrain.ts). */
  biome: Uint8Array;
  /** Drugi biom w strefie przejścia (poza nią równy `biome`). */
  biomeOther: Uint8Array;
  /** Udział `biomeOther` w kaflu: 0..128 (128 = pół na pół). */
  biomeMix: Uint8Array;
  /** Gęstość lasu 0..255 (≥ 128 = las). Typ lasu wynika z biomu kafla. */
  forest: Uint8Array;
  /** Żyzność gleby 0..255 – pod przyszłe pola uprawne wokół miast. */
  fertility: Uint8Array;
  /** Kafle rzek: odległość do ujścia wzdłuż nurtu (maleje z prądem), 0 = nie rzeka. */
  riverFlow: Uint16Array;
  /** Odległość kafla jeziora od brzegu jeziora w kaflach (0..255), poza jeziorami 0. */
  lakeDist: Uint8Array;
  /** Odległość kafla oceanu od lądu w kaflach (0..255, ląd = 0) – dla animacji fal. */
  coastDist: Uint8Array;
  /** Numer prowincji kafla (od 1), 0 = brak (woda, góry). Do czasu wiadomości `provinces` same zera. */
  province: Uint16Array;
  /** Prowincje w kolejności numerów: `provinces[id - 1]`. Puste, dopóki prowincje się liczą. */
  provinces: Province[];
  /** Czy prowincje są już policzone (przychodzą osobno, po terenie). */
  provincesReady?: boolean;
  /** Czas liczenia prowincji w workerze (ms). */
  provincesMs?: number;
  stats: MapStats;
  /** FNV-1a terenu – ten sam co w CLI `mapgen`, do porównań native vs wasm. */
  hash: number;
  /** FNV-1a biomów (`biome`, `biomeOther`, `biomeMix`) – jak „hash biomów” w CLI. */
  biomeHash: number;
  /** FNV-1a roślinności (`forest`, `fertility`) – jak „hash roślinności” w CLI. */
  vegetationHash: number;
  /** FNV-1a prowincji (bajty `province`, little endian) – jak „hash prowincji” w CLI. */
  provinceHash: number;
  ms: number;
}

/** Worker → wątek główny. */
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
  | { type: 'error'; id: number; message: string };
