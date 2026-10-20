/**
 * Odblokowania (docs/gdd/budynki.md) – tabele w oknie zapasów. Liczby odpowiadają stałym rdzenia:
 * `war::MAX_FRONTS`, `war::MAX_FRONT_PROVINCES`, `war::MAX_SEA_LANDINGS`,
 * `war::LANDING_TROOPS` (według poziomu portu), `war::RIVER_CROSSING_TROOPS` / `MOUNTAIN_CROSSING_TROOPS`,
 * `war::RIVER_PREP_TICKS` / `MOUNTAIN_PREP_TICKS`, `war::MILITARY`,
 * `buildings::FORTIFICATION_CAP`, `factories::COMMAND_LEVEL`, `factories::UNIVERSITY_BONUS`,
 * `factories::SYNTHETIC_FUEL_UNIVERSITIES`.
 */
export const COMMAND_UNLOCKS: readonly { level: number; items: readonly string[] }[] = [
  {
    level: 1,
    items: [
      'Lekkie zaopatrzenie wojskowe',
      '2 fronty naraz, po 1 prowincji',
      'Przeprawy przez rzeki (do 20 tys. wojska na froncie, przygotowanie 0,8 dnia) i góry (do 8 tys., 1,5 dnia + przejście) – front, który styka się z celem tylko przez nie',
      'Desant przez jeziora',
      'Desant morski z portu (1 na morzu naraz, zawsze przyczółek; wojska według poziomu portu: 5 / 15 / 40 tys., paliwo na rejs z góry)',
    ],
  },
  {
    level: 2,
    items: [
      'Standardowe zaopatrzenie wojskowe',
      '2 fronty, do 2 prowincji na froncie (Ctrl + klik)',
      'Umocnienia do poziomu 1',
      'Przeprawy: rzeki do 30 tys., góry do 12 tys. wojska',
    ],
  },
  {
    level: 3,
    items: ['3 fronty, do 3 prowincji na froncie', 'Desanty morskie bez limitu naraz', 'Umocnienia do poziomu 2', 'Lekkie zaopatrzenie: siła 0,6 → 0,8, 40% mniej paliwa', 'Przeprawy: rzeki do 50 tys., góry do 20 tys.'],
  },
  {
    level: 4,
    items: ['Ciężkie zaopatrzenie wojskowe', 'Fronty bez limitu, do 4 prowincji na froncie', 'Standardowe zaopatrzenie: siła 1,0 → 1,2, 40% mniej paliwa', 'Przeprawy: rzeki do 80 tys., góry do 30 tys.'],
  },
  { level: 5, items: ['Fronty i prowincje na froncie bez limitu', 'Umocnienia do poziomu 3', 'Przeprawy: rzeki do 120 tys., góry do 45 tys.'] },
  { level: 6, items: ['Ciężkie zaopatrzenie: siła 1,4 → 1,6, 40% mniej paliwa', 'Przeprawy: rzeki do 200 tys., góry do 70 tys.', 'Broń jądrowa (w przyszłości)'] },
];

/** Limity frontów według poziomu centrum dowodzenia (`war::MAX_FRONTS`, `MAX_FRONT_PROVINCES`). */
export const COMMAND_LIMITS: readonly { fronts: number; provinces: number }[] = [
  { fronts: 2, provinces: 1 },
  { fronts: 2, provinces: 1 },
  { fronts: 2, provinces: 2 },
  { fronts: 3, provinces: 3 },
  { fronts: Infinity, provinces: 4 },
  { fronts: Infinity, provinces: Infinity },
  { fronts: Infinity, provinces: Infinity },
];

export const UNIVERSITY_UNLOCKS: readonly { level: number; items: readonly string[] }[] = [
  { level: 5, items: ['Fabryki +10% produkcji'] },
  { level: 10, items: ['Paliwo syntetyczne z węgla'] },
  { level: 15, items: ['Kauczuk syntetyczny z ropy (w przyszłości)'] },
  { level: 20, items: ['Fabryki +20% produkcji'] },
  { level: 25, items: ['Dodatkowe zamienniki w recepturach (w przyszłości)'] },
  { level: 40, items: ['Wzbogacanie uranu (w przyszłości)'] },
];

/** Najwięcej wojska na desancie według poziomu portu wypłynięcia (`war::LANDING_TROOPS`, indeks = poziom). */
export const PORT_LANDING_TROOPS: readonly number[] = [0, 5_000, 15_000, 40_000];
