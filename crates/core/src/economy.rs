//! Gospodarka państwa: populacja (wojsko i robotnicy) i złoto – zasoby globalne gracza.
//!
//! Wszystko w liczbach całkowitych (determinizm natywny vs wasm): populacja w tysięcznych częściach
//! osoby (`MILLI`), więc nawet mały przyrost na tick nie ginie w zaokrągleniach.
//!
//! - **Limit populacji:** `PEOPLE_PER_TILE` na kafel państwa. Start: `START_POPULATION` niezależnie
//!   od wielkości prowincji.
//! - **Przyrost na tick** (`growth`) zależy od zapełnienia `x = populacja / limit`: do `PEAK_PPM` (55%)
//!   rośnie razem z populacją (przyrost przyspiesza), dalej maleje do zera przy limicie (zwalnia) –
//!   proporcjonalnie do pierwiastka z brakującej części, więc populacja dochodzi do limitu w skończonym
//!   czasie (bez długiego „pełzania” po kilka osób); najszybszy jest przy 55% – `PEAK_GROWTH_PPM`
//!   limitu na tick. Ponad limitem populacja maleje.
//! - **Podział:** gracz ustawia suwakiem docelowy udział wojska w **całej** populacji (`target_share`);
//!   nowa populacja od razu w tych proporcjach, a istniejąca przechodzi między wojskiem a robotnikami
//!   z bezwładnością (`SHIFT_PPM` populacji na turę) – także gdy populacja stoi na limicie.
//! - **Złoto:** co `TAX_TICKS` tur (30 s = miesiąc gry) podatek `GOLD_PER_PERSON` od każdego
//!   **robotnika** (wojsko podatków nie płaci). Na start 0.

use serde::{Deserialize, Serialize};

use crate::{
    goods::{GOLD, RESOURCE_COUNT, START_ENGINEERING_MATERIALS, SUPPLY_COUNT, Supply},
    protocol::PlayerId,
};

/// Populacja w tysięcznych częściach osoby.
pub const MILLI: u64 = 1000;
/// Limit populacji na kafel państwa.
pub const PEOPLE_PER_TILE: u64 = 100;
/// Populacja na start (niezależnie od wielkości prowincji startowej).
pub const START_POPULATION: u64 = 25_000;
/// Zapełnienie (części na milion), przy którym przyrost jest największy.
pub const PEAK_PPM: u64 = 550_000;
/// Przyrost na tick przy zapełnieniu `PEAK_PPM` – części na milion limitu (640 → ok. 0,064% limitu
/// na turę; poniżej szczytu populacja podwaja się w ok. 60 s, od 55% do pełna dochodzi w ok. 140 s).
pub const PEAK_GROWTH_PPM: u64 = 640;
/// Domyślny udział wojska w populacji (%).
pub const DEFAULT_TROOP_SHARE: u8 = 50;
/// Bezwładność suwaka: co turę między wojskiem a robotnikami przechodzi najwyżej tyle części na
/// milion populacji (500 → 0,05 punktu procentowego na turę, 0,5 na sekundę – zmiana o 50 punktów
/// trwa ok. 100 s).
pub const SHIFT_PPM: u64 = 500;
/// Podatek co tyle tur (30 s = miesiąc gry – kalendarz na ekranie liczy miesiące tym samym okresem).
pub const TAX_TICKS: u32 = 300;
/// Złota od robotnika za jeden podatek.
pub const GOLD_PER_PERSON: u64 = 1;

/// Stan gospodarki jednego państwa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Economy {
    /// Wojsko i robotnicy (w `MILLI`).
    pub troops: u64,
    pub workers: u64,
    /// Złoto w skarbcu (w interfejsie dzielone przez 10).
    pub gold: u64,
    /// Docelowy udział wojska w całej populacji (%, suwak gracza) – rzeczywisty idzie za nim
    /// z bezwładnością (`SHIFT_PPM`).
    pub target_share: u8,
    /// Przyrost w ostatniej turze (w `MILLI`, ujemny ponad limitem).
    pub last_growth: i64,
    /// Zapas surowców w globalnej puli (w tysięcznych jednostki, indeks = numer surowca; złoto zawsze 0 –
    /// trafia od razu do skarbca).
    pub stock: [u64; RESOURCE_COUNT],
    /// Zapas zaopatrzenia (w tysięcznych, indeks = `Supply as usize`); na start materiały inżynieryjne.
    pub supplies: [u64; SUPPLY_COUNT],
}

