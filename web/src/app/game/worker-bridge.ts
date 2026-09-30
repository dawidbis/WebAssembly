import { Injectable } from '@angular/core';

import type { Catchup } from '../../generated/Catchup';
import type { GameConfig } from '../../generated/GameConfig';
import type { MapGenParams } from '../../generated/MapGenParams';
import type { Turn } from '../../generated/Turn';
import type { GameEvent, GameRequest, MapPayload, WorkerCall, WorkerResponse } from '../worker/protocol';

/** Druga faza mapy: prowincje. */
export type ProvincesResult = Extract<WorkerResponse, { type: 'provinces' }>;

type WithoutId<T> = T extends unknown ? Omit<T, 'id'> : never;

/** Jedyne miejsce, które rozmawia z workerem. Zamienia postMessage na Promise (mapy) i zdarzenia (gra). */
@Injectable({ providedIn: 'root' })
export class WorkerBridge {
  private readonly worker = new Worker(new URL('../worker/game.worker', import.meta.url), { type: 'module' });
  private readonly pending = new Map<number, { resolve: (r: WorkerResponse) => void; reject: (e: Error) => void }>();
  /** Odbiorcy prowincji (przychodzą po mapie, z tym samym `id`). */
  private readonly provinceListeners = new Map<number, (r: ProvincesResult | Error) => void>();
  private nextId = 1;
  /** Odbiorca zdarzeń pętli gry (tury wykonane, hashe, błąd gry). */
  onGame?: (event: GameEvent) => void;

  constructor() {
    this.worker.onmessage = ({ data }: MessageEvent<WorkerResponse | GameEvent>) => {
      if (data.type === 'game' || data.type === 'gameError') {
        this.onGame?.(data);
        return;
      }
      const listener = this.provinceListeners.get(data.id);
      if (listener && !this.pending.has(data.id) && (data.type === 'provinces' || data.type === 'error')) {
        this.provinceListeners.delete(data.id);
        listener(data.type === 'provinces' ? data : new Error(data.message));
        return;
      }
      const call = this.pending.get(data.id);
      if (!call) return;
      this.pending.delete(data.id);
      if (data.type === 'error') call.reject(new Error(data.message));
      else call.resolve(data);
    };
  }

  async defaults(): Promise<{ params: MapGenParams; generatorVersion: number }> {
    const r = await this.call({ type: 'defaults' });
    if (r.type !== 'defaults') throw new Error(`Nieoczekiwana odpowiedź workera: ${r.type}`);
    return r;
  }

  /**
   * Generuje mapę. Promise kończy się, gdy gotowy jest teren (faza 1); prowincje (faza 2)
   * trafiają później do `onProvinces`. Wyprzedzona przez nowsze żądanie – nie przychodzą wcale.
   */
  async generateMap(params: MapGenParams, onProvinces: (r: ProvincesResult | Error) => void): Promise<MapPayload> {
    // Starsze prowincje i tak nie przyjdą (worker je pomija) – nie trzymaj odbiorców.
    this.provinceListeners.clear();
    const id = this.nextId;
    this.provinceListeners.set(id, onProvinces);
    const r = await this.call({ type: 'generateMap', params });
    if (r.type !== 'map') throw new Error(`Nieoczekiwana odpowiedź workera: ${r.type}`);
    return r.map;
  }

  /**
   * Nowa gra z serwera. Worker zbuduje ją z mapy z `config.map`, gdy będzie gotowa (z prowincjami) –
   * mapę na ekran trzeba zlecić osobno (`generateMap`), worker nie generuje jej drugi raz.
   */
  startGame(config: GameConfig, catchup: Catchup): void {
    this.post({ type: 'startGame', config, catchup });
  }

  /** Tura z serwera – worker wykonuje ją od razu albo trzyma w kolejce do startu gry. */
  turn(turn: Turn): void {
    this.post({ type: 'turn', turn });
  }

  private post(request: GameRequest): void {
    this.worker.postMessage(request);
  }

  private call(request: WithoutId<WorkerCall>): Promise<WorkerResponse> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...request, id } as WorkerCall);
    });
  }
}
