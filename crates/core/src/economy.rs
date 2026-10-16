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
//! - **Prowiant** (jedzenie, `PROVISIONS_PEOPLE_PER_UNIT` osób na sztukę dziennie): pokrycie mnoży przyrost
//!   (do `GROWTH_FOOD_PCT`% przy pełnym), a brak – głód: ubywa do `STARVE_PPM_PER_DAY` populacji dziennie
//!   (część bez jedzenia).
//! - **Zaopatrzenie cywilne:** populacja zużywa podstawowe (`BASIC_PEOPLE_PER_UNIT`) – samo nic nie daje,
//!   ale dopiero przy pełnym działa luksusowe (`LUXURY_PEOPLE_PER_UNIT`): średnie pokrycie z miesiąca daje
//!   do +`LUXURY_TAX_PCT`% podatku.

use serde::{Deserialize, Serialize};

use crate::{
    goods::{GOLD, RESOURCE_COUNT, START_SUPPLIES, SUPPLY_COUNT, Supply, TICKS_PER_DAY},
    protocol::PlayerId,
    war::AttackInfo,
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
/// Zaopatrzenie cywilne: osób na 1 sztukę dziennie – podstawowe i luksusowe.
pub const BASIC_PEOPLE_PER_UNIT: u64 = 25_000;
pub const LUXURY_PEOPLE_PER_UNIT: u64 = 50_000;
/// Prowiant: osób na 1 sztukę dziennie; przyrost × pokrycie × `GROWTH_FOOD_PCT`%; głód – ubytek
/// `STARVE_PPM_PER_DAY` populacji (części na milion) dziennie × część bez jedzenia.
pub const PROVISIONS_PEOPLE_PER_UNIT: u64 = 50_000;
pub const GROWTH_FOOD_PCT: u64 = 150;
pub const STARVE_PPM_PER_DAY: u64 = 5_000;
/// Podatek: + tyle % przy pełnym pokryciu luksusowego przez cały miesiąc (tylko przy pełnym podstawowym).
pub const LUXURY_TAX_PCT: u64 = 50;
/// Części na milion (pokrycia).
const PPM: u64 = 1_000_000;

/// Stan gospodarki jednego państwa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Economy {
    /// Wojsko w kraju (broni) i robotnicy (w `MILLI`).
    pub troops: u64,
    pub workers: u64,
    /// Wojsko w atakach (w `MILLI`) – liczy się do populacji i udziału wojska, ale nie broni
    /// i nie przechodzi do robotników; wraca do `troops`, gdy atak się kończy.
    pub deployed: u64,
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
    /// Ostatnia tura (w tysięcznych): zaopatrzenie wyprodukowane przez fabryki i surowce zużyte
    /// (fabryki, nawozy) – do raportu „na dzień”.
    pub supply_output: [u64; SUPPLY_COUNT],
    pub resource_use: [u64; RESOURCE_COUNT],
    /// Zaopatrzenie zużyte w ostatniej turze (składniki fabryk, ataki, ludność).
    pub supply_use: [u64; SUPPLY_COUNT],
    /// Pokrycie potrzeby ludności w ostatniej turze (części na milion): podstawowe i luksusowe (0, gdy
    /// podstawowego nie starczyło – luksusowe wtedy nie działa).
    pub basic_ppm: u64,
    pub luxury_ppm: u64,
    /// Pokrycie potrzeby prowiantu w ostatniej turze (części na milion).
    pub food_ppm: u64,
    /// Od ostatniego podatku: tury i suma pokrycia luksusowego (średnia – premia do podatku).
    pub civil_ticks: u32,
    pub luxury_sum: u64,
}

