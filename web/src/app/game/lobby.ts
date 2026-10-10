import { Injectable, computed, effect, inject, signal, untracked } from '@angular/core';

import type { ApiError } from '../../generated/ApiError';
import type { CreateRoom } from '../../generated/CreateRoom';
import type { JoinResponse } from '../../generated/JoinResponse';
import type { Presence } from '../../generated/Presence';
import type { RoomSummary } from '../../generated/RoomSummary';
import { GameSession } from './game-session';

const NAME_KEY = 'mapa.playerName';
const CLIENT_KEY = 'mapa.clientId';
/** Co tyle lista lobby odświeża się, gdy jest na ekranie (heartbeat serwera co 5 s). */
const ROOMS_EVERY_MS = 5000;
/** Co tyle karta zgłasza obecność (`PRESENCE_EVERY_SECS` w `core/src/lobby.rs`). */
const PRESENCE_EVERY_MS = 15000;

/** Ekran lobby: lista pokoi, poczekalnia pokoju albo gra (lobby zwinięte do przycisku). */
export type LobbyScreen = 'list' | 'room' | 'game';

function stored(storage: () => Storage, key: string, fallback: () => string): string {
  try {
    const value = storage().getItem(key);
    if (value) return value;
    const fresh = fallback();
    storage().setItem(key, fresh);
    return fresh;
  } catch {
    // Brak dostępu do storage (tryb prywatny, zablokowane dane) – wartość tylko na tę sesję.
    return fallback();
  }
}

/**
 * Lobby (meta-serwer pod `/api`): nick, liczba osób na stronie, lista pokoi w poczekalni, zakładanie,
 * poczekalnia pokoju (skład na żywo z game-servera) i start gry przez gospodarza. Gdy API nie ma
 * (lokalnie `ng serve`, serwer bez lobby) – `available` = false i gra łączy się od razu w trybie otwartym.
 */
@Injectable({ providedIn: 'root' })
export class Lobby {
  private readonly session = inject(GameSession);
  private roomsTimer?: ReturnType<typeof setInterval>;

  /** null = jeszcze nie wiadomo. */
  readonly available = signal<boolean | null>(null);
  readonly rooms = signal<RoomSummary[]>([]);
  /** Liczba kart na stronie (z `/api/presence`). */
  readonly online = signal<number | null>(null);
  /** Pokój, w którym gracz jest (poczekalnia albo gra). */
  readonly current = signal<RoomSummary | null>(null);
  readonly playerName = signal(stored(() => localStorage, NAME_KEY, () => ''));
  /** ID karty – ta sama karta wraca do trwającej gry po zerwanym połączeniu. */
  readonly clientId = stored(() => sessionStorage, CLIENT_KEY, () => crypto.randomUUID());
  readonly error = signal<string | null>(null);
  readonly busy = signal(false);
  /** W trakcie gry gracz może podejrzeć skład pokoju (przycisk pokoju) – bez wychodzenia z gry. */
  readonly roomOpen = signal(false);

  readonly screen = computed<LobbyScreen>(() => {
    if (!this.current()) return 'list';
    return this.session.started() && !this.roomOpen() ? 'game' : 'room';
  });

  constructor() {
    // Lista pokoi odświeża się tylko, gdy jest na ekranie.
    effect(() => {
      const listing = this.available() === true && this.screen() === 'list';
      untracked(() => {
        clearInterval(this.roomsTimer);
        if (!listing) return;
        void this.reload();
        this.roomsTimer = setInterval(() => void this.reload(), ROOMS_EVERY_MS);
      });
    });
    // Serwer nie wpuścił (pokój pełny, gra trwa bez nas) – z powrotem do listy z komunikatem.
    effect(() => {
      const reason = this.session.refused();
      if (!reason) return;
      untracked(() => {
        this.current.set(null);
        this.error.set(reason);
      });
    });
    // Start gry: poczekalnia znika, widać mapę.
    effect(() => {
      if (this.session.started()) untracked(() => this.roomOpen.set(false));
    });
  }

