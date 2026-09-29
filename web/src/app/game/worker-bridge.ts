import { Injectable } from '@angular/core';

import type { MapGenParams } from '../../generated/MapGenParams';
import type { MapPayload, WorkerRequest, WorkerResponse } from '../worker/protocol';

type WithoutId<T> = T extends unknown ? Omit<T, 'id'> : never;

/** Jedyne miejsce, które rozmawia z workerem. Zamienia postMessage na Promise. */
@Injectable({ providedIn: 'root' })
export class WorkerBridge {
  private readonly worker = new Worker(new URL('../worker/game.worker', import.meta.url), { type: 'module' });
  private readonly pending = new Map<number, { resolve: (r: WorkerResponse) => void; reject: (e: Error) => void }>();
  private nextId = 1;

  constructor() {
    this.worker.onmessage = ({ data }: MessageEvent<WorkerResponse>) => {
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

  async generateMap(params: MapGenParams): Promise<MapPayload> {
    const r = await this.call({ type: 'generateMap', params });
    if (r.type !== 'map') throw new Error(`Nieoczekiwana odpowiedź workera: ${r.type}`);
    return r.map;
  }

  private call(request: WithoutId<WorkerRequest>): Promise<WorkerResponse> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...request, id } as WorkerRequest);
    });
  }
}
