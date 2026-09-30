/// <reference lib="webworker" />

import init, { default_map_params, generate_map, generator_version } from '../../wasm/pkg/game_wasm';
import { CURRENT_SCALE, type MapPayload, type WorkerRequest, type WorkerResponse } from './protocol';

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

/** Gładki szum wartości 0..1 (tylko do efektów wizualnych – nie musi być zgodny z Rustem). */
function valueNoise(seed: number): (x: number, y: number) => number {
  const hash = (x: number, y: number) => {
    let h = Math.imul(x, 374761393) ^ Math.imul(y, 668265263) ^ seed;
    h = Math.imul(h ^ (h >>> 13), 1274126177);
    return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
  };
  return (x, y) => {
    const xi = Math.floor(x);
    const yi = Math.floor(y);
    const fx = x - xi;
    const fy = y - yi;
    const u = fx * fx * (3 - 2 * fx);
    const v = fy * fy * (3 - 2 * fy);
    const a = hash(xi, yi) + (hash(xi + 1, yi) - hash(xi, yi)) * u;
    const b = hash(xi, yi + 1) + (hash(xi + 1, yi + 1) - hash(xi, yi + 1)) * u;
    return a + (b - a) * v;
  };
}

/**
 * Prądy morskie z funkcji strumienia ψ: v = (∂ψ/∂y, −∂ψ/∂x). Pole bez źródeł i ujść.
 * ψ to wolnozmienny szum (wielkie wiry) wygaszony przy lądzie – dlatego przy brzegu
 * prąd płynie wzdłuż linii brzegowej, a nie w ląd.
 */
function oceanCurrents(coastDist: Uint8Array, w: number, h: number, seed: number): Uint8Array {
  const noise = valueNoise(seed ^ 0x5bd1e995);
  const psi = new Float32Array(w * h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      const d = coastDist[i];
      if (d === 0) continue;
      const t = Math.min(1, d / 30);
      const shore = t * t * (3 - 2 * t);
      const g = noise(x / 260, y / 260) + 0.5 * noise(x / 130 + 7.3, y / 130 + 1.9);
      psi[i] = (g - 0.75) * 400 * shore;
    }
  }
  const out = new Uint8Array(w * h * 2).fill(128);
  const e = 2;
  const MAX = 4;
  for (let y = e; y < h - e; y++) {
    for (let x = e; x < w - e; x++) {
      const i = y * w + x;
      if (coastDist[i] === 0) continue;
      let vx = (psi[i + e * w] - psi[i - e * w]) / (2 * e);
      let vy = -(psi[i + e] - psi[i - e]) / (2 * e);
      const s = Math.hypot(vx, vy);
      if (s > MAX) {
        vx *= MAX / s;
        vy *= MAX / s;
      }
      out[i * 2] = 128 + vx * CURRENT_SCALE;
      out[i * 2 + 1] = 128 + vy * CURRENT_SCALE;
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
          coastDist: new Uint8Array(0),
          currents: new Uint8Array(0),
          stats: JSON.parse(generated.statsJson()),
          hash: 0,
          biomeHash: 0,
          ms: 0,
        };
        generated.free();
        map.hash = fnv1a(map.terrain);
        map.biomeHash = fnv1a(map.biome, map.biomeOther, map.biomeMix);
        map.coastDist = coastDistance(map.terrain, map.width, map.height);
        map.currents = oceanCurrents(map.coastDist, map.width, map.height, data.params.seed);
        map.ms = performance.now() - t0;
        const buffers = [map.terrain, map.shade, map.waterChunks, map.biome, map.biomeOther, map.biomeMix, map.coastDist, map.currents].map(
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
