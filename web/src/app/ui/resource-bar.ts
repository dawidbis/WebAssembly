import { Component, DestroyRef, computed, effect, inject, signal } from '@angular/core';

import { GameSession } from '../game/game-session';
import { flagDataUrl } from '../render/flags';
import { nationCss } from '../render/nations';
import { MapStore } from '../game/map-store';
import { RESOURCES } from '../render/resources';
import { gameDate, ticksPerDay } from './calendar';
import { amount, full, short } from './format';

/** Złoto w interfejsie jest dzielone przez tyle (GDD). */
const GOLD_UNIT = 10;
/** Złoto – numer surowca (`GOLD` w rdzeniu), po nazwie. */
const GOLD = RESOURCES.findIndex((r) => r.name === 'Złoto');
/** Domyślny udział wojska (`economy::DEFAULT_TROOP_SHARE`). */
const DEFAULT_TROOP_SHARE = 50;
/** Tura w ms (`TURN_MS` w `server/src/room.rs`); podatek co 300 tur (`economy::TAX_TICKS`). */
const TURN_MS = 100;
const TAX_MS = 30_000;

/** Dymek z wpływem podatku unosi się i znika po tylu ms (animacja `tax-float` w resource-bar.css). */
const BUBBLE_MS = 1800;

/**
 * Pasek zasobów gracza (środek górnego paska): flaga i nazwa państwa, populacja z limitem
 * i przyrostem, podział populacji na wojsko i robotników (suwak z bezwładnością – ludzie przechodzą
 * między grupami stopniowo, w rdzeniu) i złoto z kółkiem postępu do podatku (wpływ – unoszący się dymek).
 * Liczby skrócone (k / m), pełne w podpowiedziach. Widoczny, gdy gracz ma państwo.
 */
@Component({
  selector: 'app-resource-bar',
  templateUrl: './resource-bar.html',
  styleUrls: ['./ui.css', './resource-bar.css'],
})
export class ResourceBar {
  protected readonly session = inject(GameSession);
  protected readonly store = inject(MapStore);
  /** Cel suwaka w trakcie przeciągania (zanim wróci z rdzenia w raporcie). */
  protected readonly dragging = signal<number | null>(null);
  /** Cel wysłany do rdzenia, a jeszcze niepotwierdzony raportem (suwak nie odskakuje na chwilę). */
  private readonly pending = signal<number | null>(null);
  /** Bieżący miesiąc gry (długość dnia do przyrostu „na dzień”) – zmienia się co 30 s. */
  private readonly month = computed(() => {
    const since = this.session.playingSince();
    const tick = this.session.tick();
    return since === null || tick === null ? 0 : gameDate(Math.max(0, tick - since)).month;
  });
  /** Dymki z wpływami podatku (każdy żyje `BUBBLE_MS`). */
  protected readonly bubbles = signal<{ id: number; text: string }[]>([]);
  private bubbleId = 0;
  /** Zegar odliczania podatku (co 250 ms wystarczy – sekundy). */
  private readonly now = signal(performance.now());

  protected readonly view = computed(() => {
    const e = this.session.myEconomy();
    const nation = this.session.myNation();
    if (!e || !nation) return this.preStart();
    const target = this.dragging() ?? this.pending() ?? e.targetShare;
    const fill = e.maxPopulation > 0 ? Math.min(1, e.population / e.maxPopulation) : 0;
    // Przyrost na dzień gry: na turę (raport podaje na sekundę = 10 tur) × tur na dzień w tym miesiącu.
    const growth = Math.round((e.growthPerSecond / 10) * ticksPerDay(this.month()));
    return {
      pre: false as const,
      name: this.session.playerName(e.player),
      flag: flagDataUrl(nation, 36, 24),
      color: nationCss(nation),
      population: short(e.population),
      maxPopulation: short(e.maxPopulation),
      fill: fill * 100,
      growth: `${growth >= 0 ? '+' : '−'}${short(Math.abs(growth))}/d`,
      shrinking: growth < 0,
      troops: short(e.troops),
      workers: short(e.workers),
      // Robotnicy w budynkach (obsada, najwyżej wszyscy robotnicy) – część paska robotników.
      staffedPct: e.workers > 0 ? ((100 - e.troopShare) * Math.min(e.workers, e.staff)) / e.workers : 0,
      share: e.troopShare,
      target,
      moving: target !== e.troopShare,
      gold: short(Math.floor(e.gold / GOLD_UNIT)),
      tax: this.taxCountdown(),
      // Pełne liczby w podpowiedziach (na pasku – skrócone k / m).
      popTitle:
        `Populacja ${full(e.population)} / limit ${full(e.maxPopulation)} (100 osób na kafel), przyrost na dzień gry. Najszybciej rośnie przy 55% limitu.` +
        `\nProwiant: zaspokojenie ${Math.round(e.foodCoverage * 100)}% (potrzeba ${amount(e.foodNeed)}/d) – przyrost ${Math.round(150 * e.foodCoverage)}%` +
        (e.foodCoverage < 1 ? ', część ludności głoduje (ubywa)' : ''),
      basicShort: e.foodCoverage < 1,
      troopsTitle:
        `Wojsko w kraju: ${full(e.troops)} – obrona i nowe ataki (nie płaci podatków)` +
        (e.deployed > 0 ? `
W atakach: ${full(e.deployed)} (${e.attacks.length})` : ''),
      deployed: e.deployed > 0 ? short(e.deployed) : '',
      workersTitle:
        `Robotnicy: ${full(e.workers)} – obsadzają budynki i płacą podatki` +
        (e.staff > 0 ? `
W budynkach: ${full(Math.min(e.workers, e.staff))} / ${full(e.staff)} potrzebnych, pokrycie ${Math.round(e.staffCoverage * 100)}% (mnoży produkcję)` : ''),
      understaffed: e.staffCoverage < 1,
      goldTitle:
        `Złoto: ${full(Math.floor(e.gold / GOLD_UNIT))}. Następny podatek: +${full(Math.floor(e.nextTax / GOLD_UNIT))} (do 0,1 złota od robotnika na początku miesiąca)` +
        `\nPodstawowe zaopatrzenie cywilne: podatek ${Math.round(e.basicTax * 100)}% (średnia z miesiąca: 50% bez niego, 100% przy pełnym – potrzeba ${amount(e.basicNeed)}/d)` +
        `\nLuksusowe zaopatrzenie cywilne: premia +${Math.round(e.taxBonus * 100)}% (średnia z miesiąca, do +50%; działa tylko przy pełnym podstawowym – potrzeba ${amount(e.luxuryNeed)}/d)`,
    };
  });

