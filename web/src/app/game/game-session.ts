import { Injectable, Injector, computed, effect, inject, signal } from '@angular/core';

import type { Building } from '../../generated/Building';
import type { Supply } from '../../generated/Supply';

import type { GameConfig } from '../../generated/GameConfig';
import type { LobbyPlayer } from '../../generated/LobbyPlayer';
import type { FrontMark } from '../../generated/FrontMark';
import type { FrontReport } from '../../generated/FrontReport';
import type { SailingReport } from '../../generated/SailingReport';
import type { MarketReport } from '../../generated/MarketReport';
import type { GarrisonReport } from '../../generated/GarrisonReport';
import type { PortReport } from '../../generated/PortReport';
import type { NationReport } from '../../generated/NationReport';
import type { ProvinceReport } from '../../generated/ProvinceReport';
import type { RoomSettings } from '../../generated/RoomSettings';
import type { ServerMsg } from '../../generated/ServerMsg';
import type { PoolState } from '../ui/recipe';
import { sameConfig, sameParams, type AttackCheck, type GameEvent, type GamePhase, type OwnersState, type TickHash } from '../worker/protocol';
import { MapStore } from './map-store';
import { Transport, type PathProvider } from './transport';
import { WorkerBridge } from './worker-bridge';

/** Po tylu ms bez `Welcome` klient generuje mapę z domyślnych parametrów (serwer nie odpowiada). */
const OFFLINE_FALLBACK_MS = 3000;
/** Gracz „nikt” dla workera (stempel serwera – żaden gracz nie ma tego numeru). */
const NO_PLAYER = 0xffff;
/** Najwięcej hashy wysyłanych na sekundę (normalnie 1 – co 10 tur). */
const HASHES_PER_SECOND = 5;

/**
 * Pętla lockstep po stronie klienta: `Welcome` → mapa z `GameConfig` (na ekran i do gry, generowana
 * raz) → worker buduje z niej `WasmGame`, wykonuje tury z serwera i zwraca hashe stanu, które
 * sesja odsyła serwerowi. Bez serwera mapa powstaje z domyślnych parametrów, a gry nie ma.
 */
@Injectable({ providedIn: 'root' })
export class GameSession {
  private readonly transport = inject(Transport);
  private readonly bridge = inject(WorkerBridge);
  private readonly store = inject(MapStore);
  private readonly injector = inject(Injector);
  /** Tryb otwarty (bez lobby) – wtedy brak serwera = mapa z domyślnych parametrów. */
  private openMode = true;

