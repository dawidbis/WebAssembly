/// <reference lib="webworker" />

// Malowanie warstw mapy (RGBA na kafel) poza wątkiem głównym – zmiana widoku nie zamraża strony.

import type { MapPayload } from '../worker/protocol';
import type { PaintRequest, PaintResponse } from './painter';
import { paintOwners } from './owners';
import { paintProvinceBorders } from './provinces';
import { paintTerrain } from './terrain';

let map: MapPayload | null = null;

function reply(msg: PaintResponse, transfer: Transferable[]): void {
  postMessage(msg, transfer);
}

addEventListener('message', ({ data }: MessageEvent<PaintRequest>) => {
  switch (data.type) {
    case 'map':
      map = data.map;
      return;
    case 'provinces':
      if (map) map = { ...map, province: data.province, provinces: data.provinces, polished: data.polished };
      return;
    case 'terrain': {
      if (!map) return;
      const terrain = paintTerrain(map, data.view, data.contours, data.enabled);
      reply({ type: 'terrain', id: data.id, terrain }, [terrain.buffer]);
      return;
    }
    case 'borders': {
      if (!map) return;
      const borders = paintProvinceBorders(map);
      reply({ type: 'borders', id: data.id, borders }, [borders.buffer]);
      return;
    }
    case 'owners': {
      if (!map) return;
      const owners = paintOwners(map, data.tiles);
      reply({ type: 'owners', id: data.id, owners }, [owners.buffer]);
      return;
    }
  }
});
