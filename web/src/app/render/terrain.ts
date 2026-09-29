import type { MapPayload } from '../worker/protocol';

/** Typy kafli – muszą zgadzać się z `game_mapgen::Terrain`. */
export const Terrain = {
  Ocean: 0,
  Lake: 1,
  River: 2,
  Plains: 3,
  Highlands: 4,
  Mountains: 5,
} as const;

/** Ta sama paleta co w CLI `mapgen` (crates/mapgen/src/bin/mapgen.rs). */
export function paintTerrain(map: MapPayload): Uint8ClampedArray<ArrayBuffer> {
  const { width: w, height: h, terrain, shade } = map;
  const out = new Uint8ClampedArray(w * h * 4);

  for (let i = 0; i < w * h; i++) {
    const t = terrain[i];
    const k = shade[i] / 255;
    let r: number, g: number, b: number;

    switch (t) {
      case Terrain.Ocean: {
        const d = Math.sqrt(k);
        r = 47 + (13 - 47) * d;
        g = 111 + (42 - 111) * d;
        b = 159 + (74 - 159) * d;
        break;
      }
      case Terrain.Lake:
        [r, g, b] = [63, 134, 184];
        break;
      case Terrain.River:
        [r, g, b] = [74, 144, 196];
        break;
      case Terrain.Plains:
        r = 104 + 46 * k;
        g = 150 + 20 * k;
        b = 72 + 24 * k;
        break;
      case Terrain.Highlands:
        r = 160 - 20 * k;
        g = 150 - 30 * k;
        b = 98 - 14 * k;
        break;
      default: {
        const snow = Math.min(1, Math.max(0, (k - 0.55) / 0.45));
        r = 128 + 110 * snow;
        g = 118 + 118 * snow;
        b = 108 + 122 * snow;
      }
    }

    if (t >= Terrain.Plains) {
      const light = hillshade(terrain, shade, w, h, i);
      r *= light;
      g *= light;
      b *= light;
    }
    const o = i * 4;
    out[o] = r;
    out[o + 1] = g;
    out[o + 2] = b;
    out[o + 3] = 255;
  }
  return out;
}

/** Proste cieniowanie rzeźby: światło z lewego górnego rogu. */
function hillshade(terrain: Uint8Array, shade: Uint8Array, w: number, h: number, i: number): number {
  const x = i % w;
  const y = (i - x) / w;
  if (x === 0 || y === 0 || x + 1 >= w || y + 1 >= h) return 1;
  const a = terrain[i - w - 1] >= Terrain.Plains ? shade[i - w - 1] : shade[i];
  const b = terrain[i + w + 1] >= Terrain.Plains ? shade[i + w + 1] : shade[i];
  return Math.min(1.3, Math.max(0.7, 1 + (a - b) * 0.02));
}
