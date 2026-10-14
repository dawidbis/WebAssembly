/** Klucze tożsamości karty i nicku – wspólne dla lobby i trybu otwartego. */
export const NAME_KEY = 'mapa.playerName';
export const CLIENT_KEY = 'mapa.clientId';

/**
 * Wartość z `storage` pod `key`; brak – `fallback()` zapisane na przyszłość. Bez dostępu do storage
 * (tryb prywatny, zablokowane dane) – wartość tylko na tę sesję.
 */
export function stored(storage: () => Storage, key: string, fallback: () => string): string {
  try {
    const value = storage().getItem(key);
    if (value) return value;
    const fresh = fallback();
    storage().setItem(key, fresh);
    return fresh;
  } catch {
    return fallback();
  }
}

let fallbackTabId: string | undefined;

/**
 * Losowe ID karty (nie konto), trwałe w obrębie karty (`sessionStorage`) – serwer po nim rozpoznaje
 * gracza, który wraca po zerwanym połączeniu, i oddaje mu to samo ID gracza (jego państwo).
 */
export function tabId(): string {
  return stored(() => sessionStorage, CLIENT_KEY, () => (fallbackTabId ??= crypto.randomUUID()));
}

/** Zapisany nick gracza (z ekranu powitalnego lobby) albo `null`. */
export function storedName(): string | null {
  try {
    return localStorage.getItem(NAME_KEY);
  } catch {
    return null;
  }
}
