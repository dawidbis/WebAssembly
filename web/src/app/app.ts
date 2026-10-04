import {
  Component,
  DestroyRef,
  ElementRef,
  ViewContainerRef,
  afterNextRender,
  effect,
  inject,
  untracked,
  viewChild,
} from '@angular/core';

import { GameSession } from './game/game-session';
import { MapStore, type Wasteland } from './game/map-store';
import { MapRenderer } from './render/map-renderer';
import { Terrain } from './render/terrain';
import { GameStatus } from './ui/game-status';
import { Loading } from './ui/loading';
import { ProvinceInfo } from './ui/province-info';
import { TopBar } from './ui/top-bar';
import type { MapPayload } from './worker/protocol';

/** Najkrótszy czas etapu „chowanie easter eggów” (ms) – żeby napis nie mignął. */
const EGGS_MIN_MS = 900;

/** Rodzaj lądu bez prowincji na kaflu `i`: góry, lodowiec, a poza nimi – enklawa. */
function wasteland(map: MapPayload, i: number): Wasteland {
  if (map.terrain[i] === Terrain.Mountains) return 'mountains';
  return map.glacier[i] ? 'glacier' : 'enclave';
}

@Component({
  selector: 'app-root',
  imports: [TopBar, ProvinceInfo, Loading, GameStatus],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  private readonly stage = viewChild.required<ElementRef<HTMLElement>>('stage');
  private readonly debugHost = viewChild.required('debugHost', { read: ViewContainerRef });
  private readonly store = inject(MapStore);
  private readonly renderer = new MapRenderer();

  constructor() {
    afterNextRender(async () => {
      await this.renderer.init(this.stage().nativeElement);
      if (DEV_TOOLS) {
        // W buildzie produkcyjnym DEV_TOOLS = false → esbuild wycina ten import w całości.
        const { DebugPanel } = await import('./debug/debug-panel');
        this.debugHost().createComponent(DebugPanel);
      }
    });

    effect(() => {
      const map = this.store.map();
      if (map) this.renderer.setMap(map);
    });
    effect(() => this.renderer.setChunkGridVisible(this.store.showChunkGrid()));
    effect(() => this.renderer.setContours(this.store.showContours()));
    effect(() => this.renderer.setWaves(this.store.showWaves(), this.store.waves()));
    effect(() => this.renderer.setProvinces(this.store.showProvinces(), this.store.borderOpacity()));
    effect(() => this.renderer.setView(this.store.view()));
    effect(() => this.renderer.setCreaturePixel(this.store.yetiPixel()));
    effect(() => this.renderer.setHighlight(this.store.selectedProvince(), this.store.hoveredProvince()));
    const provinceAt = (tile: { x: number; y: number } | null) => {
      const map = this.store.map();
      return map && tile ? map.province[tile.y * map.width + tile.x] : 0;
    };
    // Najechanie podświetla lekko; kliknięcie (bez przeciągania) zaznacza, a woda albo ta sama prowincja odznacza.
    this.renderer.onHover = (tile) => {
      const map = this.store.map();
      // Do końca ostatnich szlifów prowincje nie reagują (enklawy mogą jeszcze zmienić numerację).
      const id = this.store.selectable() ? provinceAt(tile) : 0;
      this.store.hoveredProvince.set(id);
      const i = map && tile ? tile.y * map.width + tile.x : -1;
      const waste = this.store.selectable() && i >= 0 && id === 0 && map!.terrain[i] >= Terrain.Plains;
      this.store.hoveredWaste.set(waste ? wasteland(map!, i) : null);
    };
    this.renderer.onPainting = (painting) => this.store.painting.set(painting);
    this.renderer.onTileClick = (tile) => {
      if (!this.store.selectable()) return;
      const id = provinceAt(tile);
      this.store.selectedProvince.update((cur) => (id === cur ? 0 : id));
    };
    // Yeti w enklawach: etap „chowanie easter eggów” trwa co najmniej chwilę.
    this.renderer.onCreatures = (count) => {
      if (count === 0) return;
      this.store.eggsPending.set(true);
      setTimeout(() => this.store.eggsPending.set(false), EGGS_MIN_MS);
    };
    effect(() => {
      this.store.fitRequest();
      untracked(() => this.renderer.fit());
    });

    inject(GameSession).start();
    inject(DestroyRef).onDestroy(() => this.renderer.destroy());
  }
}