  /** Sprawdza API: jest – ekran powitalny (nic się nie generuje); nie ma – tryb otwarty. */
  async start(): Promise<void> {
    try {
      this.rooms.set(await this.request<RoomSummary[]>('GET', '/api/rooms'));
      this.available.set(true);
      this.session.start(false);
      void this.heartbeat();
      setInterval(() => void this.heartbeat(), PRESENCE_EVERY_MS);
    } catch {
      this.available.set(false);
      this.session.start();
    }
  }

  setName(name: string): void {
    this.playerName.set(name);
    try {
      localStorage.setItem(NAME_KEY, name);
    } catch {
      // Nick zostaje tylko na tę sesję.
    }
  }

  async reload(): Promise<void> {
    try {
      this.rooms.set(await this.request<RoomSummary[]>('GET', '/api/rooms'));
    } catch (e) {
      this.error.set(message(e));
    }
  }

  /** Zakłada pokój i od razu do niego dołącza (pierwszy gracz = gospodarz). */
  async create(name: string, seed: number | null, maxPlayers: number): Promise<boolean> {
    if (!this.requireName()) return false;
    const body: CreateRoom = { name, maxPlayers, ...(seed === null ? {} : { seed }) };
    const room = await this.run(() => this.request<RoomSummary>('POST', '/api/rooms', body));
    return !!room && (await this.join(room));
  }

  async join(room: RoomSummary): Promise<boolean> {
    if (!this.requireName()) return false;
    const playerName = this.playerName().trim();
    // Pierwszy bilet od razu (błędy, np. pełny pokój, widać w lobby); kolejne – przy każdym ponownym łączeniu.
    const first = await this.run(() => this.ticket(room.id, playerName));
    if (!first) return false;
    let next: string | null = first.wsPath;
    this.current.set(first.room);
    this.roomOpen.set(false);
    this.session.join(async () => {
      if (next) {
        const path = next;
        next = null;
        return path;
      }
      return (await this.ticket(room.id, playerName)).wsPath;
    }, playerName);
    return true;
  }

  /** Opuszcza pokój (poczekalnię albo grę) i wraca do listy. */
  leave(): void {
    this.session.leave();
    this.current.set(null);
    this.roomOpen.set(false);
    this.error.set(null);
  }

  startGame(): void {
    this.session.startGame();
  }

  private requireName(): boolean {
    if (this.playerName().trim()) return true;
    this.error.set('Najpierw wpisz swój nick.');
    return false;
  }

  private async heartbeat(): Promise<void> {
    try {
      const presence = await this.request<Presence>('POST', '/api/presence', { clientId: this.clientId });
      this.online.set(presence.online);
    } catch {
      // Obecność to tylko informacja – błąd nie przeszkadza w grze.
    }
  }

  private ticket(id: string, playerName: string): Promise<JoinResponse> {
    const body = { playerName, clientId: this.clientId };
    return this.request<JoinResponse>('POST', `/api/rooms/${encodeURIComponent(id)}/join`, body);
  }

  /** Wywołanie z `busy` i komunikatem błędu w lobby; `null` przy błędzie. */
  private async run<T>(fn: () => Promise<T>): Promise<T | null> {
    this.busy.set(true);
    this.error.set(null);
    try {
      return await fn();
    } catch (e) {
      this.error.set(message(e));
      return null;
    } finally {
      this.busy.set(false);
    }
  }

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const res = await fetch(path, {
      method,
      headers: body === undefined ? {} : { 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    // Bez lobby (ng serve, CloudFront bez /api) przychodzi index.html – to nie jest API.
    if (!res.headers.get('content-type')?.includes('application/json')) throw new Error('lobby niedostępne');
    const json = await res.json();
    if (!res.ok) throw new Error((json as ApiError).error ?? `błąd ${res.status}`);
    return json as T;
  }
}

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