impl Default for Economy {
    fn default() -> Self {
        let half = START_POPULATION * MILLI / 2;
        Economy {
            troops: half,
            workers: START_POPULATION * MILLI - half,
            gold: 0,
            target_share: DEFAULT_TROOP_SHARE,
            last_growth: 0,
            stock: [0; RESOURCE_COUNT],
            supplies: {
                let mut s = [0; SUPPLY_COUNT];
                s[Supply::EngineeringMaterials as usize] = START_ENGINEERING_MATERIALS * MILLI;
                s
            },
        }
    }
}

/// Przyrost populacji na tick (w `MILLI`) przy populacji `pop` i limicie `cap` (oba w `MILLI`).
pub fn growth(pop: u64, cap: u64) -> i64 {
    let (pop, cap) = (pop as i128, cap as i128);
    if pop == 0 {
        return 0;
    }
    if cap == 0 {
        // Państwo bez kafli – ludność odpływa w tempie jak ponad limitem.
        return -((pop * PEAK_GROWTH_PPM as i128) / (1_000_000 - PEAK_PPM as i128)).max(1) as i64;
    }
    let peak = cap * PEAK_GROWTH_PPM as i128 / 1_000_000;
    if pop * 1_000_000 <= cap * PEAK_PPM as i128 {
        // x ≤ 55%: przyrost ∝ x (przyspiesza); przy x = 55% – szczyt.
        return (pop * PEAK_GROWTH_PPM as i128 / PEAK_PPM as i128) as i64;
    }
    if pop > cap {
        // Ponad limitem (np. po utracie ziemi): ubytek ∝ nadwyżce.
        return -((pop - cap) * PEAK_GROWTH_PPM as i128 / (1_000_000 - PEAK_PPM as i128)).max(1) as i64;
    }
    // x > 55%: przyrost = szczyt × √(brakująca część / brakująca część przy 55%) – przy 55% szczyt,
    // przy limicie zero, ale do limitu dochodzi w skończonym czasie. Pierwiastek całkowity (`isqrt`)
    // – ten sam wynik natywnie i w wasm. Ostatni krok kończy dokładnie na limicie.
    let room = cap - pop;
    let room_at_peak = cap * (1_000_000 - PEAK_PPM as i128) / 1_000_000;
    let ratio_ppm = (room * 1_000_000 / room_at_peak.max(1)) as u128;
    let sqrt_ppm = (ratio_ppm * 1_000_000).isqrt() as i128;
    let d = (peak * sqrt_ppm / 1_000_000).max(MILLI as i128);
    d.min(room) as i64
}

impl Economy {
    /// Cała populacja (w `MILLI`).
    pub fn population(&self) -> u64 {
        self.troops + self.workers
    }

    /// Podział populacji od razu według celu suwaka – bez bezwładności (przed startem gry).
    pub fn split_now(&mut self) {
        let pop = self.population();
        self.troops = (pop as u128 * self.target_share as u128 / 100) as u64;
        self.workers = pop - self.troops;
    }

    /// Rzeczywisty udział wojska w populacji (%, zaokrąglony).
    pub fn troop_share(&self) -> u8 {
        let pop = self.population();
        if pop == 0 { self.target_share } else { ((self.troops as u128 * 100 + pop as u128 / 2) / pop as u128) as u8 }
    }

