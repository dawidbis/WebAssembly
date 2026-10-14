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
import { Lobby } from './game/lobby';
import { MapStore, type Wasteland } from './game/map-store';
import { MapRenderer } from './render/map-renderer';
import { Terrain } from './render/terrain';
import { GameStatus } from './ui/game-status';
import { LobbyPanel } from './ui/lobby-panel';
import { Loading } from './ui/loading';
import { ProvinceInfo } from './ui/province-info';
import { ResourceLegend } from './ui/resource-legend';
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
  imports: [TopBar, ProvinceInfo, ResourceLegend, Loading, GameStatus, LobbyPanel],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  private readonly stage = viewChild.required<ElementRef<HTMLElement>>('stage');
  private readonly debugHost = viewChild.required('debugHost', { read: ViewContainerRef });
  private readonly store = inject(MapStore);
  private readonly session = inject(GameSession);
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
    effect(() => this.renderer.setResourceFilter(this.store.resourceFilter()));
    effect(() => this.renderer.setCreaturePixel(this.store.yetiPixel()));
    effect(() => this.renderer.setHighlight(this.store.selectedProvince(), this.store.hoveredProvince()));
    // Właściciele tylko na mapie gry (nie na lokalnym podglądzie z panelu) i tylko, gdy ktoś coś ma.
    effect(() => {
      const owners = this.session.owners();
      this.renderer.setOwners(this.session.showsGameMap() && owners?.nations.length ? owners.tiles : null);
    });
    // Nazwy państw = nazwy graczy, którzy nimi kierują.
    effect(() => {
      const nations = this.session.owners()?.nations ?? [];
      this.session.roster();
      const names = new Map(nations.map((player, k) => [k + 1, this.session.playerName(player)] as const));
      untracked(() => this.renderer.setNationNames(names));
    });
    effect(() => this.renderer.setTerritoryStyle(this.store.territory()));
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
      // Czas wyboru startu: kliknięcie wolnej prowincji to wybór (można zmieniać do końca odliczania).
      if (id > 0 && this.session.phase() === 'selection' && this.session.showsGameMap() && this.canStartIn(id)) {
        this.session.chooseStart(id);
        this.store.selectedProvince.set(id);
        return;
      }
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

    // Z lobby (API /api dostępne) gracz wybiera pokój; bez niego – od razu tryb otwarty (/ws).
    void inject(Lobby).start();
    inject(DestroyRef).onDestroy(() => this.renderer.destroy());
  }

  /**
   * Czy prowincja nadaje się na start tego gracza: bez tunelu i bez innego gracza (własny start
   * można kliknąć – nic się nie zmieni). Ostatecznie decyduje rdzeń (`Game::choose_start`).
   */
  private canStartIn(province: number): boolean {
    const map = this.store.map();
    const owners = this.session.owners();
    if (!map || map.provinces[province - 1]?.tunnel) return false;
    const mine = this.session.myNation();
    return (owners?.shares[province - 1] ?? []).every(([nation]) => nation === mine);
  }
}
