/// <reference lib="webworker" />

import init, { default_map_params, generate_map, generator_version } from '../../wasm/pkg/game_wasm';
import type { MapPayload, WorkerRequest, WorkerResponse } from './protocol';

// Plik .wasm trafia pod /wasm dzięki wpisowi "assets" w angular.json.
const ready = init({ module_or_path: '/wasm/game_wasm_bg.wasm' });

function reply(msg: WorkerResponse, transfer: Transferable[] = []): void {
  postMessage(msg, transfer);
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
          stats: JSON.parse(generated.statsJson()),
          hash: 0,
          biomeHash: 0,
          ms: 0,
        };
        generated.free();
        map.hash = fnv1a(map.terrain);
        map.biomeHash = fnv1a(map.biome, map.biomeOther, map.biomeMix);
        map.ms = performance.now() - t0;
        const buffers = [map.terrain, map.shade, map.waterChunks, map.biome, map.biomeOther, map.biomeMix].map(
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
