/// <reference lib="webworker" />

import type { Catchup } from '../../generated/Catchup';
import type { GameConfig } from '../../generated/GameConfig';
import type { MapGenParams } from '../../generated/MapGenParams';
import type { Turn } from '../../generated/Turn';
import init, { WasmGame, default_map_params, generate_map, generator_version, supply_rules, type GeneratedMap } from '../../wasm/pkg/game_wasm';
import {
  sameConfig,
  sameParams,
  type GameEvent,
  type GamePhase,
  type MapPayload,
  type OwnersState,
  type TickHash,
  type WorkerRequest,
  type WorkerResponse,
} from './protocol';

// Plik .wasm trafia pod /wasm dzięki wpisowi "assets" w angular.json.
const ready = init({ module_or_path: '/wasm/game_wasm_bg.wasm' });

/** Więcej zmian kafli w jednym zdarzeniu nie idzie osobno (np. po nadrabianiu) – pokryją je pełne `owners`. */
const MAX_CHANGES = 20_000;

/** Właściciele kafli idą na ekran najwyżej co tyle tur (1 s) – ostatnia zmiana i tak dochodzi po chwili. */
const OWNERS_EVERY = 10;

/** Co tyle tur worker odsyła hash stanu serwerowi (serwer pamięta hashe z ostatnich 600 tur). */
const HASH_EVERY = 10;

/** `WasmGame.phase` → etap gry. */
const PHASES: readonly GamePhase[] = ['waiting', 'selection', 'playing'];

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
  // Dwa przebiegi transformaty odległości: w przód (sąsiedzi z lewej i z góry), potem w tył.
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) d[y * w + x] = forwardMin(d, w, x, y);
  }
  const out = new Uint8Array(w * h);
  for (let y = h - 1; y >= 0; y--) {
    for (let x = w - 1; x >= 0; x--) {
      const i = y * w + x;
      d[i] = backwardMin(d, w, h, x, y);
      out[i] = Math.min(255, Math.round(d[i] / 3));
    }
  }
  return out;
}

/** Przebieg w przód: minimum z kafla i sąsiadów już policzonych (lewy, górny, górne skosy). */
function forwardMin(d: Int32Array, w: number, x: number, y: number): number {
  const i = y * w + x;
  let v = d[i];
  if (x > 0) v = Math.min(v, d[i - 1] + 3);
  if (y > 0) {
    v = Math.min(v, d[i - w] + 3);
    if (x > 0) v = Math.min(v, d[i - w - 1] + 4);
    if (x + 1 < w) v = Math.min(v, d[i - w + 1] + 4);
  }
  return v;
}

/** Przebieg w tył: minimum z kafla i sąsiadów prawego, dolnego i dolnych skosów. */
function backwardMin(d: Int32Array, w: number, h: number, x: number, y: number): number {
  const i = y * w + x;
  let v = d[i];
  if (x + 1 < w) v = Math.min(v, d[i + 1] + 3);
  if (y + 1 < h) {
    v = Math.min(v, d[i + w] + 3);
    if (x + 1 < w) v = Math.min(v, d[i + w + 1] + 4);
    if (x > 0) v = Math.min(v, d[i + w - 1] + 4);
  }
  return v;
}

/**
 * Ostatnio wygenerowana mapa. Zostaje w pamięci wasm, bo może z niej powstać gra – wtedy
 * przechodzi do `WasmGame` (mapa nie jest generowana drugi raz). Nowsze żądanie mapy ją zwalnia,
 * a prowincje starszej mapy są pomijane.
 */
let held: { id: number; params: MapGenParams; generated: GeneratedMap; polished: boolean } | null = null;

/**
 * Gra z serwera (ostatnie `Welcome`). `game` powstaje dopiero z mapy z `config.map` z policzonymi
 * prowincjami; do tego czasu tury czekają w `queue`. `sentOwners` – wersja właścicieli ostatnio
 * wysłana na ekran (-1 = jeszcze żadna w tej grze, także po restarcie).
 */