    /// Jedna tura gry właściwej przy limicie populacji `cap` (osoby, `population_cap`); `tax` – tura podatku.
    pub fn tick(&mut self, cap: u64, tax: bool) {
        let pop = self.population();
        let d = growth(pop, cap * MILLI);
        self.last_growth = d;
        if d >= 0 {
            // Nowa populacja od razu w docelowych proporcjach.
            let to_troops = d as u64 * self.target_share as u64 / 100;
            self.troops += to_troops;
            self.workers += d as u64 - to_troops;
        } else {
            // Ubytek dzieli się po równo z obecnym składem.
            let loss = (-d as u64).min(pop);
            let from_troops = (loss as u128 * self.troops as u128 / pop.max(1) as u128) as u64;
            self.troops -= from_troops.min(self.troops);
            self.workers -= (loss - from_troops).min(self.workers);
        }
        self.shift();
        if tax {
            self.gold += self.workers / MILLI * GOLD_PER_PERSON;
        }
    }

    /// Przesunięcie ludzi między wojskiem a robotnikami w stronę celu suwaka – najwyżej `SHIFT_PPM`
    /// populacji na turę (także gdy populacja stoi na limicie).
    fn shift(&mut self) {
        let pop = self.population();
        let wanted = (pop as u128 * self.target_share as u128 / 100) as u64;
        let step = (pop as u128 * SHIFT_PPM as u128 / 1_000_000).max(1) as u64;
        if wanted > self.troops {
            let k = (wanted - self.troops).min(step).min(self.workers);
            self.troops += k;
            self.workers -= k;
        } else {
            let k = (self.troops - wanted).min(step);
            self.troops -= k;
            self.workers += k;
        }
    }

    /// Bajty do hasha stanu.
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        let mut bytes = Vec::with_capacity(25 + 8 * (RESOURCE_COUNT + SUPPLY_COUNT));
        bytes.extend(self.troops.to_le_bytes());
        bytes.extend(self.workers.to_le_bytes());
        bytes.extend(self.gold.to_le_bytes());
        bytes.push(self.target_share);
        bytes.extend(self.stock.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.supplies.iter().flat_map(|s| s.to_le_bytes()));
        bytes.into_iter()
    }
}

/// Wielkości państwa liczone z mapy i budynków na początku tury (`Game::rates`) – wspólne dla
/// rozliczenia tury i raportu dla interfejsu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NationRates {
    /// Limit populacji (osoby): kafle × `PEOPLE_PER_TILE` + miasta.
    pub cap: u64,
    /// Robotników potrzebnych do pełnej obsady budynków (osoby).
    pub staff: u64,
    /// Pokrycie obsady (części na milion): min(1, robotnicy / obsada) – mnoży produkcję budynków.
    pub staff_ppm: u64,
    /// Dzienna potrzeba nawozów pól uprawnych (w tysięcznych) i jej pokrycie z puli (części na milion).
    pub fertilizer_need: u64,
    pub fertilizer_ppm: u64,
    /// Wydobycie na dzień (w tysięcznych, indeks = numer surowca; złoto – w surowym złocie).
    pub production: [u64; RESOURCE_COUNT],
}

/// Gospodarka państwa dla interfejsu (osoby i złoto w całych jednostkach; liczby jako `f64` dla TS).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct NationReport {
    /// Gracz kierujący państwem.
    pub player: PlayerId,
    pub population: f64,
    /// Limit populacji (kafle × `PEOPLE_PER_TILE` + miasta).
    pub max_population: f64,
    pub troops: f64,
    pub workers: f64,
    /// Złoto w skarbcu (surowe – interfejs dzieli przez 10).
    pub gold: f64,
    pub troop_share: u8,
    pub target_share: u8,
    /// Przyrost populacji na sekundę (osoby, przy 10 turach/s).
    pub growth_per_second: f64,
    /// Ile złota przyniesie najbliższy podatek (surowe).
    pub next_tax: f64,
    /// Zapas surowców (jednostki, indeks = numer surowca) i wydobycie na dzień.
    pub stock: Vec<f64>,
    pub production: Vec<f64>,
    /// Złoto (surowe) na dzień z wydobycia złota.
    pub gold_mining: f64,
    /// Zapas zaopatrzenia (jednostki, indeks = `Supply as usize`).
    pub supplies: Vec<f64>,
    /// Obsada budynków: potrzebni robotnicy (osoby) i pokrycie (0–1).
    pub staff: f64,
    pub staff_coverage: f64,
    /// Nawozy: potrzeba na dzień (jednostki) i pokrycie (0–1).
    pub fertilizer_need: f64,
    pub fertilizer_coverage: f64,
}

