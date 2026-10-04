import type { Province } from '../../generated/Province';
import type { MapPayload } from '../worker/protocol';
import type { TerrainView } from './terrain';

/** Wątek główny → worker malujący. */
export type PaintRequest =
  | { type: 'map'; map: MapPayload }
  | { type: 'provinces'; province: Uint16Array; provinces: Province[] }
  | { type: 'terrain'; id: number; view: TerrainView; contours: boolean; iceEdges: boolean }
  | { type: 'borders'; id: number };

/** Worker malujący → wątek główny. Bufory RGBA są przenoszone, nie kopiowane. */
export type PaintResponse =
  | { type: 'terrain'; id: number; terrain: Uint8ClampedArray<ArrayBuffer> }
  | { type: 'borders'; id: number; borders: Uint8ClampedArray<ArrayBuffer> };

/** Klient workera malującego: mapa trafia do niego raz, potem prośby o warstwy zwracają Promise. */
export class Painter {
  private readonly worker = new Worker(new URL('./paint.worker', import.meta.url), { type: 'module' });
  private readonly pending = new Map<number, (r: PaintResponse) => void>();
  private nextId = 1;

  constructor() {
    this.worker.onmessage = ({ data }: MessageEvent<PaintResponse>) => {
      this.pending.get(data.id)?.(data);
      this.pending.delete(data.id);
    };
  }

  /** Kopia danych mapy dla workera (klonowanie strukturalne – wątek główny zachowuje swoje bufory). */
  setMap(map: MapPayload): void {
    this.worker.postMessage({ type: 'map', map } satisfies PaintRequest);
  }

  setProvinces(map: MapPayload): void {
    this.worker.postMessage({ type: 'provinces', province: map.province, provinces: map.provinces } satisfies PaintRequest);
  }

  terrain(view: TerrainView, contours: boolean, iceEdges: boolean): Promise<Extract<PaintResponse, { type: 'terrain' }>> {
    return this.call({ type: 'terrain', id: 0, view, contours, iceEdges }) as Promise<Extract<PaintResponse, { type: 'terrain' }>>;
  }

  borders(): Promise<Extract<PaintResponse, { type: 'borders' }>> {
    return this.call({ type: 'borders', id: 0 }) as Promise<Extract<PaintResponse, { type: 'borders' }>>;
  }

  destroy(): void {
    this.worker.terminate();
  }

  private call(request: Extract<PaintRequest, { id: number }>): Promise<PaintResponse> {
    const id = this.nextId++;
    return new Promise((resolve) => {
      this.pending.set(id, resolve);
      this.worker.postMessage({ ...request, id });
    });
  }
}
