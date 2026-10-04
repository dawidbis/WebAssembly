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
import { MapStore } from './game/map-store';
import { MapRenderer } from './render/map-renderer';
import { GameStatus } from './ui/game-status';
import { Loading } from './ui/loading';
import { ProvinceInfo } from './ui/province-info';
import { TopBar } from './ui/top-bar';

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
    effect(() => this.renderer.setHighlight(this.store.selectedProvince(), this.store.hoveredProvince()));
    const provinceAt = (tile: { x: number; y: number } | null) => {
      const map = this.store.map();
      return map && tile ? map.province[tile.y * map.width + tile.x] : 0;
    };
    // Najechanie podświetla lekko; kliknięcie (bez przeciągania) zaznacza, a woda albo ta sama prowincja odznacza.
    this.renderer.onHover = (tile) => {
      const map = this.store.map();
      const id = provinceAt(tile);
      this.store.hoveredProvince.set(id);
      this.store.hoveredMountain.set(
        !!map?.provincesReady && !!tile && id === 0 && map.terrain[tile.y * map.width + tile.x] >= 2,
      );
    };
    this.renderer.onPainting = (painting) => this.store.painting.set(painting);
    this.renderer.onTileClick = (tile) => {
      const id = provinceAt(tile);
      this.store.selectedProvince.update((cur) => (id === cur ? 0 : id));
    };
    effect(() => {
      this.store.fitRequest();
      untracked(() => this.renderer.fit());
    });

    inject(GameSession).start();
    inject(DestroyRef).onDestroy(() => this.renderer.destroy());
  }
}