impl Default for Economy {
    fn default() -> Self {
        let half = START_POPULATION * MILLI / 2;
        Economy {
            troops: half,
            workers: START_POPULATION * MILLI - half,
            deployed: 0,
            gold: 0,
            target_share: DEFAULT_TROOP_SHARE,
            last_growth: 0,
            stock: [0; RESOURCE_COUNT],
            supplies: START_SUPPLIES.map(|u| u * MILLI),
            supply_output: [0; SUPPLY_COUNT],
            resource_use: [0; RESOURCE_COUNT],
            supply_use: [0; SUPPLY_COUNT],
            basic_ppm: PPM,
            luxury_ppm: 0,
            food_ppm: PPM,
            civil_ticks: 0,
            luxury_sum: 0,
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
        self.troops + self.workers + self.deployed
    }

    /// Całe wojsko: w kraju i w atakach (w `MILLI`).
    pub fn army(&self) -> u64 {
        self.troops + self.deployed
    }

    /// Podział populacji od razu według celu suwaka – bez bezwładności (przed startem gry).
    pub fn split_now(&mut self) {
        let pop = self.troops + self.workers;
        self.troops = (pop as u128 * self.target_share as u128 / 100) as u64;
        self.workers = pop - self.troops;
    }

    /// Rzeczywisty udział wojska w populacji (%, zaokrąglony).
    pub fn troop_share(&self) -> u8 {
        let pop = self.population();
        if pop == 0 { self.target_share } else { ((self.army() as u128 * 100 + pop as u128 / 2) / pop as u128) as u8 }
    }

    /// Jedna tura gry właściwej przy limicie populacji `cap` (osoby, `population_cap`); `tax` – tura podatku.
    pub fn tick(&mut self, cap: u64, tax: bool) {
        self.consume_civil();
        let pop = self.population();
        let mut d = growth(pop, cap * MILLI);
        if d > 0 {
            // Prowiant: przyrost × pokrycie × `GROWTH_FOOD_PCT`% (ubytek ponad limitem – bez zmian).
            let scaled = d as i128 * (GROWTH_FOOD_PCT * self.food_ppm) as i128 / (100 * PPM) as i128;
            d = if self.food_ppm > 0 { (scaled as i64).max(1) } else { 0 };
        }
        // Głód: część populacji bez jedzenia ubywa.
        let starve = pop as u128 * STARVE_PPM_PER_DAY as u128 * (PPM - self.food_ppm) as u128
            / (PPM as u128 * PPM as u128 * TICKS_PER_DAY as u128);
        d -= starve as i64;
        self.last_growth = d;
        if d >= 0 {
            // Nowa populacja od razu w docelowych proporcjach.
            let to_troops = d as u64 * self.target_share as u64 / 100;
            self.troops += to_troops;
            self.workers += d as u64 - to_troops;
        } else {
            // Ubytek dzieli się po równo z obecnym składem (wojsko w atakach nie ubywa).
            let home = self.troops + self.workers;
            let loss = (-d as u64).min(home);
            let from_troops = (loss as u128 * self.troops as u128 / home.max(1) as u128) as u64;
            self.troops -= from_troops.min(self.troops);
            self.workers -= (loss - from_troops).min(self.workers);
        }
        self.shift();
        self.civil_ticks += 1;
        self.luxury_sum += self.luxury_ppm;
        if tax {
            self.gold += self.next_tax();
            (self.civil_ticks, self.luxury_sum) = (0, 0);
        }
    }

    /// Podatek, gdyby był teraz: robotnicy × `GOLD_PER_PERSON` + premia z luksusowego (średnia z miesiąca).
    pub fn next_tax(&self) -> u64 {
        let base = self.workers / MILLI * GOLD_PER_PERSON;
        base + base * LUXURY_TAX_PCT * self.tax_bonus_ppm() / (100 * PPM)
    }

    /// Średnie pokrycie luksusowego od ostatniego podatku (części na milion).
    pub fn tax_bonus_ppm(&self) -> u64 {
        self.luxury_sum.checked_div(self.civil_ticks as u64).unwrap_or(0)
    }

    /// Zużycie przez ludność na tę turę: prowiant, podstawowe cywilne, a przy pełnym – luksusowe.
    fn consume_civil(&mut self) {
        let pop = self.population();
        let take = |e: &mut Self, s: Supply, people: u64| {
            let need = pop / (people * TICKS_PER_DAY);
            if need == 0 {
                return PPM;
            }
            let got = e.supplies[s as usize].min(need);
            e.supplies[s as usize] -= got;
            e.supply_use[s as usize] += got;
            got * PPM / need
        };
        self.food_ppm = take(self, Supply::Provisions, PROVISIONS_PEOPLE_PER_UNIT);
        self.basic_ppm = take(self, Supply::BasicCivil, BASIC_PEOPLE_PER_UNIT);
        self.luxury_ppm =
            if self.basic_ppm >= PPM { take(self, Supply::LuxuryCivil, LUXURY_PEOPLE_PER_UNIT) } else { 0 };
    }

    /// Przesunięcie ludzi między wojskiem a robotnikami w stronę celu suwaka – najwyżej `SHIFT_PPM`
    /// populacji na turę (także gdy populacja stoi na limicie).
    fn shift(&mut self) {
        let pop = self.population();
        let wanted = (pop as u128 * self.target_share as u128 / 100) as u64;
        let step = (pop as u128 * SHIFT_PPM as u128 / 1_000_000).max(1) as u64;
        let army = self.army();
        if wanted > army {
            let k = (wanted - army).min(step).min(self.workers);
            self.troops += k;
            self.workers -= k;
        } else {
            // Do robotników wracają tylko żołnierze z kraju.
            let k = (army - wanted).min(step).min(self.troops);
            self.troops -= k;
            self.workers += k;
        }
    }

    /// Bajty do hasha stanu.
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        let mut bytes = Vec::with_capacity(25 + 8 * (RESOURCE_COUNT + SUPPLY_COUNT));
        bytes.extend(self.troops.to_le_bytes());
        bytes.extend(self.workers.to_le_bytes());
        bytes.extend(self.deployed.to_le_bytes());
        bytes.extend(self.gold.to_le_bytes());
        bytes.push(self.target_share);
        bytes.extend(self.stock.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.supplies.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.supply_output.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.resource_use.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.supply_use.iter().flat_map(|s| s.to_le_bytes()));
        bytes.extend(self.basic_ppm.to_le_bytes());
        bytes.extend(self.luxury_ppm.to_le_bytes());
        bytes.extend(self.food_ppm.to_le_bytes());
        bytes.extend(self.civil_ticks.to_le_bytes());
        bytes.extend(self.luxury_sum.to_le_bytes());
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
    /// Wydobycie na dzień (w tysięcznych, indeks = numer surowca; złoto – w surowym złocie; surowce
    /// spożywcze – 0, idą od razu w prowiant).
    pub production: [u64; RESOURCE_COUNT],
    /// Prowiant z wydobycia surowców spożywczych na dzień (w tysięcznych).
    pub provisions: u64,
    /// Suma poziomów uniwersytetów w państwie (progi technologii, `factories::university_bonus`).
    pub universities: u32,
    /// Poziom centrum dowodzenia (`Game::command_level`).
    pub command_level: u8,
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
    /// Na dzień (jednostki, z ostatniej tury): zaopatrzenie z fabryk (indeks = `Supply as usize`)
    /// i zużycie surowców przez fabryki i nawożenie (indeks = numer surowca).
    pub supply_production: Vec<f64>,
    pub resource_use: Vec<f64>,
    /// Zużycie zaopatrzenia na dzień (składniki fabryk, ataki).
    pub supply_use: Vec<f64>,
    /// Poziom centrum dowodzenia.
    pub command_level: u8,
    /// Suma poziomów uniwersytetów w państwie.
    pub universities: u32,
    /// Pokrycie potrzeby ludności (0–1): podstawowe (przyrost) i luksusowe (premia do podatku) z ostatniej
    /// tury, średnia luksusowego od ostatniego podatku i potrzeba na dzień (sztuki) obu rodzajów.
    pub basic_coverage: f64,
    pub luxury_coverage: f64,
    /// Prowiant: pokrycie (0–1) z ostatniej tury i potrzeba na dzień (sztuki).
    pub food_coverage: f64,
    pub food_need: f64,
    pub tax_bonus: f64,
    pub basic_need: f64,
    pub luxury_need: f64,
    /// Wojsko w atakach (osoby; `troops` – tylko w kraju) i trwające ataki.
    pub deployed: f64,
    pub attacks: Vec<AttackInfo>,
}

impl NationReport {
    pub fn new(player: PlayerId, e: &Economy, rates: &NationRates, attacks: Vec<AttackInfo>) -> Self {
        let production = &rates.production;
        let people = |m: u64| (m / MILLI) as f64;
        // Z ostatniej tury (w tysięcznych) na dzień, w jednostkach.
        let per_day = |m: u64| (m * TICKS_PER_DAY) as f64 / MILLI as f64;
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
            next_tax: e.next_tax() as f64,
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
            supply_production: e.supply_output.iter().map(|&s| per_day(s)).collect(),
            resource_use: e.resource_use.iter().map(|&s| per_day(s)).collect(),
            supply_use: e.supply_use.iter().map(|&s| per_day(s)).collect(),
            command_level: rates.command_level,
            universities: rates.universities,
            basic_coverage: e.basic_ppm as f64 / 1e6,
            food_coverage: e.food_ppm as f64 / 1e6,
            food_need: people(e.population()) / PROVISIONS_PEOPLE_PER_UNIT as f64,
            luxury_coverage: e.luxury_ppm as f64 / 1e6,
            tax_bonus: e.tax_bonus_ppm() as f64 / 1e6 * LUXURY_TAX_PCT as f64 / 100.0,
            basic_need: people(e.population()) / BASIC_PEOPLE_PER_UNIT as f64,
            luxury_need: people(e.population()) / LUXURY_PEOPLE_PER_UNIT as f64,
            deployed: people(e.deployed),
            attacks,
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
        // Pod dostatkiem prowiantu (przyrost 150%), bez luksusowego (podatek bez premii).
        let mut supplies = [0; SUPPLY_COUNT];
        supplies[Supply::Provisions as usize] = 100_000 * MILLI;
        let mut e = Economy { supplies, ..Default::default() };
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
        assert!((700..=1600).contains(&full_at), "pełna po {full_at} turach");
        assert_eq!(e.population(), cap);
        // Podatek tylko od robotników (połowa populacji przy suwaku 50%).
        assert!(e.gold > 10 * 12_500 && e.gold < 10 * 30_000, "10 podatków od robotników: {}", e.gold);
        // Domyślnie pół na pół.
        assert!(e.troops.abs_diff(e.workers) <= 10 * MILLI, "zaokrąglenia: najwyżej kilka osób różnicy");
    }