  /** Konfiguracja gry z ostatniego `Welcome` (null = jeszcze bez serwera). */
  readonly config = signal<GameConfig | null>(null);
  readonly player = signal<number | null>(null);
  /** Ostatnia tura rozesłana przez serwer. */
  readonly serverTick = signal<number | null>(null);
  /** Tury wykonane przez grę w workerze (null = gra jeszcze nie ruszyła, np. mapa się generuje). */
  readonly tick = signal<number | null>(null);
  /** Bieżący hash stanu gry. */
  readonly hash = signal<number | null>(null);
  /** Ostatni hash odesłany serwerowi. */
  readonly sentHash = signal<TickHash | null>(null);
  /** Właściciele kafli w bieżącej grze (null = gra jeszcze nie ruszyła). */
  readonly owners = signal<OwnersState | null>(null);
  /** Etap gry: czekanie na gotowość graczy, wybór prowincji startowych, gra (null – gra nie ruszyła). */
  readonly phase = signal<GamePhase | null>(null);
  /**
   * Odliczanie wyboru startu: pierwszy tick po wyborze i chwila (`performance.now()`) wykonania ostatniej
   * tury – czas na ekranie liczy się z ticków (tura = 100 ms), a między turami płynnie dopowiada go zegar.
   */
  readonly selection = signal<{ end: number; tick: number; at: number } | null>(null);
  /** Gospodarka państw (`[n - 1]` – państwo `n`) z ostatniej wykonanej tury. */
  readonly economy = signal<NationReport[]>([]);
  /** Okno limitu hashy (`HASHES_PER_SECOND`). */
  private hashWindow = { start: 0, sent: 0 };
  /** Najbliższy podatek: tick i chwila wykonania ostatniej tury (odliczanie jak przy wyborze startu). */
  readonly tax = signal<{ at: number; tick: number; now: number } | null>(null);
  /** Tick początku gry właściwej (null przed nią) – kalendarz. */
  readonly playingSince = signal<number | null>(null);
  /** Budynki na mapie: stałe miejsca (raz na grę) i poziomy wszystkich prowincji (po zmianie). */
  /** Miejsca budynków z mapy (`Province::slots`: [x, y] × 4 na prowincję; 65535 – brak). */
  readonly buildingSlots = computed(() => {
    const provinces = this.store.map()?.provinces;
    if (!provinces || provinces.length === 0) return null;
    const out = new Uint16Array(provinces.length * 8);
    provinces.forEach((p, k) => p.slots.forEach(([x, y], i) => out.set([x, y], k * 8 + i * 2)));
    return out;
  });
  readonly buildingLevels = signal<Uint8Array | null>(null);
  /** Trwające budowy wszystkich prowincji (ikony z pierścieniem postępu na mapie). */
  readonly constructions = signal<Uint8Array | null>(null);
  /** Produkt fabryki każdej prowincji (`Supply` + 1, 0 – bez fabryki) – ikona na mapie. */
  readonly factoryProducts = signal<Uint8Array | null>(null);
  /** Surowiec każdej prowincji (`[id - 1]`) – ikona na wydobyciu. */
  readonly provinceResources = computed(() => {
    const provinces = this.store.map()?.provinces;
    return provinces && provinces.length > 0 ? Uint8Array.from(provinces, (p) => p.resource) : null;
  });
  /** Aktywne fronty gracza (jego ataki i ataki na jego prowincje) z ostatniej tury. */
  readonly fronts = signal<FrontReport[]>([]);
  /** Płynące desanty wszystkich państw (kropki na mapie). */
  readonly landings = signal<SailingReport[]>([]);
  /** Rynek gracza (tylko przy otwartym oknie rynku). */
  readonly market = signal<MarketReport | null>(null);
  /** Garnizony przyczółków gracza. */
  readonly garrisons = signal<GarrisonReport[]>([]);
  /** Port z okienka budynku: partnerzy handlu morskiego. */
  readonly port = signal<PortReport | null>(null);
  /** Walczące fronty wszystkich państw – flaga i „ile vs ile” na mapie. */
  readonly frontMarks = signal<FrontMark[]>([]);
  /** Kafle odciętych przyczółków wszystkich państw (kreskowanie na mapie politycznej). */
  readonly bridgeheads = signal<Uint32Array | null>(null);
  /** Zmiany kafli z każdej tury (`kafel << 8 | państwo`) – renderer rysuje front płynnie, bez sygnałów. */
  onOwnerChanges?: (changes: Uint32Array) => void;
  /** Budynki prowincji w ramce (zaznaczonej albo pod kursorem) z ostatniej tury; null – bez gry. */
  readonly provinceReport = signal<ProvinceReport | null>(null);
  /** Sprawdzenie ataku na prowincje zaznaczone z Ctrl (z workera). */
  readonly attackCheck = signal<AttackCheck | null>(null);
  /** Gra u klienta zgłoszona serwerowi jako gotowa (`ClientMsg::Ready`) – raz na `Welcome`. */
  private readySent = false;
  /** Pierwszy tick, na którym serwer zgłosił rozbieżność stanu (null = brak). */
  readonly desync = signal<number | null>(null);
  /** Gra stanęła z błędem (np. inna wersja generatora niż na serwerze). */
  readonly error = signal<string | null>(null);
  /** Poczekalnia pokoju (`ServerMsg::Lobby`): gracze, gospodarz, czy gra ruszyła. */
  readonly roster = signal<LobbyPlayer[]>([]);
  readonly host = signal<number | null>(null);
  readonly started = signal(false);
  /** Bieżące ustawienia pokoju (z `ServerMsg::Lobby`; gospodarz może je zmieniać w poczekalni). */
  readonly settings = signal<RoomSettings | null>(null);
  readonly isHost = computed(() => this.player() !== null && this.player() === this.host());
  /** Serwer nie wpuścił do pokoju (pełny, gra trwa bez nas) albo pokój się skończył (gospodarz go zamknął). */
  readonly refused = signal<string | null>(null);
  /** Czy na ekranie jest mapa gry (a nie lokalny podgląd z panelu debugu). */
  readonly showsGameMap = computed(() => {
    const config = this.config();
    const map = this.store.map();
    return !!config && !!map && sameParams(map.params, config.map);
  });

