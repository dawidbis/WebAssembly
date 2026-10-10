import { Injectable, inject, signal } from '@angular/core';

import type { ApiError } from '../../generated/ApiError';
import type { CreateRoom } from '../../generated/CreateRoom';
import type { JoinResponse } from '../../generated/JoinResponse';
import type { RoomSummary } from '../../generated/RoomSummary';
import { GameSession } from './game-session';

const NAME_KEY = 'mapa.playerName';
/** Co tyle lista pokoi odświeża się, gdy lobby jest otwarte. */
const REFRESH_MS = 5000;

function storedName(): string {
  try {
    const name = localStorage.getItem(NAME_KEY);
    if (name) return name;
  } catch {
    // Brak dostępu do localStorage (tryb prywatny, zablokowane dane) – nazwa domyślna.
  }
  return `Gracz ${Math.floor(Math.random() * 900) + 100}`;
}

/**
 * Lobby (meta-serwer pod `/api`): lista pokoi, tworzenie, dołączanie. Gdy API nie ma (lokalnie
 * `ng serve`, serwer bez lobby) – `available` = false i gra łączy się od razu w trybie otwartym.
 */
@Injectable({ providedIn: 'root' })
export class Lobby {
  private readonly session = inject(GameSession);
  private refresh?: ReturnType<typeof setInterval>;

  /** null = jeszcze nie wiadomo. */
  readonly available = signal<boolean | null>(null);
  readonly open = signal(false);
  readonly rooms = signal<RoomSummary[]>([]);
  /** Pokój, w którym gracz jest (albo do którego dołącza). */
  readonly current = signal<RoomSummary | null>(null);
  readonly playerName = signal(storedName());
  readonly error = signal<string | null>(null);
  readonly busy = signal(false);

  /** Sprawdza API: jest – sesja startuje bez łączenia i otwiera się lobby; nie ma – tryb otwarty. */
  async start(): Promise<void> {
    try {
      this.rooms.set(await this.list());
      this.available.set(true);
      this.session.start(false);
      this.show();
    } catch {
      this.available.set(false);
      this.session.start();
    }
  }

  show(): void {
    this.open.set(true);
    clearInterval(this.refresh);
    this.refresh = setInterval(() => void this.reload(), REFRESH_MS);
    void this.reload();
  }

  hide(): void {
    this.open.set(false);
    clearInterval(this.refresh);
  }

  setName(name: string): void {
    this.playerName.set(name);
    try {
      localStorage.setItem(NAME_KEY, name);
    } catch {
      // Nazwa zostaje tylko na tę sesję.
    }
  }

  async reload(): Promise<void> {
    try {
      this.rooms.set(await this.list());
    } catch (e) {
      this.error.set(message(e));
    }
  }

  async create(name: string, seed: number | null): Promise<void> {
    const body: CreateRoom = { name, ...(seed === null ? {} : { seed }) };
    const room = await this.run(() => this.request<RoomSummary>('POST', '/api/rooms', body));
    if (room) await this.join(room);
  }

  async join(room: RoomSummary): Promise<void> {
    const playerName = this.playerName();
    // Pierwszy bilet od razu (błędy, np. pełny pokój, widać w lobby); kolejne – przy każdym ponownym łączeniu.
    const first = await this.run(() => this.ticket(room.id, playerName));
    if (!first) return;
    let next: string | null = first.wsPath;
    this.current.set(first.room);
    this.session.join(async () => {
      if (next) {
        const path = next;
        next = null;
        return path;
      }
      return (await this.ticket(room.id, playerName)).wsPath;
    }, playerName);
    this.hide();
  }

  private ticket(id: string, playerName: string): Promise<JoinResponse> {
    return this.request<JoinResponse>('POST', `/api/rooms/${encodeURIComponent(id)}/join`, { playerName });
  }

  private list(): Promise<RoomSummary[]> {
    return this.request<RoomSummary[]>('GET', '/api/rooms');
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
