import { Component, ElementRef, computed, inject, signal, viewChild } from '@angular/core';

import type { Climate } from '../../generated/Climate';
import type { LandAmount } from '../../generated/LandAmount';
import type { MapAspect } from '../../generated/MapAspect';
import type { MapSettings } from '../../generated/MapSettings';
import type { MapSize } from '../../generated/MapSize';
import type { RoomSummary } from '../../generated/RoomSummary';
import { GameSession } from '../game/game-session';
import { Lobby } from '../game/lobby';
import { MapStore } from '../game/map-store';

/** Najwyższy seed (u32). */
const MAX_SEED = 4294967295;
/** Najdłuższa nazwa lobby (`MAX_NAME_CHARS` w `core/src/lobby.rs`). */
const MAX_NAME = 24;
/** Limity graczy do wyboru przy zakładaniu lobby. */
const PLAYER_LIMITS = [2, 3, 4, 6, 8, 12, 16];
/** Uproszczone ustawienia mapy (`MapSettings` w `core/src/lobby.rs`). */
const SIZES: { value: MapSize; label: string; width: number }[] = [
  { value: 'small', label: 'Mała', width: 1000 },
  { value: 'medium', label: 'Średnia', width: 1400 },
  { value: 'large', label: 'Duża', width: 1800 },
];
/** Proporcje mapy (szerokość : wysokość) – pole mapy zależy tylko od rozmiaru. */
const ASPECTS: { value: MapAspect; label: string }[] = [
  { value: 'square', label: '1:1' },
  { value: 'wide', label: '3:2' },
  { value: 'wider', label: '2:1' },
];
const LANDS: { value: LandAmount; label: string }[] = [
  { value: 'islands', label: 'Archipelagi' },
  { value: 'standard', label: 'Standard' },
  { value: 'pangea', label: 'Dużo lądu' },
];
const CLIMATES: { value: Climate; label: string }[] = [
  { value: 'varied', label: 'Zróżnicowany' },
  { value: 'warm', label: 'Ciepły' },
  { value: 'cold', label: 'Chłodny' },
];
const CONTINENTS = [1, 2, 3, 4, 5, 6];

/** Okno ustawień: zakładanie nowego lobby albo zmiana ustawień pokoju przez gospodarza. */
type DialogMode = 'create' | 'edit';

/** Opis mapy, np. „średnia · 3 kontynenty” albo „średnia 2:1 · 3 kontynenty” (proporcje inne niż 1:1). */
function describeMap(map: Pick<MapSettings, 'size' | 'aspect' | 'continents'>): string {
  const n = map.continents;
  let word = 'kontynentów';
  if (n === 1) word = 'kontynent';
  else if (n >= 2 && n <= 4) word = 'kontynenty';
  const size = SIZES.find((s) => s.value === map.size)?.label.toLowerCase() ?? map.size;
  const aspect = map.aspect === 'square' ? '' : ` ${ASPECTS.find((a) => a.value === map.aspect)?.label ?? ''}`;
  return `${size}${aspect} · ${n} ${word}`;
}

/**
 * Ekran powitalny i poczekalnia (z lobby). Bez API lobby (tryb lokalny) – niewidoczny; zanim
 * wiadomo, czy API jest, ekran już zasłania mapę (żeby nie mignął pasek mapy).
 *
 * - **Lista:** nick, liczba osób na stronie, lobby w poczekalni (gracze / limit), „+ Utwórz lobby”
 *   w rogu listy – okno modalne (`<dialog>`, `showModal`: tło przyciemnione, fokus w formularzu, Esc zamyka).
 * - **Poczekalnia:** skład pokoju na żywo, zębatka ustawień (gospodarz – to samo okno co przy
 *   zakładaniu), gospodarz startuje grę; mapa generuje się w tle.
 * - **Gra:** ekran powitalny płynnie znika; zostaje tylko przycisk wyjścia do menu (bez powrotu do poczekalni).
 */
@Component({
  selector: 'app-lobby-panel',
  templateUrl: './lobby-panel.html',
  styleUrls: ['./ui.css', './lobby-panel.css'],
})
export class LobbyPanel {
  protected readonly lobby = inject(Lobby);
  protected readonly session = inject(GameSession);
  private readonly store = inject(MapStore);
  protected readonly maxSeed = MAX_SEED;
  protected readonly limits = PLAYER_LIMITS;
  protected readonly sizes = SIZES;
  protected readonly aspects = ASPECTS;
  protected readonly lands = LANDS;
  protected readonly climates = CLIMATES;
  protected readonly continentCounts = CONTINENTS;
  private readonly settingsDialog = viewChild<ElementRef<HTMLDialogElement>>('settingsDialog');
  private readonly exitDialog = viewChild<ElementRef<HTMLDialogElement>>('exitDialog');
  protected readonly mode = signal<DialogMode>('create');

