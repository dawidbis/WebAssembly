/**
 * Odblokowania (docs/gdd/budynki.md) – tabele w oknie zapasów. Liczby odpowiadają stałym rdzenia:
 * `war::MAX_FRONTS`, `war::MAX_FRONT_PROVINCES`, `war::MAX_SEA_LANDINGS`, `war::MAX_LANDING_DISTANCE`,
 * `war::LANDING_TROOPS`, `war::MILITARY`,
 * `buildings::FORTIFICATION_CAP`, `factories::COMMAND_LEVEL`, `factories::UNIVERSITY_BONUS`,
 * `factories::SYNTHETIC_FUEL_UNIVERSITIES`.
 */
export const COMMAND_UNLOCKS: readonly { level: number; items: readonly string[] }[] = [
  {
    level: 1,
    items: [
      'Lekkie zaopatrzenie wojskowe',
      '2 fronty naraz, po 1 prowincji',
      'Przeprawy przez rzeki i desant przez jeziora',
      'Desant morski z portu (1 na morzu naraz; do 3 tys. wojska, paliwo na rejs z góry; zasięg według poziomu portu: 100 / 300 kafli / cała mapa)',
    ],
  },
  {
    level: 2,
    items: [
      'Standardowe zaopatrzenie wojskowe',
      '2 fronty, do 2 prowincji na froncie (Ctrl + klik)',
      'Desant do 5 tys. wojska',
      'Umocnienia do poziomu 1',
      'Budowa mostów i tuneli (w przyszłości)',
    ],
  },
  {
    level: 3,
    items: ['3 fronty, do 3 prowincji na froncie', 'Desanty morskie bez limitu, do 15 tys. wojska', 'Umocnienia do poziomu 2', 'Lekkie zaopatrzenie: siła 0,6 → 0,8, 40% mniej paliwa'],
  },
  {
    level: 4,
    items: ['Ciężkie zaopatrzenie wojskowe', 'Fronty bez limitu, do 4 prowincji na froncie', 'Desant do 30 tys. wojska', 'Standardowe zaopatrzenie: siła 1,0 → 1,2, 40% mniej paliwa'],
  },
  { level: 5, items: ['Fronty i prowincje na froncie bez limitu', 'Umocnienia do poziomu 3', 'Desant do 60 tys. wojska'] },
  { level: 6, items: ['Ciężkie zaopatrzenie: siła 1,4 → 1,6, 40% mniej paliwa', 'Desant do 120 tys. wojska', 'Broń jądrowa (w przyszłości)'] },
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
