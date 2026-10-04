/// <reference lib="webworker" />

import type { Catchup } from '../../generated/Catchup';
import type { GameConfig } from '../../generated/GameConfig';
import type { MapGenParams } from '../../generated/MapGenParams';
import type { Turn } from '../../generated/Turn';
import init, { WasmGame, default_map_params, generate_map, generator_version, type GeneratedMap } from '../../wasm/pkg/game_wasm';
import { sameConfig, sameParams, type GameEvent, type MapPayload, type TickHash, type WorkerRequest, type WorkerResponse } from './protocol';

// Plik .wasm trafia pod /wasm dzięki wpisowi "assets" w angular.json.
const ready = init({ module_or_path: '/wasm/game_wasm_bg.wasm' });

/** Co tyle tur worker odsyła hash stanu serwerowi (serwer pamięta hashe z ostatnich 600 tur). */
const HASH_EVERY = 10;

function reply(msg: WorkerResponse | GameEvent, transfer: Transferable[] = []): void {
  postMessage(msg, transfer);
}

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Odległość (chamfer 3-4, ~euklidesowa) kafli danego typu wody (domyślnie ocean) od najbliższego innego kafla, w kaflach, obcięta do 255. */
function coastDistance(terrain: Uint8Array, w: number, h: number, water = 0): Uint8Array {
  const INF = 1 << 20;
  const d = new Int32Array(w * h);
  for (let i = 0; i < w * h; i++) d[i] = terrain[i] === water ? INF : 0;
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

/**
 * Ostatnio wygenerowana mapa. Zostaje w pamięci wasm, bo może z niej powstać gra – wtedy
 * przechodzi do `WasmGame` (mapa nie jest generowana drugi raz). Nowsze żądanie mapy ją zwalnia,
 * a prowincje starszej mapy są pomijane.
 */
let held: { id: number; params: MapGenParams; generated: GeneratedMap; provincesReady: boolean } | null = null;

/**
 * Gra z serwera (ostatnie `Welcome`). `game` powstaje dopiero z mapy z `config.map` z policzonymi
 * prowincjami; do tego czasu tury czekają w `queue`.
 */
let session: { config: GameConfig; catchup: Catchup; queue: Turn[]; game: WasmGame | null } | null = null;

function releaseHeld(): void {
  held?.generated.free();
  held = null;
}

function stopGame(error?: unknown): void {
  session?.game?.free();
  session = null;
  if (error !== undefined) reply({ type: 'gameError', message: message(error) });
}

/** Buduje grę z trzymanej mapy, jeśli to mapa z konfiguracji gry i ma już prowincje. */
function tryStartGame(): void {
  if (!session || session.game || !held?.provincesReady || !sameParams(held.params, session.config.map)) return;
  const generated = held.generated;
  held = null; // `fromMap` przejmuje mapę (także przy błędzie)
  try {
    session.game = WasmGame.fromMap(JSON.stringify(session.config), generated);
    session.game.catchUp(JSON.stringify(session.catchup));
    playQueued();
  } catch (e) {
    stopGame(e);
  }
}

/** Wykonuje tury z kolejki i melduje stan (co `HASH_EVERY` tur także hash dla serwera). */
function playQueued(): void {
  const game = session?.game;
  if (!session || !game) return;
  const hashes: TickHash[] = [];
  try {
    for (const turn of session.queue) {
      game.applyTurn(JSON.stringify(turn));
      if (turn.tick % HASH_EVERY === 0) hashes.push({ tick: turn.tick, hash: game.stateHash() });
    }
  } catch (e) {
    stopGame(e);
    return;
  }
  session.queue = [];
  reply({ type: 'game', tick: game.tick, hash: game.stateHash(), hashes });
}

function startGame(config: GameConfig, catchup: Catchup): void {
  if (config.generatorVersion !== generator_version()) {
    stopGame(`wersja generatora serwera (${config.generatorVersion}) inna niż klienta (${generator_version()}) – odśwież stronę`);
    return;
  }
  const game = session?.game;
  if (game && session && sameConfig(session.config, config)) {
    // Ta sama gra po ponownym połączeniu – od nowa na tej samej mapie, bez generowania.
    session = { config, catchup, queue: [], game };
    try {
      game.restart();
      game.catchUp(JSON.stringify(catchup));
      playQueued();
    } catch (e) {
      stopGame(e);
    }
    return;
  }
  stopGame();
  session = { config, catchup, queue: [], game: null };
  tryStartGame();
}

function fnv1a(...arrays: Uint8Array[]): number {
  let h = 0x811c9dc5;
  for (const bytes of arrays) {
    for (let i = 0; i < bytes.length; i++) h = Math.imul(h ^ bytes[i], 0x01000193);
  }
  return h >>> 0;
}

addEventListener('message', async ({ data }: MessageEvent<WorkerRequest>) => {
  if (data.type === 'startGame' || data.type === 'turn') {
    try {
      await ready;
    } catch (e) {
      reply({ type: 'gameError', message: message(e) });
      return;
    }
    if (data.type === 'startGame') {
      startGame(data.config, data.catchup);
    } else if (session) {
      session.queue.push(data.turn);
      playQueued();
    }
    return;
  }
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
        releaseHeld();
        // Faza 1: teren, biomy, woda, lasy. Prowincje (najdłuższy etap) dochodzą osobną wiadomością.
        const generated = generate_map(JSON.stringify(data.params));
        held = { id: data.id, params: data.params, generated, provincesReady: false };
        const map: MapPayload = {
          params: data.params,
          width: generated.width,
          height: generated.height,
          chunkCols: generated.chunkCols,
          chunkRows: generated.chunkRows,
          waterChunks: generated.waterChunks(),
          terrain: generated.terrain(),
          shade: generated.shade(),
          biome: generated.biome(),
          biomeOther: generated.biomeOther(),
          biomeMix: generated.biomeMix(),
          forest: generated.forest(),
          riverFlow: generated.riverFlow(),
          lakeDist: new Uint8Array(0),
          coastDist: new Uint8Array(0),
          province: new Uint16Array(generated.width * generated.height),
          provinces: [],
          stats: JSON.parse(generated.statsJson()),
          hash: 0,
          biomeHash: 0,
          vegetationHash: 0,
          provinceHash: 0,
          ms: 0,
        };
        map.hash = fnv1a(map.terrain);
        map.biomeHash = fnv1a(map.biome, map.biomeOther, map.biomeMix);
        map.vegetationHash = fnv1a(map.forest);
        map.coastDist = coastDistance(map.terrain, map.width, map.height);
        map.lakeDist = coastDistance(map.terrain, map.width, map.height, 1);
        map.ms = performance.now() - t0;
        const buffers = [map.terrain, map.shade, map.waterChunks, map.biome, map.biomeOther, map.biomeMix, map.forest, map.coastDist, map.riverFlow, map.lakeDist, map.province].map(
          (a) => a.buffer as ArrayBuffer,
        );
        reply({ type: 'map', id: data.id, map }, buffers);

        // Faza 2: prowincje – w osobnym zadaniu, żeby nowsze żądanie mogło je wyprzedzić.
        const id = data.id;
        setTimeout(() => {
          if (held?.id !== id) return;
          try {
            const t1 = performance.now();
            generated.computeProvinces();
            const province = generated.province();
            reply(
              {
                type: 'provinces',
                id,
                province,
                provinces: JSON.parse(generated.provincesJson()),
                stats: JSON.parse(generated.statsJson()),
                // Bajty Uint16Array w pamięci są little endian (wasm i praktycznie każdy procesor) – jak w CLI.
                provinceHash: fnv1a(new Uint8Array(province.buffer, province.byteOffset, province.byteLength)),
                ms: performance.now() - t1,
              },
              [province.buffer as ArrayBuffer],
            );
            held.provincesReady = true;
            tryStartGame();
          } catch (e) {
            reply({ type: 'error', id, message: message(e) });
            releaseHeld();
          }
        }, 0);
        break;
      }
    }
  } catch (e) {
    reply({ type: 'error', id: data.id, message: message(e) });
  }
});