  /**
   * Start sesji. Tryb otwarty (`connect`, bez lobby): od razu łączy się z `/ws`, a gdy serwer
   * milczy – mapa z domyślnych parametrów. Z lobby nic się nie łączy ani nie generuje, dopóki
   * gracz nie wejdzie do pokoju (`join`) – mapa powstaje wtedy z konfiguracji pokoju.
   */
  start(connect = true): void {
    this.transport.onMessage = (msg) => this.onServer(msg);
    this.transport.onClose = () => this.onClose();
    this.bridge.onGame = (event) => this.onWorker(event);
    // Worker dołącza do zdarzeń gry budynki prowincji z okienka budynku, z ramki (zaznaczonej) albo pod kursorem.
    effect(
      () =>
        this.bridge.watch(
          this.store.buildingPopup()?.province || this.store.selectedProvince() || this.store.hoveredProvince(),
          this.player() ?? NO_PLAYER,
          this.store.attackTargets(),
          this.store.marketWindow() !== 'closed',
          this.store.buildingPopup()?.building === 'port' ? this.store.buildingPopup()!.province : 0,
        ),
      {
      injector: this.injector,
    });
    void this.store.init();
    this.openMode = connect;
    if (!connect) return;
    this.transport.connect();
    setTimeout(() => {
      if (!this.config()) void this.store.showDefault();
    }, OFFLINE_FALLBACK_MS);
  }

  /** Dołącza do pokoju z lobby (albo przechodzi do innego) – `path` daje świeży bilet przy każdej próbie. */
  join(path: PathProvider, playerName: string): void {
    this.resetRoom();
    this.transport.playerName = playerName;
    this.transport.connect(path);
  }

  /** Opuszcza pokój: koniec połączenia (bez ponawiania), gra przestaje być „bieżąca”. */
  leave(): void {
    this.transport.disconnect();
    this.resetRoom();
    this.config.set(null);
    this.player.set(null);
    this.owners.set(null);
    this.phase.set(null);
    this.selection.set(null);
    this.economy.set([]);
    this.tax.set(null);
    this.playingSince.set(null);
    this.provinceReport.set(null);
    this.fronts.set([]);
    this.landings.set([]);
    this.market.set(null);
    this.garrisons.set([]);
    this.port.set(null);
    this.frontMarks.set([]);
    this.bridgeheads.set(null);
    this.buildingLevels.set(null);
    this.constructions.set(null);
    this.factoryProducts.set(null);
  }

  /** Budowa kolejnego poziomu budynku w prowincji (`Intent::Build`, walidacja w rdzeniu). */
  build(province: number, building: Building): void {
    this.transport.send({ type: 'intent', intent: { type: 'build', province, building } });
  }

  /** Przeniesienie stolicy (z centrum dowodzenia) do prowincji (`Intent::MoveCapital`, walidacja w rdzeniu). */
  moveCapital(province: number): void {
    this.transport.send({ type: 'intent', intent: { type: 'moveCapital', province } });
  }

  /** Wojna (`war`) albo pokój z graczem w prowincji (`Intent::SetStance`, walidacja w rdzeniu). */
  setStance(province: number, player: number, war: boolean): void {
    this.transport.send({ type: 'intent', intent: { type: 'setStance', province, player, war } });
  }

  /** Wejście składnika fabryki: `option` – ręcznie ten zamiennik, `null` – automatycznie (`Intent::SetFactoryInput`). */
  setFactoryInput(province: number, ingredient: number, option: number | null): void {
    this.transport.send({ type: 'intent', intent: { type: 'setFactoryInput', province, ingredient, option } });
  }

  /** Rynek: bilans surowca −100…100 – % przepustowości handlu (`Intent::SetTradeResource`). */
  setTradeResource(resource: number, percent: number): void {
    this.transport.send({ type: 'intent', intent: { type: 'setTradeResource', resource, percent: Math.round(percent) } });
  }

  /** Wybór produkcji fabryki w prowincji (`Intent::SetFactory`; zmiana – przezbrojenie 5 dni). */
  setFactory(province: number, supply: Supply): void {
    this.transport.send({ type: 'intent', intent: { type: 'setFactory', province, supply } });
  }

  /** Atak na prowincję: `percent` wojska z kraju idzie na front (`Intent::Attack`; do trwającego – dosłane). */
  /** Atak na prowincje jednym frontem (`Intent::Attack`). */
  attack(provinces: number[], percent: number): void {
    if (provinces.length === 0 || provinces.some((p) => p <= 0)) return;
    this.transport.send({ type: 'intent', intent: { type: 'attack', provinces, percent: Math.round(percent) } });
  }

  /** Odwołanie ataku na prowincję – wojsko wraca do kraju. */
  cancelAttack(province: number): void {
    this.transport.send({ type: 'intent', intent: { type: 'cancelAttack', province } });
  }

