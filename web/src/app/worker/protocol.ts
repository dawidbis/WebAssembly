import type { MapGenParams } from '../../generated/MapGenParams';
import type { MapStats } from '../../generated/MapStats';

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
  /** Odległość kafla oceanu od lądu w kaflach (0..255, ląd = 0) – dla animacji fal. */
  coastDist: Uint8Array;
  stats: MapStats;
  /** FNV-1a terenu – ten sam co w CLI `mapgen`, do porównań native vs wasm. */
  hash: number;
  /** FNV-1a biomów (`biome`, `biomeOther`, `biomeMix`) – jak „hash biomów” w CLI. */
  biomeHash: number;
  ms: number;
}

/** Worker → wątek główny. */
export type WorkerResponse =
  | { type: 'defaults'; id: number; params: MapGenParams; generatorVersion: number }
  | { type: 'map'; id: number; map: MapPayload }
  | { type: 'error'; id: number; message: string };
