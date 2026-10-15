/**
 * Odblokowania (docs/gdd/budynki.md) – tabele w oknie zapasów. Liczby odpowiadają stałym rdzenia:
 * `war::MAX_ATTACKS`, `war::RIVER_CROSSING_LEVEL`, `buildings::FORTIFICATION_CAP`,
 * `factories::COMMAND_LEVEL`, `factories::UNIVERSITY_BONUS`, `factories::SYNTHETIC_FUEL_UNIVERSITIES`.
 */
export const COMMAND_UNLOCKS: readonly { level: number; items: readonly string[] }[] = [
  { level: 1, items: ['Lekkie zaopatrzenie wojskowe', '1 atak naraz'] },
  { level: 2, items: ['Standardowe zaopatrzenie wojskowe', 'Przeprawy przez rzeki', '3 ataki naraz'] },
  { level: 3, items: ['Umocnienia do poziomu 1', '5 ataków naraz', 'Desant morski, tunele i mosty (w przyszłości)'] },
  { level: 4, items: ['Ciężkie zaopatrzenie wojskowe'] },
  { level: 5, items: ['Umocnienia do poziomu 3', '8 ataków naraz'] },
  { level: 6, items: ['Broń jądrowa (w przyszłości)'] },
];

export const UNIVERSITY_UNLOCKS: readonly { level: number; items: readonly string[] }[] = [
  { level: 5, items: ['Fabryki +10% produkcji'] },
  { level: 10, items: ['Paliwo syntetyczne z węgla'] },
  { level: 15, items: ['Kauczuk syntetyczny z ropy (w przyszłości)'] },
  { level: 20, items: ['Fabryki +20% produkcji'] },
  { level: 25, items: ['Dodatkowe zamienniki w recepturach (w przyszłości)'] },
  { level: 40, items: ['Wzbogacanie uranu (w przyszłości)'] },
];