  /** Anulowanie budowy w prowincji (zwrot połowy kosztu). */
  cancelBuild(province: number): void {
    this.transport.send({ type: 'intent', intent: { type: 'cancelBuild', province } });
  }

  /** Start gry – serwer przyjmie go tylko od gospodarza. */
  startGame(): void {
    this.transport.send({ type: 'start' });
  }

  /** Nowe ustawienia pokoju – serwer przyjmie je tylko od gospodarza w poczekalni. */
  changeSettings(settings: RoomSettings): void {
    this.transport.send({ type: 'settings', settings });
  }

  /**
   * Wybór prowincji startowej (`Intent::ChooseStart`) w czasie wyboru – skutek widać po wykonaniu
   * najbliższej tury u wszystkich graczy; prowincja z innym graczem – intencja przepada w rdzeniu.
   */
  /**
   * Wybrany start, zanim tura z wyborem wróci z serwera (opóźnienie lockstep) – podpis odliczania
   * pokazuje go od razu; `tick` – tura kliknięcia (po kilkunastu turach liczy się już stan gry).
   */
  readonly pendingStart = signal<{ province: number; tick: number } | null>(null);

  chooseStart(province: number): void {
    this.pendingStart.set({ province, tick: this.tick() ?? 0 });
    this.transport.send({ type: 'intent', intent: { type: 'chooseStart', province } });
  }

  /**
   * Suwak podziału nowej populacji (`Intent::SetTroopShare`): docelowy udział wojska w %. Rzeczywisty
   * podział całej populacji dochodzi do celu z bezwładnością (w rdzeniu, `economy::SHIFT_PPM`).
   */
  setTroopShare(percent: number): void {
    this.transport.send({ type: 'intent', intent: { type: 'setTroopShare', percent: Math.round(percent) } });
  }

  /** Gospodarka tego gracza (null – jeszcze bez państwa). */
  readonly myEconomy = computed(() => {
    const nation = this.myNation();
    return nation ? (this.economy()[nation - 1] ?? null) : null;
  });

  /** Pula tego gracza do receptur: zapasy i bilans na dzień (produkcja − zużycie). */
  readonly myPool = computed((): PoolState | null => {
    const e = this.myEconomy();
    if (!e) return null;
    return {
      stock: e.stock,
      supplies: e.supplies,
      resourceRate: e.production.map((p, r) => p - (e.resourceUse[r] ?? 0)),
      supplyRate: e.supplyProduction.map((p, s) => p - (e.supplyUse[s] ?? 0)),
    };
  });

  /** Numer państwa tego gracza (null – jeszcze nie ma prowincji). */
  readonly myNation = computed(() => {
    const k = this.owners()?.nations.indexOf(this.player() ?? -1) ?? -1;
    return k >= 0 ? k + 1 : null;
  });

  /** Nazwa gracza (z poczekalni) albo „Gracz N” w trybie otwartym. */
  playerName(id: number): string {
    return this.roster().find((p) => p.id === id)?.name ?? `Gracz ${id}`;
  }

  /** Czy graczem steruje komputer: bot albo gracz, który wyszedł z gry (z gry, a przed nią – z poczekalni). */
  isBot(id: number): boolean {
    return this.economy().some((e) => e.player === id && e.bot) || !!this.roster().find((p) => p.id === id)?.bot;
  }

  /** Nowy bot – w poczekalni od gospodarza, w trybie otwartym z `--dev` w każdej chwili (`ClientMsg::AddBot`). */
  addBot(): void {
    this.transport.send({ type: 'addBot' });
  }

  /** Usunięcie bota z poczekalni (tylko gospodarz). */
  removeBot(player: number): void {
    this.transport.send({ type: 'removeBot', player });
  }

  private resetRoom(): void {
    this.roster.set([]);
    this.host.set(null);
    this.started.set(false);
    this.settings.set(null);
    this.refused.set(null);
  }

