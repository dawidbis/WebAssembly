import { Component, ElementRef, computed, inject, viewChild } from '@angular/core';

import type { Climate } from '../../generated/Climate';
import type { LandAmount } from '../../generated/LandAmount';
import type { MapSize } from '../../generated/MapSize';
import type { RoomSummary } from '../../generated/RoomSummary';
import { GameSession } from '../game/game-session';
import { Lobby } from '../game/lobby';
import { MapStore } from '../game/map-store';

/** Najwyższy seed (u32). */
const MAX_SEED = 4294967295;
/** Limity graczy do wyboru przy zakładaniu lobby. */
const PLAYER_LIMITS = [2, 3, 4, 6, 8, 12, 16];
/** Uproszczone ustawienia mapy (`MapSettings` w `core/src/lobby.rs`). */
const SIZES: { value: MapSize; label: string; width: number }[] = [
  { value: 'small', label: 'Mała', width: 1000 },
  { value: 'medium', label: 'Średnia', width: 1400 },
  { value: 'large', label: 'Duża', width: 1800 },
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

/**
 * Ekran powitalny i poczekalnia (z lobby). Bez API lobby (tryb lokalny) – niewidoczny.
 *
 * - **Lista:** nick, liczba osób na stronie, lobby w poczekalni (gracze / limit), „+ Utwórz lobby”
 *   w rogu listy – okno modalne (`<dialog>`, `showModal`: tło przyciemnione, fokus w formularzu, Esc zamyka).
 * - **Poczekalnia:** skład pokoju na żywo, gospodarz startuje grę; mapa generuje się w tle.
 * - **Gra:** zwinięte do przycisku z nazwą pokoju.
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
  protected readonly lands = LANDS;
  protected readonly climates = CLIMATES;
  protected readonly continentCounts = CONTINENTS;
  private readonly createDialog = viewChild<ElementRef<HTMLDialogElement>>('createDialog');

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

  /** Opis mapy na liście, np. „średnia · 3 kontynenty”. */
  protected mapLabel(room: RoomSummary): string {
    const size = SIZES.find((s) => s.width === room.mapWidth)?.label.toLowerCase() ?? `${room.mapWidth} kafli`;
    const n = room.continents;
    const word = n === 1 ? 'kontynent' : n >= 2 && n <= 4 ? 'kontynenty' : 'kontynentów';
    return `${size} · ${n} ${word}`;
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

  protected openCreate(): void {
    this.lobby.error.set(null);
    this.createDialog()?.nativeElement.showModal();
  }

  protected closeCreate(): void {
    this.createDialog()?.nativeElement.close();
  }

  protected create(event: SubmitEvent): void {
    event.preventDefault();
    const form = event.target as HTMLFormElement;
    const data = new FormData(form);
    const text = (field: string) => {
      const value = data.get(field);
      return typeof value === 'string' ? value.trim() : '';
    };
    const name = text('room');
    const seedText = text('seed');
    const seed = seedText === '' ? null : Number(seedText);
    const maxPlayers = Number(text('max'));
    if (!name || (seed !== null && (!Number.isInteger(seed) || seed < 0 || seed > MAX_SEED))) return;
    const map = {
      size: text('size') as MapSize,
      continents: Number(text('continents')),
      land: text('land') as LandAmount,
      climate: text('climate') as Climate,
    };
    void this.lobby.create(name, seed, maxPlayers, map).then((ok) => {
      if (!ok) return;
      form.reset();
      this.closeCreate();
    });
  }
}