  /**
   * Wybór startu, a gracz nie ma jeszcze prowincji: sam suwak podziału – cel czeka w rdzeniu
   * (`pending_share`) i dzieli populację od razu, gdy powstanie państwo.
   */
  private preStart() {
    if (this.session.phase() !== 'selection') return null;
    const target = this.dragging() ?? this.pending() ?? DEFAULT_TROOP_SHARE;
    return { pre: true as const, share: target, target, moving: false };
  }

  constructor() {
    const timer = setInterval(() => this.now.set(performance.now()), 250);
    effect(() => {
      if (this.session.myEconomy()?.targetShare === this.pending()) this.pending.set(null);
    });
    // Podatek wpłynął – zielony dymek „+333” nad złotem, unosi się i znika.
    let lastGold: number | null = null;
    effect(() => {
      const gold = this.session.myEconomy()?.gold ?? null;
      const gain = gold !== null && lastGold !== null ? Math.floor(gold / GOLD_UNIT) - Math.floor(lastGold / GOLD_UNIT) : 0;
      lastGold = gold;
      if (gain <= 0) return;
      const bubble = { id: ++this.bubbleId, text: `+${short(gain)}` };
      this.bubbles.update((list) => [...list, bubble]);
      setTimeout(() => this.bubbles.update((list) => list.filter((b) => b !== bubble)), BUBBLE_MS);
    });
    inject(DestroyRef).onDestroy(() => clearInterval(timer));
  }

  /** Postęp okresu podatku 0..1 – z ticku podatku, płynnie między turami. */
  private taxCountdown(): { progress: number } | null {
    const t = this.session.tax();
    if (!t) return null;
    const left = Math.max(0, (t.at - t.tick) * TURN_MS - (this.now() - t.now));
    return { progress: 1 - left / TAX_MS };
  }

  /** Liczba rodzajów surowców w puli albo wydobywanych (złoto osobno – trafia od razu do skarbca). */
  protected readonly kinds = computed(() => {
    const e = this.session.myEconomy();
    if (!e) return 0;
    return RESOURCES.filter((_, r) => r !== GOLD && ((e.stock[r] ?? 0) > 0 || (e.production[r] ?? 0) > 0)).length;
  });

  /** Przycisk ze skrzynką otwiera okno zapasów (`ui/stock-window`) albo je zamyka. */
  /** Okno państwa na zakładce `tab` (drugie kliknięcie tej samej – zamyka). */
  protected openNation(tab: 'finance' | 'population'): void {
    this.store.nationWindow.update((w) => (w === tab ? 'closed' : tab));
  }

  protected toggleMarket(): void {
    this.store.marketWindow.update((w) => (w === 'closed' ? 'open' : 'closed'));
  }

  protected toggleStock(): void {
    this.store.stockWindow.update((w) => (w === 'closed' ? 'open' : 'closed'));
  }

  protected drag(event: Event): void {
    this.dragging.set(Number((event.target as HTMLInputElement).value));
  }

  /** Puszczenie suwaka: cel idzie do rdzenia jako intencja (rzeczywisty podział dojdzie z bezwładnością). */
  protected commit(event: Event): void {
    const value = Number((event.target as HTMLInputElement).value);
    this.session.setTroopShare(value);
    this.pending.set(value);
    this.dragging.set(null);
  }
}