  private onServer(msg: ServerMsg): void {
    switch (msg.type) {
      case 'welcome': {
        const previous = this.config();
        this.config.set(msg.config);
        this.player.set(msg.player);
        this.serverTick.set(null);
        this.tick.set(null);
        this.hash.set(null);
        this.sentHash.set(null);
        this.desync.set(null);
        this.owners.set(null);
        this.phase.set(null);
        this.selection.set(null);
        this.economy.set([]);
        this.tax.set(null);
        this.playingSince.set(null);
        this.provinceReport.set(null);
        this.fronts.set([]);
        this.landings.set([]);
        this.market.set(null);
        this.garrisons.set([]);
        this.port.set(null);
        this.frontMarks.set([]);
        this.bridgeheads.set(null);
        this.attackCheck.set(null);
        this.buildingLevels.set(null);
        this.constructions.set(null);
        this.factoryProducts.set(null);
        this.readySent = false;
        this.error.set(null);
        // Gra najpierw: tury, które zaraz przyjdą, muszą trafić do workera po `startGame`.
        this.bridge.startGame(msg.config, msg.catchup);
        // Ta sama gra po ponownym połączeniu – mapa na ekranie zostaje (także lokalny podgląd z panelu).
        if (!previous || !sameConfig(previous, msg.config)) this.store.show(msg.config.map);
        break;
      }
      case 'lobby':
        this.roster.set(msg.players);
        this.host.set(msg.host);
        this.started.set(msg.started);
        this.settings.set(msg.settings);
        break;
      case 'refused':
      case 'closed':
        this.refused.set(msg.reason);
        this.transport.disconnect();
        break;
      case 'turn':
        this.serverTick.set(msg.turn.tick);
        this.bridge.turn(msg.turn);
        break;
      case 'desync':
        // Serwer zgłasza każdy rozbieżny hash – wystarczy pierwszy (reszta jest w logu serwera).
        if (this.desync() === null) {
          console.warn(`Desync na ticku ${msg.tick}`);
          this.desync.set(msg.tick);
        }
        break;
    }
  }

  private onWorker(event: GameEvent): void {
    if (event.type === 'gameError') {
      console.error(`Gra zatrzymana: ${event.message}`);
      this.error.set(event.message);
      return;
    }
    this.tick.set(event.tick);
    this.hash.set(event.hash);
    if (event.owners) this.owners.set(event.owners);
    this.phase.set(event.phase);
    this.selection.set(event.phase === 'selection' ? { end: event.selectionEnd, tick: event.tick, at: performance.now() } : null);
    // Gospodarka i raporty przychodzą tylko po zmianie (gospodarka co pół sekundy gry – worker).
    if (event.economy) this.economy.set(event.economy);
    this.tax.set(event.nextTax > 0 ? { at: event.nextTax, tick: event.tick, now: performance.now() } : null);
    this.playingSince.set(event.playingSince > 0 ? event.playingSince : null);
    if (event.province !== undefined) this.provinceReport.set(event.province);
    if (event.attackCheck !== undefined) this.attackCheck.set(event.attackCheck);
    if (event.fronts) this.fronts.set(event.fronts);
    if (event.landings) this.landings.set(event.landings);
    if (event.market !== undefined) this.market.set(event.market);
    if (event.garrisons) this.garrisons.set(event.garrisons);
    if (event.port !== undefined) this.port.set(event.port);
    if (event.frontMarks) this.frontMarks.set(event.frontMarks);
    if (event.bridgeheads) this.bridgeheads.set(event.bridgeheads);
    if (event.buildingLevels) this.buildingLevels.set(event.buildingLevels);
    if (event.constructions) this.constructions.set(event.constructions);
    if (event.factoryProducts) this.factoryProducts.set(event.factoryProducts);
    if (event.changes) this.onOwnerChanges?.(event.changes);
    // Pierwsze zdarzenie gry po `Welcome` = gra zbudowana z mapy i przebieg nadrobiony – gotowi.
    if (!this.readySent) {
      this.readySent = true;
      this.transport.send({ type: 'ready' });
    }
    // Najwyżej `HASHES_PER_SECOND` hashy na sekundę: po zdławionej karcie zdarzenia przychodzą naraz,
    // a serwer rozłącza za zalew wiadomości (`MessageBucket`); do wykrycia desynchronizacji wystarczą.
    const now = performance.now();
    if (now - this.hashWindow.start > 1000) this.hashWindow = { start: now, sent: 0 };
    for (const { tick, hash } of event.hashes) {
      if (this.hashWindow.sent >= HASHES_PER_SECOND) break;
      this.hashWindow.sent++;
      this.transport.send({ type: 'hash', tick, hash });
    }
    const last = event.hashes.at(-1);
    if (last) this.sentHash.set(last);
  }

  private onClose(): void {
    this.player.set(null);
    // Tryb otwarty bez serwera od początku – mapa z domyślnych parametrów (nie czekamy na limit
    // czasu). Z lobby mapa powstaje tylko z konfiguracji pokoju.
    if (this.openMode && !this.config()) void this.store.showDefault();
  }
}