let session: {
  config: GameConfig;
  catchup: Catchup;
  queue: Turn[];
  game: WasmGame | null;
  sentOwners: number;
  /** Miejsca budynków wysłane w tej grze i wersja wysłanych poziomów budynków (-1 – żadna). */
  sentSlots: boolean;
  sentBuildings: number;
  /** Ostatnio wysłane budowy (wysyłane tylko po zmianie postępu). */
  sentConstructions: Uint8Array | null;
  /** Tick ostatniego wysłania właścicieli – w czasie ataków zmieniają się co turę, a pełne przemalowanie
   * terytoriów jest drogie, więc idą najwyżej co `OWNERS_EVERY` tur. */
  ownersTick: number;
} | null = null;

/** Prowincja w ramce na ekranie i gracz tej karty – jej budynki idą w każdym zdarzeniu gry. */
let watched: { province: number; player: number; targets: number[] } = { province: 0, player: 0, targets: [] };

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
  if (!session || session.game || !held?.polished || !sameParams(held.params, session.config.map)) return;
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
  const all = game.ownerChanges();
  const changes = all.length > 0 && all.length <= MAX_CHANGES ? all : undefined;
  const owners = changedOwners(game);
  const slots = session.sentSlots ? undefined : game.buildingSlots();
  session.sentSlots = true;
  const buildingLevels = session.sentBuildings === game.buildingsRevision ? undefined : game.buildingLevels();
  session.sentBuildings = game.buildingsRevision;
  const sites = game.buildingConstructions();
  const constructions = sameBytes(sites, session.sentConstructions) ? undefined : sites;
  if (constructions) session.sentConstructions = constructions.slice();
  const phase = PHASES[game.phase] ?? 'waiting';
  reply(
    {
      type: 'game',
      tick: game.tick,
      hash: game.stateHash(),
      hashes,
      phase,
      selectionEnd: game.selectionEnd,
      economy: JSON.parse(game.reportsJson()),
      nextTax: game.nextTaxTick,
      playingSince: game.playingSince,
      owners,
      province: watched.province > 0 ? JSON.parse(game.provinceReportJson(watched.player, watched.province)) : null,
      attackCheck:
        watched.targets.length > 1 ? JSON.parse(game.checkAttackJson(watched.player, Uint16Array.from(watched.targets))) : null,
      changes,
      fronts: JSON.parse(game.frontsJson(watched.player)),
      slots,
      buildingLevels,
      constructions,
    },
    [owners?.tiles, changes, slots, buildingLevels, constructions].filter((a) => a !== undefined).map((a) => a.buffer as ArrayBuffer),
  );
}