    #[test]
    fn provisions_feed_growth_and_luxury_raises_taxes() {
        let fed = |food: u64, basic: u64, luxury: u64| {
            let mut s = [0; SUPPLY_COUNT];
            s[Supply::Provisions as usize] = food * MILLI;
            s[Supply::BasicCivil as usize] = basic * MILLI;
            s[Supply::LuxuryCivil as usize] = luxury * MILLI;
            Economy { supplies: s, ..Default::default() }
        };
        let start = fed(0, 0, 0).population();
        let (mut hungry, mut fed_up, mut basic_only) = (fed(0, 0, 0), fed(1000, 0, 0), fed(1000, 1000, 0));
        for _ in 0..50 {
            hungry.tick(60_000, false);
            fed_up.tick(60_000, false);
            basic_only.tick(60_000, false);
        }
        assert!(hungry.population() < start, "bez prowiantu populacji ubywa");
        assert!(fed_up.population() > start, "z prowiantem rośnie");
        assert_eq!(basic_only.population(), fed_up.population(), "podstawowe cywilne nie zmienia przyrostu");
        assert!(fed_up.supplies[Supply::Provisions as usize] < 1000 * MILLI, "ludność je prowiant");
        assert!(basic_only.supplies[Supply::BasicCivil as usize] < 1000 * MILLI, "i zużywa podstawowe");
        let fed = |basic: u64, luxury: u64| fed(1000, basic, luxury);
        // Podatek: luksusowe daje premię tylko przy pełnym podstawowym.
        let tax = |mut e: Economy| {
            for t in 1..=TAX_TICKS {
                e.tick(60_000, t == TAX_TICKS);
            }
            (e.gold, e.workers / MILLI * GOLD_PER_PERSON)
        };
        let (gold, base) = tax(fed(1000, 1000));
        assert!(gold > base * 14 / 10, "luksusowe: +50% podatku ({gold} przy {base})");
        let (gold, base) = tax(fed(0, 1000));
        assert!(gold <= base + 1, "bez podstawowego luksusowe nie działa");
        let (gold, base) = tax(fed(1000, 0));
        assert!(gold <= base + 1, "samo podstawowe nie podnosi podatku");
    }

    #[test]
    fn share_follows_the_target_with_inertia() {
        // Populacja na limicie (bez przyrostu) – podział i tak idzie za suwakiem.
        let cap = 60_000 * MILLI;
        let mut supplies = [0; SUPPLY_COUNT];
        supplies[Supply::Provisions as usize] = 100_000 * MILLI;
        let mut e = Economy { troops: cap / 2, workers: cap / 2, target_share: 70, supplies, ..Default::default() };
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