impl NationReport {
    pub fn new(player: PlayerId, e: &Economy, rates: &NationRates) -> Self {
        let production = &rates.production;
        let people = |m: u64| (m / MILLI) as f64;
        NationReport {
            player,
            population: people(e.population()),
            max_population: rates.cap as f64,
            troops: people(e.troops),
            workers: people(e.workers),
            gold: e.gold as f64,
            troop_share: e.troop_share(),
            target_share: e.target_share,
            growth_per_second: e.last_growth as f64 / MILLI as f64 * 10.0,
            next_tax: (e.workers / MILLI * GOLD_PER_PERSON) as f64,
            stock: e.stock.iter().map(|&s| s as f64 / MILLI as f64).collect(),
            production: production
                .iter()
                .enumerate()
                .map(|(r, &p)| if r == GOLD as usize { 0.0 } else { p as f64 / MILLI as f64 })
                .collect(),
            gold_mining: production[GOLD as usize] as f64,
            supplies: e.supplies.iter().map(|&s| s as f64 / MILLI as f64).collect(),
            staff: rates.staff as f64,
            staff_coverage: rates.staff_ppm as f64 / 1e6,
            fertilizer_need: rates.fertilizer_need as f64 / MILLI as f64,
            fertilizer_coverage: rates.fertilizer_ppm as f64 / 1e6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_speeds_up_until_55_percent_then_slows_down() {
        let cap = 60_000 * MILLI;
        let at = |pct: u64| growth(cap * pct / 100, cap);
        assert!(at(10) < at(30) && at(30) < at(50) && at(50) < at(55), "przyspiesza do 55%");
        assert!(at(55) > at(70) && at(70) > at(90) && at(90) > at(99), "potem zwalnia");
        assert_eq!(at(100), 0);
        assert!(at(120) < 0, "ponad limitem maleje");
        // Szczyt: PEAK_GROWTH_PPM limitu na turę.
        assert_eq!(at(55), (cap * PEAK_GROWTH_PPM / 1_000_000) as i64);
    }

    #[test]
    fn population_approaches_the_cap_and_tax_is_paid() {
        let mut e = Economy::default();
        let limit = 60_000;
        let cap = 60_000 * MILLI;
        let mut full_at = None;
        for tick in 1..=3000u32 {
            e.tick(limit, tick % TAX_TICKS == 0);
            if full_at.is_none() && e.population() == cap {
                full_at = Some(tick);
            }
        }
        // Dochodzi do limitu w skończonym czasie (ok. 20 s do 55% + ok. 140 s do pełna) i na nim stoi.
        let full_at = full_at.expect("populacja dochodzi do limitu");
        assert!((1200..=2200).contains(&full_at), "pełna po {full_at} turach");
        assert_eq!(e.population(), cap);
        // Podatek tylko od robotników (połowa populacji przy suwaku 50%).
        assert!(e.gold > 10 * 12_500 && e.gold < 10 * 30_000, "10 podatków od robotników: {}", e.gold);
        // Domyślnie pół na pół.
        assert!(e.troops.abs_diff(e.workers) <= 10 * MILLI, "zaokrąglenia: najwyżej kilka osób różnicy");
    }

    #[test]
    fn share_follows_the_target_with_inertia() {
        // Populacja na limicie (bez przyrostu) – podział i tak idzie za suwakiem.
        let cap = 60_000 * MILLI;
        let mut e = Economy { troops: cap / 2, workers: cap / 2, target_share: 70, ..Default::default() };
        for _ in 0..100 {
            e.tick(60_000, false);
        }
        assert_eq!(e.population(), cap);
        assert_eq!(e.troop_share(), 55, "100 tur po 0,05 punktu");
        for _ in 0..400 {
            e.tick(60_000, false);
        }
        assert_eq!(e.troop_share(), 70, "dochodzi do celu i stoi");
        e.target_share = 0;
        for _ in 0..2000 {
            e.tick(60_000, false);
        }
        assert_eq!((e.troops, e.workers), (0, cap), "w obie strony, aż do zera");
    }
}