function sameBytes(a: Uint8Array, b: Uint8Array | null): boolean {
  if (!b || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

/** Właściciele kafli, jeśli zmienili się od ostatniego wysłania (tablica kafli jest kopią z wasm). */
function changedOwners(game: WasmGame): OwnersState | undefined {
  if (!session || game.ownersRevision === session.sentOwners) return undefined;
  // Rzadziej tylko w grze właściwej – przy wyborze startu kolor i podpis mają się pojawić od razu.
  if (session.sentOwners !== -1 && PHASES[game.phase] === 'playing' && game.tick - session.ownersTick < OWNERS_EVERY)
    return undefined;
  session.sentOwners = game.ownersRevision;
  session.ownersTick = game.tick;
  return {
    revision: game.ownersRevision,
    tiles: game.owners(),
    nations: Array.from(game.nations()),
    shares: JSON.parse(game.provinceSharesJson()),
  };
}

function startGame(config: GameConfig, catchup: Catchup): void {
  if (config.generatorVersion !== generator_version()) {
    stopGame(`wersja generatora serwera (${config.generatorVersion}) inna niż klienta (${generator_version()}) – odśwież stronę`);
    return;
  }
  const game = session?.game;
  if (game && session && sameConfig(session.config, config)) {
    // Ta sama gra po ponownym połączeniu – od nowa na tej samej mapie, bez generowania.
    session = { config, catchup, queue: [], game, sentOwners: -1, ownersTick: 0, sentSlots: false, sentBuildings: -1, sentConstructions: null };
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
  session = { config, catchup, queue: [], game: null, sentOwners: -1, ownersTick: 0, sentSlots: false, sentBuildings: -1, sentConstructions: null };
  tryStartGame();
}

/** Faza 3 (ostatnie szlify): tunele i enklawy – też w osobnym zadaniu, potem gra może ruszyć. */
function polish(id: number, generated: GeneratedMap): void {
  if (held?.id !== id) return;
  try {
    const t = performance.now();
    // Enklawy zmieniają numerację prowincji – wtedy idzie też nowa tablica `province`.
    const province = generated.polish() ? generated.province() : undefined;
    reply(
      {
        type: 'polished',
        id,
        provinces: JSON.parse(generated.provincesJson()),
        enclaves: JSON.parse(generated.enclavesJson()),
        stats: JSON.parse(generated.statsJson()),
        province,
        provinceHash: province && provinceHash(province),
        ms: performance.now() - t,
      },
      province ? [province.buffer as ArrayBuffer] : [],
    );
    held.polished = true;
    tryStartGame();
  } catch (e) {
    reply({ type: 'error', id, message: message(e) });
    releaseHeld();
  }
}

/** Hash prowincji – bajty Uint16Array w pamięci są little endian (wasm i praktycznie każdy procesor), jak w CLI. */
function provinceHash(province: Uint16Array): number {
  return fnv1a(new Uint8Array(province.buffer, province.byteOffset, province.byteLength));
}

function fnv1a(...arrays: Uint8Array[]): number {
  let h = 0x811c9dc5;
  for (const bytes of arrays) {
    for (const b of bytes) h = Math.imul(h ^ b, 0x01000193);
  }
  return h >>> 0;
}

addEventListener('message', async ({ data }: MessageEvent<WorkerRequest>) => {
  if (data.type === 'watch') {
    watched = { province: data.province, player: data.player, targets: data.targets };
    // Od razu, bez czekania na turę (pusta kolejka – samo zdarzenie ze stanem).
    if (session?.game) playQueued();
    return;
  }
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
          supplyRules: JSON.parse(supply_rules()),
        });
        break;

      case 'generateMap': {
        const t0 = performance.now();
        releaseHeld();
        // Faza 1: teren, biomy, woda, lasy. Prowincje (najdłuższy etap) dochodzą osobną wiadomością.
        const generated = generate_map(JSON.stringify(data.params));
        held = { id: data.id, params: data.params, generated, polished: false };
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
          biomeLayers: generated.biomeLayers(),
          biomeMix: generated.biomeMix(),
          seaIce: generated.seaIce(),
          glacier: generated.glacier(),
          forest: generated.forest(),
          riverFlow: generated.riverFlow(),
          lakeDist: new Uint8Array(0),
          coastDist: new Uint8Array(0),
          province: new Uint16Array(generated.width * generated.height),
          provinces: [],
          enclaves: [],
          stats: JSON.parse(generated.statsJson()),
          hash: 0,
          biomeHash: 0,
          vegetationHash: 0,
          provinceHash: 0,
          ms: 0,
        };
        map.hash = fnv1a(map.terrain);
        map.biomeHash = fnv1a(map.biome, map.biomeLayers, map.biomeMix, map.seaIce);
        map.vegetationHash = fnv1a(map.forest);
        // Fale łamią się na krawędzi lodu morskiego – lód liczy się tu jak ląd.
        const waveTerrain = map.terrain.map((t, i) => (map.seaIce[i] ? 3 : t)) // 3 = Terrain.Plains);
        map.coastDist = coastDistance(waveTerrain, map.width, map.height);
        map.lakeDist = coastDistance(map.terrain, map.width, map.height, 1);
        map.ms = performance.now() - t0;
        const buffers = [map.terrain, map.shade, map.waterChunks, map.biome, map.biomeLayers, map.biomeMix, map.seaIce, map.glacier, map.forest, map.coastDist, map.riverFlow, map.lakeDist, map.province].map(
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
                provinceHash: provinceHash(province),
                ms: performance.now() - t1,
              },
              [province.buffer as ArrayBuffer],
            );
            setTimeout(() => polish(id, generated), 0);
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
