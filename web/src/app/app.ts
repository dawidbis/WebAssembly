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
import { Calendar } from './ui/calendar';
import { ResourceBar } from './ui/resource-bar';
import { StartTimer } from './ui/start-timer';
import { MapStore, type Wasteland } from './game/map-store';
import { MapRenderer } from './render/map-renderer';
import { Terrain } from './render/terrain';
import { GameStatus } from './ui/game-status';
import { ProvinceTooltip } from './ui/province-tooltip';
import { LobbyPanel } from './ui/lobby-panel';
import { Loading } from './ui/loading';
import { ProvinceInfo } from './ui/province-info';
import { ResourceLegend } from './ui/resource-legend';
import { StockWindow } from './ui/stock-window';
import { MarketWindow } from './ui/market-window';
import { NationWindow } from './ui/nation-window';
import { AttackPanel } from './ui/attack-panel';
import { BuildingsWindow } from './ui/buildings-window';
import { FrontsPanel } from './ui/fronts-panel';
import { RadialMenu } from './ui/radial-menu';
import { BuildingPopup } from './ui/building-popup';
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
  imports: [TopBar, ResourceBar, StartTimer, Calendar, ProvinceInfo, ResourceLegend, StockWindow, MarketWindow, NationWindow, BuildingsWindow, AttackPanel, FrontsPanel, RadialMenu, BuildingPopup, Loading, GameStatus, LobbyPanel, ProvinceTooltip],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  private readonly stage = viewChild.required<ElementRef<HTMLElement>>('stage');
  private readonly debugHost = viewChild.required('debugHost', { read: ViewContainerRef });
  protected readonly store = inject(MapStore);
  private readonly session = inject(GameSession);
  protected readonly lobby = inject(Lobby);
  private readonly renderer = new MapRenderer();

  constructor() {
    afterNextRender(async () => {
      await this.renderer.init(this.stage().nativeElement);
      if (DEV_TOOLS) {
        // Narzędzia i testy w przeglądarce (tylko dev): renderer z konsoli.
        (globalThis as Record<string, unknown>)['__renderer'] = this.renderer;
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
    effect(() => this.renderer.setLandingStyle(this.store.landingStyle()));
    effect(() => this.renderer.setPortReachStyle(this.store.portReachStyle()));
    // Kliknięty port: porty partnerów handlu z kosztem dostawy.
    effect(() => {
      const port = this.store.buildingPopup()?.building === 'port' ? this.session.port() : null;
      const view = port && {
        x: port.x,
        y: port.y,
        partners: port.partners.map((p) => ({
          x: p.x,
          y: p.y,
          label: `${this.session.playerName(p.player)} · ${Math.round(p.delivery * 100)}%${p.transit.length ? ' + cło' : ''}`,
        })),
      };
      untracked(() => this.renderer.setPortView(view));
    });
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
    this.renderer.onTileClick = (tile, mods) => {
      this.store.buildingPopup.set(null);
      if (!this.store.selectable()) return;
      const id = provinceAt(tile);
      // Czas wyboru startu: kliknięcie wolnej prowincji to wybór (można zmieniać do końca odliczania).
      if (id > 0 && this.session.phase() === 'selection' && this.session.showsGameMap() && this.canStartIn(id)) {
        this.previewStart(id);
        this.session.chooseStart(id);
        this.store.selectedProvince.set(id);
        return;
      }
      // Gra: Ctrl + klik – prowincja dołącza do celów jednego frontu (albo z nich wypada).
      if (id > 0 && mods.ctrl && this.session.phase() === 'playing') {
        const cur = this.store.selectedProvince();
        this.store.attackTargets.update((t) => {
          // Pierwszy Ctrl + klik zabiera też zaznaczoną wcześniej cudzą prowincję.
          let base = t;
          if (base.length === 0) base = cur > 0 && cur !== id && !this.allMine(cur) ? [cur] : [];
          if (base.includes(id)) return base.filter((q) => q !== id);
          return this.allMine(id) ? base : [...base, id];
        });
        this.store.selectedProvince.set(id);
        return;
      }
      this.store.attackTargets.set([]);
      this.store.selectedProvince.update((cur) => (id === cur ? 0 : id));
    };
    // Dwuklik na cudzą (albo nie w całości naszą) prowincję – atak od razu z udziałem wojska z suwaka panelu.
    this.renderer.onTileDoubleClick = (tile) => {
      if (!this.store.selectable() || this.session.phase() !== 'playing') return;
      const id = provinceAt(tile);
      if (id <= 0 || this.allMine(id)) return;
      const targets = this.store.attackTargets();
      this.session.attack(targets.includes(id) ? targets : [id], this.store.attackShare());
      // Drugie kliknięcie odznaczyło prowincję – zostaje zaznaczona (panel ataku pokazuje front).
      this.store.selectedProvince.set(id);
    };
    effect(() => this.renderer.setTargets(this.store.attackTargets()));
    effect(() => {
      if (this.session.phase() !== 'playing') untracked(() => this.store.attackTargets.set([]));
    });
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
    // Płynny przelot kamery (fronty, menu kołowe, start gry).
    effect(() => {
      const fly = this.store.flyRequest();
      if (fly) untracked(() => this.renderer.flyTo(fly.x, fly.y, fly.tiles ? this.renderer.scaleFor(fly.tiles) : undefined));
    });
    // Budynki na mapie (miejsca raz na grę, poziomy po zmianie); kliknięcie ikony – okienko budynku.
    effect(() => {
      const slots = this.session.showsGameMap() ? this.session.buildingSlots() : null;
      const levels = this.session.buildingLevels();
      const goods = { resources: this.session.provinceResources(), products: this.session.factoryProducts() };
      untracked(() => this.renderer.setBuildings(slots, levels, goods));
    });
    // Widoczność budynków (opcje wyświetlania, klawisz B, Shift + 1…7).
    effect(() => {
      const show = this.store.showBuildings();
      const filter = this.store.buildingFilter();
      untracked(() => this.renderer.setBuildingFilter(show, filter));
    });
    // Płynące desanty: kropka z flagą i wojskiem sunie od portu do celu.
    effect(() => {
      const list = this.session.showsGameMap() ? this.session.landings() : [];
      const tick = this.session.tick() ?? 0;
      untracked(() => this.renderer.setLandings(list, tick));
    });
    // Walczące fronty (flaga, ile vs ile) i przyczółki (kreskowanie na mapie politycznej).
    effect(() => {
      const marks = this.session.showsGameMap() ? this.session.frontMarks() : [];
      untracked(() => this.renderer.setFrontMarks(marks));
    });
    effect(() => {
      const tiles = this.session.showsGameMap() ? this.session.bridgeheads() : null;
      untracked(() => this.renderer.setBridgeheads(tiles));
    });
    // Trwające budowy: ikona z pierścieniem postępu.
    effect(() => {
      const slots = this.session.showsGameMap() ? this.session.buildingSlots() : null;
      const sites = this.session.constructions();
      untracked(() => this.renderer.setConstructions(slots, sites));
    });
    this.renderer.onBuildingClick = (hit, screen) => this.store.buildingPopup.set({ ...screen, ...hit });
    // Front co turę: zmiany kafli z workera prosto do renderera (bez sygnałów).
    this.session.onOwnerChanges = (changes) => this.renderer.applyOwnerChanges(changes);
    // Prawy przycisk na prowincji w grze – menu kołowe.
    this.renderer.onContextClick = (tile, screen) => {
      const id = this.store.selectable() ? provinceAt(tile) : 0;
      this.store.contextMenu.set(id > 0 && this.session.phase() === 'playing' ? { ...screen, province: id } : null);
    };
    // Okno budynków zamknięte ręcznie – otworzy się znowu po zaznaczeniu innej prowincji.
    effect(() => {
      const id = this.store.selectedProvince();
      untracked(() => {
        if (this.store.buildingsClosedFor() !== id) this.store.buildingsClosedFor.set(0);
      });
    });
    // Koniec odliczania: kamera płynnie na prowincję startową (gdy dojdą właściciele z tym startem).
    // Raz na grę: gdy gra właściwa trwa i znamy własne państwo – także przy losowych startach (bez
    // etapu wyboru) i po ponownym dołączeniu; nowa gra (poczekalnia, wybór) zeruje znacznik.
    let flyToStart = true;
    effect(() => {
      const phase = this.session.phase();
      const owners = this.session.owners();
      const nation = this.session.myNation();
      if (phase !== 'playing') flyToStart = true;
      if (!flyToStart || phase !== 'playing' || !owners || !nation) return;
      const k = owners.shares.findIndex((s) => s.some(([n]) => n === nation));
      const p = k >= 0 ? this.store.map()?.provinces[k] : undefined;
      if (!p) return;
      flyToStart = false;
      untracked(() => this.store.flyTo(p.centerX, p.centerY, p.area * 4));
    });

    // Z lobby (API /api dostępne) gracz wybiera pokój; bez niego – od razu tryb otwarty (/ws).
    void inject(Lobby).start();
    inject(DestroyRef).onDestroy(() => this.renderer.destroy());
  }

  /**
   * Czy prowincja nadaje się na start tego gracza: bez tunelu i bez innego gracza (własny start
   * można kliknąć – nic się nie zmieni). Ostatecznie decyduje rdzeń (`Game::choose_start`).
   */
  /**
   * Kolor startu od razu po kliknięciu (tura z wyborem wraca z serwera po kilku turach): poprzedni start
   * gracza wraca do niczyich. Numer nowego państwa – następny wolny (tak nadaje go rdzeń).
   */
  private previewStart(province: number): void {
    const owners = this.session.owners();
    const pending = this.session.pendingStart();
    const nation = this.session.myNation() ?? (owners?.nations.length ?? 0) + 1;
    const owned = (owners?.shares ?? []).findIndex((list) => list.some(([n]) => n === nation)) + 1;
    // Poprzedni start: z gry, a gdy jeszcze go nie ma – kliknięty przed chwilą (niepotwierdzony).
    const fresh = pending && (this.session.tick() ?? 0) - pending.tick < 20 ? pending.province : 0;
    const previous = owned || fresh;
    this.renderer.previewStart(province, previous === province ? 0 : previous, nation);
  }

  /** Czy cała prowincja należy do gracza. */
  private allMine(province: number): boolean {
    const nation = this.session.myNation();
    const area = this.store.map()?.provinces[province - 1]?.area ?? 0;
    const shares = this.session.owners()?.shares[province - 1] ?? [];
    return !!nation && shares.length === 1 && shares[0][0] === nation && shares[0][1] === area;
  }

  private canStartIn(province: number): boolean {
    const map = this.store.map();
    const owners = this.session.owners();
    if (!map || map.provinces[province - 1]?.tunnel) return false;
    const mine = this.session.myNation();
    return (owners?.shares[province - 1] ?? []).every(([nation]) => nation === mine);
  }
}