  /** Ekran powitalny / poczekalnia zasłania mapę (także zanim wiadomo, czy lobby jest dostępne). */
  protected readonly covering = computed(() => this.lobby.available() !== false && this.lobby.screen() !== 'game');
  /** Mapa pokoju gotowa (generuje się w tle już w poczekalni). */
  protected readonly mapReady = computed(
    () =>
      this.session.showsGameMap() &&
      !this.store.busy() &&
      !this.store.provincesPending() &&
      !this.store.polishPending(),
  );
  protected readonly hasName = computed(() => this.lobby.playerName().trim().length > 0);
  /** Wyjście tego gracza zamknie lobby (gospodarz w poczekalni). */
  protected readonly closesRoom = computed(() => this.session.isHost() && !this.session.started());
  /** Opis pokoju w poczekalni – z bieżących ustawień z serwera (gospodarz mógł je zmienić). */
  protected readonly roomInfo = computed(() => {
    const room = this.lobby.current();
    const settings = this.session.settings();
    if (!room) return null;
    if (!settings) return { name: room.name, map: this.mapLabel(room), seed: room.seed, max: room.maxPlayers };
    return {
      name: settings.name,
      map: describeMap(settings.map),
      seed: settings.seed,
      max: settings.maxPlayers,
    };
  });

  /** Opis mapy na liście, np. „średnia · 3 kontynenty”. */
  protected mapLabel(room: RoomSummary): string {
    // Starsza Lambda nie przysyła `map` – wtedy rozmiar z szerokości (mapy były kwadratowe).
    const map = (room.map as MapSettings | undefined) ?? {
      size: SIZES.find((s) => s.width === room.mapWidth)?.value ?? 'medium',
      aspect: 'square',
      continents: room.continents,
    };
    return describeMap(map);
  }

  /** Polska odmiana: 1 osoba, 2–4 osoby (bez 12–14), 5+ osób. */
  protected people(n: number): string {
    if (n === 1) return 'osoba';
    const few = n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14);
    return few ? 'osoby' : 'osób';
  }

  protected setName(event: Event): void {
    this.lobby.setName((event.target as HTMLInputElement).value);
  }

  /** Nowe lobby: domyślne ustawienia, nazwa „<nick> lobby”. */
  protected openCreate(): void {
    const name = `${this.lobby.playerName().trim()} lobby`.slice(0, MAX_NAME);
    this.openDialog('create', {
      room: name,
      size: 'medium',
      aspect: 'square',
      continents: '3',
      land: 'standard',
      climate: 'varied',
    });
    this.fill({ max: '8', seed: '' });
  }

  /** Zębatka w poczekalni: bieżące ustawienia pokoju do zmiany (tylko gospodarz). */
  protected openSettings(): void {
    const s = this.session.settings();
    if (!s) return;
    const { size, aspect, continents, land, climate } = s.map;
    this.openDialog('edit', { room: s.name, size, aspect, continents: String(continents), land, climate });
    this.fill({ max: String(s.maxPlayers), seed: String(s.seed) });
  }

  protected closeDialog(): void {
    this.settingsDialog()?.nativeElement.close();
  }

  protected submit(event: SubmitEvent): void {
    event.preventDefault();
    const data = new FormData(event.target as HTMLFormElement);
    const text = (field: string) => {
      const value = data.get(field);
      return typeof value === 'string' ? value.trim() : '';
    };
    const name = text('room');
    const seedText = text('seed');
    const seed = seedText === '' ? null : Number(seedText);
    const maxPlayers = Number(text('max'));
    if (!name || (seed !== null && (!Number.isInteger(seed) || seed < 0 || seed > MAX_SEED))) return;
    const map: MapSettings = {
      size: text('size') as MapSize,
      aspect: text('aspect') as MapAspect,
      continents: Number(text('continents')),
      land: text('land') as LandAmount,
      climate: text('climate') as Climate,
    };
    if (this.mode() === 'edit') {
      this.lobby.changeSettings(name, seed, maxPlayers, map);
      this.closeDialog();
      return;
    }
    void this.lobby.create(name, seed, maxPlayers, map).then((ok) => {
      if (ok) this.closeDialog();
    });
  }

  protected openExit(): void {
    this.exitDialog()?.nativeElement.showModal();
  }

  protected closeExit(): void {
    this.exitDialog()?.nativeElement.close();
  }

  /** Wyjście z gry do menu głównego – bez powrotu do tej rozgrywki. */
  protected exitGame(): void {
    this.closeExit();
    this.lobby.leave();
  }

  private openDialog(mode: DialogMode, values: Record<string, string>): void {
    this.mode.set(mode);
    this.lobby.error.set(null);
    this.fill(values);
    this.settingsDialog()?.nativeElement.showModal();
  }

  /** Wpisuje wartości do pól formularza okna ustawień (po nazwach pól). */
  private fill(values: Record<string, string>): void {
    const form = this.settingsDialog()?.nativeElement.querySelector('form');
    for (const [field, value] of Object.entries(values)) {
      const el = form?.elements.namedItem(field);
      if (el instanceof HTMLInputElement || el instanceof HTMLSelectElement) el.value = value;
    }
  }
}
