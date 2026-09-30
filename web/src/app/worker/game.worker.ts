/// <reference lib="webworker" />

import init, { default_map_params, generate_map, generator_version } from '../../wasm/pkg/game_wasm';
import type { MapPayload, WorkerRequest, WorkerResponse } from './protocol';

// Plik .wasm trafia pod /wasm dzięki wpisowi "assets" w angular.json.
const ready = init({ module_or_path: '/wasm/game_wasm_bg.wasm' });

function reply(msg: WorkerResponse, transfer: Transferable[] = []): void {
  postMessage(msg, transfer);
}

/** Odległość (chamfer 3-4, ~euklidesowa) kafli oceanu od najbliższego nie-oceanu, w kaflach, obcięta do 255. */
function coastDistance(terrain: Uint8Array, w: number, h: number): Uint8Array {
  const INF = 1 << 20;
  const d = new Int32Array(w * h);
  for (let i = 0; i < w * h; i++) d[i] = terrain[i] === 0 ? INF : 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      let v = d[i];
      if (x > 0) v = Math.min(v, d[i - 1] + 3);
      if (y > 0) {
        v = Math.min(v, d[i - w] + 3);
        if (x > 0) v = Math.min(v, d[i - w - 1] + 4);
        if (x + 1 < w) v = Math.min(v, d[i - w + 1] + 4);
      }
      d[i] = v;
    }
  }
  const out = new Uint8Array(w * h);
  for (let y = h - 1; y >= 0; y--) {
    for (let x = w - 1; x >= 0; x--) {
      const i = y * w + x;
      let v = d[i];
      if (x + 1 < w) v = Math.min(v, d[i + 1] + 3);
      if (y + 1 < h) {
        v = Math.min(v, d[i + w] + 3);
        if (x + 1 < w) v = Math.min(v, d[i + w + 1] + 4);
        if (x > 0) v = Math.min(v, d[i + w - 1] + 4);
      }
      d[i] = v;
      out[i] = Math.min(255, Math.round(v / 3));
    }
  }
  return out;
}

function fnv1a(...arrays: Uint8Array[]): number {
  let h = 0x811c9dc5;
  for (const bytes of arrays) {
    for (let i = 0; i < bytes.length; i++) h = Math.imul(h ^ bytes[i], 0x01000193);
  }
  return h >>> 0;
}

addEventListener('message', async ({ data }: MessageEvent<WorkerRequest>) => {
  try {
    await ready;
    switch (data.type) {
      case 'defaults':
        reply({
          type: 'defaults',
          id: data.id,
          params: JSON.parse(default_map_params()),
          generatorVersion: generator_version(),
        });
        break;

      case 'generateMap': {
        const t0 = performance.now();
        const generated = generate_map(JSON.stringify(data.params));
        const map: MapPayload = {
          params: data.params,
          width: generated.width,
          height: generated.height,
          chunkCols: generated.chunkCols,
          chunkRows: generated.chunkRows,
          waterChunks: generated.takeWaterChunks(),
          terrain: generated.takeTerrain(),
          shade: generated.takeShade(),
          biome: generated.takeBiome(),
          biomeOther: generated.takeBiomeOther(),
          biomeMix: generated.takeBiomeMix(),
          forest: generated.takeForest(),
          fertility: generated.takeFertility(),
          coastDist: new Uint8Array(0),
          stats: JSON.parse(generated.statsJson()),
          hash: 0,
          biomeHash: 0,
          vegetationHash: 0,
          ms: 0,
        };
        generated.free();
        map.hash = fnv1a(map.terrain);
        map.biomeHash = fnv1a(map.biome, map.biomeOther, map.biomeMix);
        map.vegetationHash = fnv1a(map.forest, map.fertility);
        map.coastDist = coastDistance(map.terrain, map.width, map.height);
        map.ms = performance.now() - t0;
        const buffers = [map.terrain, map.shade, map.waterChunks, map.biome, map.biomeOther, map.biomeMix, map.forest, map.fertility, map.coastDist].map(
          (a) => a.buffer as ArrayBuffer,
        );
        reply({ type: 'map', id: data.id, map }, buffers);
        break;
      }
    }
  } catch (e) {
    reply({ type: 'error', id: data.id, message: e instanceof Error ? e.message : String(e) });
  }
});
