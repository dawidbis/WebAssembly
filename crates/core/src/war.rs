//! Ekspansja i walka (docs/gdd/zaopatrzenie.md „Wojsko w walce”, docs/gdd/budynki.md „Centrum
//! dowodzenia”): stałe do strojenia, stan frontu i typy raportów. Zasady (front, koszty, zaopatrzenie)
//! są w `game.rs`.
//!
//! Front to wojsko wysłane na **jedną albo kilka prowincji** naraz (ile – zależy od centrum dowodzenia):
//! co turę zajmuje kafle tych prowincji stykające się (bokiem) z naszymi – niczyje za koszt walki
//! z armią tubylczą, cudze według gęstości wojska obrońcy i stosunku zaopatrzenia obu stron. Front
//! sięga też przez rzekę i góry (przeprawa – przygotowanie i limit wojska), przez jezioro i – jako desant
//! morski – przez morze. Zużyte wojsko ginie, resztka wraca do puli, gdy front się kończy.

use serde::{Deserialize, Serialize};

use crate::{economy::MILLI, goods::Supply, owners::Nation};

/// Armia tubylcza każdej prowincji: `NATIVES_PER_TILE` osób na kafel × mnożnik infrastruktury startowej
/// (%; indeks = poziom 1–3) – słaba, ma spowalniać, nie blokować.
pub const NATIVES_PER_TILE: u64 = 8;
pub const NATIVES_INFRA_PCT: [u64; 4] = [100, 80, 100, 130];
/// Kafel niczyj na nizinie bez lasu: `NATIVE_TILE_COST` + gęstość tubylców na kafel × `NATIVE_DEFENSE_PCT`%
/// (osoby wojska); tubylcy tracą gęstość za każdy kafel. Teren mnoży koszt, punkty szybkości i ustala
/// kolejność (`Game::terrain_pct`: las 125%, wyżyny 150%, wyżyny z lasem 190%).
pub const NATIVE_TILE_COST: u64 = 2;
pub const NATIVE_DEFENSE_PCT: u64 = 10;
/// Potyczki z tubylcami (co turę, póki front styka się z ziemią niczyją): tubylcy w styczności
/// (kafle frontu niczyje × gęstość) zabijają dziennie `NATIVE_HIT_PCT`% swojej liczby wojska atakującego
/// (× losowo `SKIRMISH_MIN_PCT`–`SKIRMISH_MAX_PCT`%), sami tracą `NATIVE_LOSS_PCT`% tej liczby.
pub const NATIVE_HIT_PCT: u64 = 30;
pub const NATIVE_LOSS_PCT: u64 = 60;
pub const SKIRMISH_MIN_PCT: u64 = 50;
pub const SKIRMISH_MAX_PCT: u64 = 150;
/// Chaos frontu: szybkość co turę × losowo `SPEED_MIN_PCT`–`SPEED_MAX_PCT`%; „kępy” – wspólny los
/// kwadratów `CLUMP_TILES` × `CLUMP_TILES` kafli zmieniany co `CLUMP_TICKS` tur (do `CLUMP_NOISE` punktów
/// kolejności); „zrywy” – co `THRUST_TICKS` tur front wybiera 1 + długość / `THRUST_PER_TILES` punktów
/// natarcia, a kafle w promieniu `THRUST_RADIUS` idą wcześniej (do `THRUST_BONUS` punktów).
pub const SPEED_MIN_PCT: u64 = 60;
pub const SPEED_MAX_PCT: u64 = 140;
pub const CLUMP_TILES: usize = 6;
pub const CLUMP_TICKS: u32 = 12;
pub const CLUMP_NOISE: u64 = 90;
pub const THRUST_TICKS: u32 = 25;
pub const THRUST_PER_TILES: usize = 40;
pub const THRUST_RADIUS: i64 = 9;
pub const THRUST_BONUS: i64 = 160;
/// Najmniej wojska, jakie można wysłać na front (osoby).
pub const MIN_ATTACK_TROOPS: u64 = 20;
/// Kolejność frontu (`front_score`, mniej = wcześniej): trudność terenu (%) + „zawirowanie taktyczne”
/// (losowo 0..`TURBULENCE`) − `SURROUND_BONUS` za każdy nasz kafel wokół (8 sąsiadów) – kafle
/// otoczone znikają od razu, a front przesuwa się spójnie.
pub const TURBULENCE: u64 = 60;
pub const SURROUND_BONUS: i64 = 45;
/// Kafel innego gracza: `ENEMY_TILE_COST` + gęstość wojska obrońcy na kafel × `DEFENSE_PCT`% ×
/// (zaopatrzenie obrońcy / zaopatrzenie atakującego). Obrońca traci gęstość wojska za każdy kafel.
pub const ENEMY_TILE_COST: u64 = 20;
pub const DEFENSE_PCT: u64 = 200;
/// Desant (kafel zajmowany z wody – jezioro, morze): koszt i punkty szybkości × `LANDING_PCT`%.
pub const LANDING_PCT: u64 = 150;
/// Teren kafla (%): niziny, las, wyżyny, wyżyny z lasem – mnoży koszt zajęcia (straty zdobywającego),
/// punkty szybkości i kolejność frontu.
pub const TERRAIN_PCT: [u64; 4] = [100, 140, 180, 250];
/// Zdobywający kafel gracza traci tyle % kosztu kafla (obrońca traci gęstość swojej armii na kafel).
pub const CAPTURE_LOSS_PCT: u64 = 150;
/// Armia obrony: gdy wrogi front (w wojnie) walczy o prowincję, obrońca wystawia w niej armię z puli –
/// `DEFENSE_SHARE_PCT`% limitu populacji, który daje mu ta prowincja (jego kafle × 100 + miasto). Bez
/// uzupełniania: dosłanie wojska ręcznie (`Intent::Attack` na tę prowincję). Zużywa zaopatrzenie i paliwo
/// jak armia `DEFENSE_SUPPLY_PCT`% swojej wielkości. Odbija tylko swoje kafle w tej prowincji.
pub const DEFENSE_SHARE_PCT: u64 = 100;
pub const DEFENSE_SUPPLY_PCT: u64 = 50;
/// Szybkość frontu: punkty na turę przy równych siłach (kafel niczyj – `NEUTRAL_TILE_POINTS`, cudzy –
/// `ENEMY_TILE_POINTS`) × tempo. **Tempo** = √(siła ataku na kafel frontu / obrona na kafel), przycięte do
/// `PACE_MIN_PCT`–`PACE_MAX_PCT`%: siła ataku – wojsko frontu × siła zaopatrzenia / kafle frontu; obrona –
/// tubylcy (gęstość × `NO_SUPPLY_PCT`) albo gracz (gęstość wojska × siła zaopatrzenia), najmniej
/// `MIN_DEFENSE` osób na kafel. Duża armia na wąskim froncie idzie dużo szybciej niż rozciągnięta.
pub const POINTS_PER_TICK: u64 = 12;
pub const MIN_DEFENSE: u64 = 20;
pub const PACE_MIN_PCT: u64 = 30;
pub const PACE_MAX_PCT: u64 = 400;
pub const NEUTRAL_TILE_POINTS: u64 = 10;
pub const ENEMY_TILE_POINTS: u64 = 20;
/// Paliwo przyspiesza front: × (1 + `FUEL_SPEED_PCT`% × pokrycie paliwem).
pub const FUEL_SPEED_PCT: u64 = 50;
/// Zaopatrzenie wojskowe w walce na 1 000 wojska na dzień (w tysięcznych sztuki) – zużywane tylko, gdy
/// front styka się z innym graczem.
pub const SUPPLY_PER_1000_PER_DAY: u64 = 50;
/// Paliwo na 1 000 wojska na dzień (w tysięcznych sztuki) bez zaopatrzenia wojskowego (także na ziemi
/// niczyjej); z zaopatrzeniem – `Military::fuel`.
pub const FUEL_NO_SUPPLY: u64 = 50;
/// Od progu centrum dowodzenia rodzaj zaopatrzenia zużywa tyle % paliwa mniej.
pub const FUEL_SAVING_PCT: u64 = 40;
/// Konie jako zamiennik paliwa: 1 koń = `HORSE_FUEL_PCT`% sztuki paliwa, najwyżej `HORSE_MAX_PCT`% potrzeby.
pub const HORSE_FUEL_PCT: u64 = 50;
pub const HORSE_MAX_PCT: u64 = 50;
/// Współczynnik siły (%) bez zaopatrzenia wojskowego.
pub const NO_SUPPLY_PCT: u64 = 25;
/// Kocioł: kafle państwa odcięte od stolicy, morza i ziemi niczyjej (dookoła sami wrogowie, góry, rzeki,
/// jeziora) bronią się tyle % zwykłej obrony. Kocioł może obejmować kilka prowincji; liczony co
/// `POCKET_TICKS` tur.
pub const POCKET_DEFENSE_PCT: u64 = 40;
/// Umocnienia prowincji (poziom 0–3) mnożą obronę kafli państwa, które je kontroluje (> 50% kafli prowincji),
/// w koszcie kafla i tempie frontu (%): +50% za poziom. Zdobyte (przewaga w prowincji) bronią zdobywcy.
pub const FORT_DEFENSE_PCT: [u64; 4] = [100, 150, 200, 250];
pub const POCKET_TICKS: u32 = 10;
/// Wspólny front (starcie): dwa fronty różnych państw w tej samej albo sąsiednich prowincjach, z kaflami
/// przeciwnika w swoich prowincjach. Obie strony dalej zajmują kafle (żywy, chaotyczny front), ale:
/// - kafla bronią razem obie armie obrońcy – w kraju (wojsko na kafel państwa) i na jego froncie (wojsko frontu
///   na kafel jego państwa w prowincjach obu frontów) – i obie tracą ludzi przy zajęciu kafla (według siły),
/// - armie frontów co turę ścierają się ze sobą: dziennie ginie `CLASH_PCT`% mniejszej z nich (× los
///   `SKIRMISH_*`), dzielone odwrotnie do siły (wojsko × siła zaopatrzenia) – słabszy traci więcej.
pub const CLASH_PCT: u64 = 2;
/// Front kończy się sam (resztka wraca do kraju), gdy jego armia spadnie poniżej tylu % wojska wysłanego na
/// niego (z posiłkami) albo gdy stoi `STALL_TICKS` tur bez zdobytego kafla i bez posiłków – na wspólnym
/// froncie, gdy tak stoją obie strony.
pub const AUTO_END_PCT: u64 = 2;
pub const STALL_TICKS: u32 = 300;
/// Aneksja po zdobyciu kafla stolicy (centrum dowodzenia): zdobywca dostaje całe terytorium pokonanego
/// i tyle % jego złota, populacji (wojsko i robotnicy w kraju), każdego surowca i zaopatrzenia z magazynu.
/// Wojsko pokonanego na frontach przepada, centrum dowodzenia jest zniszczone.
pub const ANNEX_GOLD_PCT: u64 = 50;
pub const ANNEX_POPULATION_PCT: u64 = 25;
pub const ANNEX_STOCK_PCT: u64 = 30;

/// Rodzaj zaopatrzenia wojskowego w walce: siła (%), od poziomu centrum dowodzenia `upgrade` – siła
/// `upgraded_pct` i paliwo × (100 − `FUEL_SAVING_PCT`)%; paliwo na 1 000 wojska na dzień (tysięczne).
#[derive(Clone, Copy, Debug)]
pub struct Military {
    pub supply: Supply,
    pub pct: u64,
    pub upgraded_pct: u64,
    pub upgrade: u8,
    pub fuel: u64,
}

/// Zaopatrzenie wojskowe od najlepszego (w tej kolejności zużywane).
pub const MILITARY: [Military; 3] = [
    Military { supply: Supply::HeavyMilitary, pct: 140, upgraded_pct: 160, upgrade: 6, fuel: 400 },
    Military { supply: Supply::StandardMilitary, pct: 100, upgraded_pct: 120, upgrade: 4, fuel: 200 },
    Military { supply: Supply::LightMilitary, pct: 60, upgraded_pct: 80, upgrade: 3, fuel: 100 },
];

impl Military {
    /// Siła (%) przy poziomie centrum dowodzenia `command`.
    pub fn strength(&self, command: u8) -> u64 {
        if command >= self.upgrade { self.upgraded_pct } else { self.pct }
    }

    /// Paliwo na 1 000 wojska na dzień (tysięczne) przy poziomie centrum dowodzenia `command`.
    pub fn fuel(&self, command: u8) -> u64 {
        if command >= self.upgrade { self.fuel * (100 - FUEL_SAVING_PCT) / 100 } else { self.fuel }
    }
}

/// Limity frontów według poziomu centrum dowodzenia (indeks = poziom; `usize::MAX` – bez limitu):
/// frontów naraz, prowincji na jednym froncie, desantów morskich naraz (desant to też front).
pub const MAX_FRONTS: [usize; 7] = [2, 2, 2, 3, usize::MAX, usize::MAX, usize::MAX];
pub const MAX_FRONT_PROVINCES: [usize; 7] = [1, 1, 2, 3, 4, usize::MAX, usize::MAX];
pub const MAX_SEA_LANDINGS: [usize; 7] = [1, 1, 1, usize::MAX, usize::MAX, usize::MAX, usize::MAX];
/// Desant morski: wypływa z naszego portu (budynek) nad tym samym morzem, z którego rejs jest najkrótszy
/// (najkrótsza droga po kaflach morza, bez limitu odległości) i zawsze tworzy odcięty przyczółek; najwięcej
/// wojska na jednym desancie według poziomu portu wypłynięcia (indeks = poziom, osoby), rejs `EMBARK_TICKS` +
/// odległość / `SAIL_TILES_PER_TICK` tur, paliwo z góry: `LANDING_FUEL` tysięcznych sztuki na 1 000 wojska na
/// kafel odległości.
pub const LANDING_TROOPS: [u64; 4] = [0, 5_000, 15_000, 40_000];
/// Przyczółek (desant albo atak z niego) sprawdza połączenie z krajem co tyle tur: lądem do stolicy albo
/// zdobyty port w przyczółku (budynek musi stać).
pub const EXPEDITION_TICKS: u32 = 10;
/// `Attack::anchor`: przyczółek jeszcze bez kafla.
pub const NO_ANCHOR: u32 = u32::MAX;
pub const EMBARK_TICKS: u32 = 20;
pub const SAIL_TILES_PER_TICK: u32 = 1;
pub const LANDING_FUEL: u64 = 20;

/// Desant morski frontu: morze (zbiornik), port wypłynięcia (prowincja), odległość rejsu (kafle drogi po
/// morzu), kafle: miejsce portu, brzeg celu i ostatni kafel morza przed nim (koniec drogi).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landing {
    pub body: u32,
    pub port: u16,
    pub distance: u32,
    pub from: u32,
    pub to: u32,
    pub sea: u32,
}

impl Landing {
    /// Tury rejsu.
    pub fn sailing(&self) -> u32 {
        EMBARK_TICKS + self.distance / SAIL_TILES_PER_TICK
    }

    /// Paliwo (tysięczne sztuki) na rejs `troops` wojska (tysięczne osoby).
    pub fn fuel(&self, troops: u64) -> u64 {
        (troops as u128 * self.distance as u128 * LANDING_FUEL as u128 / (MILLI as u128 * 1000)) as u64
    }

    /// Najwięcej wojska (tysięczne osoby), jakie przewiezie paliwo `fuel` (tysięczne sztuki).
    pub fn troops_for(&self, fuel: u64) -> u64 {
        (fuel as u128 * MILLI as u128 * 1000 / (self.distance.max(1) as u128 * LANDING_FUEL as u128)) as u64
    }
}
/// Przeprawa przez rzekę (od centrum dowodzenia 1): front sięga przez najwyżej tyle kafli rzeki w linii
/// prostej. Desant przez jezioro – też od 1.
pub const RIVER_CROSSING_TILES: usize = 4;
/// Przeprawa górska: droga przez kafle gór, lodowca i rzek (bez prowincji, bez jezior i morza) – najwyżej
/// tyle kafli (jak `tunnel_max` generatora, więc sięga każdej doliny za górami).
pub const MOUNTAIN_CROSSING_TILES: u32 = 60;
/// Przeprawy (front zaczyna się tylko przez rzekę albo góry): wojsko na froncie najwyżej
/// `*_CROSSING_TROOPS` według poziomu centrum dowodzenia (indeks = poziom, osoby; więcej niż desant),
/// przygotowanie `*_PREP_TICKS` tur (krócej niż zaokrętowanie desantu, `EMBARK_TICKS`) i przejście
/// `*_TICKS_PER_TILE` tur na kafel drogi. Góry: dłużej i mniej wojska. Limit dotyczy całego frontu (także
/// posiłków) – przeprawa to wąskie gardło.
pub const RIVER_CROSSING_TROOPS: [u64; 7] = [0, 20_000, 30_000, 50_000, 80_000, 120_000, 200_000];
pub const MOUNTAIN_CROSSING_TROOPS: [u64; 7] = [0, 8_000, 12_000, 20_000, 30_000, 45_000, 70_000];
pub const RIVER_PREP_TICKS: u32 = 8;
pub const MOUNTAIN_PREP_TICKS: u32 = 15;
pub const RIVER_TICKS_PER_TILE: u32 = 1;
pub const MOUNTAIN_TICKS_PER_TILE: u32 = 1;

/// Rodzaj przeprawy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum CrossingKind {
    River,
    Mountain,
}

/// Przeprawa frontu: rodzaj, nasz kafel wyjścia, kafel celu (tu front zaczyna) i długość drogi (kafle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crossing {
    pub kind: CrossingKind,
    pub from: u32,
    pub to: u32,
    pub length: u32,
}

impl Crossing {
    /// Tury przygotowania (pierścień ładowania na mapie).
    pub fn prep(&self) -> u32 {
        match self.kind {
            CrossingKind::River => RIVER_PREP_TICKS,
            CrossingKind::Mountain => MOUNTAIN_PREP_TICKS,
        }
    }

    /// Tury przygotowania i przejścia.
    pub fn ticks(&self) -> u32 {
        let per_tile = match self.kind {
            CrossingKind::River => RIVER_TICKS_PER_TILE,
            CrossingKind::Mountain => MOUNTAIN_TICKS_PER_TILE,
        };
        self.prep() + self.length * per_tile
    }

    /// Najwięcej wojska na froncie przeprawy (tysięczne osoby) przy poziomie centrum dowodzenia `command`.
    pub fn max_troops(&self, command: u8) -> u64 {
        let table = match self.kind {
            CrossingKind::River => &RIVER_CROSSING_TROOPS,
            CrossingKind::Mountain => &MOUNTAIN_CROSSING_TROOPS,
        };
        table[command.min(6) as usize] * MILLI
    }
}

/// Przeprawa dla interfejsu: rodzaj, czas (przygotowanie i przejście, dni), długość i limit wojska na froncie.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CrossingReport {
    pub kind: CrossingKind,
    pub days: f64,
    pub tiles: u32,
    /// Limit wojska na froncie (osoby) i ile już na nim jest (trwający front).
    pub max_troops: f64,
    pub troops: f64,
}
/// Ziarno kolejności zajmowania kafli frontu (XOR z kaflem i tickiem).
pub const FRONT_SALT: u64 = 0x4652_4f4e_5400_0001;

/// `Attack::supply`: front nie styka się z graczem (zaopatrzenie wojskowe nie jest zużywane).
pub const NOT_FIGHTING: u8 = u8::MAX;
/// `Attack::supply`: walka bez zaopatrzenia wojskowego.
pub const NO_SUPPLY: u8 = u8::MAX - 1;

/// Trwający front państwa na jedną albo kilka prowincji.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attack {
    pub nation: Nation,
    /// Prowincje frontu (zajęte w całości odpadają; front kończy się, gdy nie zostanie żadna).
    pub provinces: Vec<u16>,
    /// Wojsko na froncie (w tysięcznych osoby).
    pub troops: u64,
    /// Kafle zajęte na tym froncie.
    pub taken: u32,
    /// Zaopatrzenie wojskowe zużyte w ostatniej turze (najwięcej sztuk; `Supply as u8`, `NO_SUPPLY`,
    /// `NOT_FIGHTING`).
    pub supply: u8,
    /// Desant morski (front przez morze z portu – wlicza się do `MAX_SEA_LANDINGS`).
    pub landing: Option<Landing>,
    /// Przeprawa przez rzekę albo góry (front zaczęty tylko przez nią – limit wojska na całym froncie).
    pub crossing: Option<Crossing>,
    /// Tury rejsu (desant) albo przygotowania i przejścia (przeprawa), które zostały – front nie zajmuje
    /// kafli, póki trwają.
    pub sailing: u32,
    /// Straty frontu (w tysięcznych osoby): koszty kafli i potyczki z tubylcami.
    pub lost: u64,
    /// Punkty natarcia (kafle) – zmieniane co `THRUST_TICKS` tur.
    pub thrusts: Vec<u32>,
    /// Tempo w ostatniej turze (%, `PACE_MIN_PCT`–`PACE_MAX_PCT`).
    pub pace: u16,
    /// Długość frontu (kafle) i siła zaopatrzenia (%) w ostatniej turze – obrona i starcie wspólnego frontu.
    pub front: u32,
    pub strength: u16,
    /// Pokrycie potrzeby paliwa w ostatniej turze (%) i czy brakujące paliwo zastąpiły konie.
    pub fuel_pct: u8,
    pub horses: bool,
    /// Wojsko wysłane na front razem z posiłkami (w tysięcznych osoby) i tury bez zdobyczy i posiłków.
    pub sent: u64,
    pub idle: u32,
    /// Przyczółek odcięty od kraju (desant albo atak z przyczółka): bez posiłków z kraju – tylko kolejnym
    /// desantem; po końcu frontu wojsko zostaje w przyczółku (`Garrison`). Kafel przyczółka (`NO_ANCHOR` –
    /// jeszcze żaden).
    pub expedition: bool,
    pub anchor: u32,
    /// Posiłki desantem do trwającego frontu-przyczółka: po rejsie dołączają do niego.
    pub reinforce: bool,
    /// Armia obrony prowincji (jedyna w `provinces`) – `DEFENSE_SHARE_PCT`; trwa, dopóki ktoś o nią walczy.
    pub defense: bool,
    /// Droga rejsu (kafle morza od portu do brzegu, co kilka kafli) – z mapy, poza hashem.
    pub path: Vec<u32>,
}

/// Wojsko w odciętym przyczółku (po froncie): państwo, kafel przyczółka i wojsko (w tysięcznych osoby).
/// Kolejne ataki z przyczółka biorą wojsko stąd; po połączeniu z krajem wraca do puli.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Garrison {
    pub nation: Nation,
    pub anchor: u32,
    pub troops: u64,
    /// Kafle odciętego kawałka terytorium (rosnąco) z ostatniego sprawdzenia – wynikają ze stanu, poza hashem.
    pub tiles: Vec<u32>,
}

impl Garrison {
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        [self.nation].into_iter().chain(self.anchor.to_le_bytes()).chain(self.troops.to_le_bytes())
    }
}

/// Garnizon przyczółka dla interfejsu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct GarrisonReport {
    pub province: u16,
    pub troops: f64,
    pub x: u32,
    pub y: u32,
}

impl Attack {
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        let mut b = vec![self.nation, self.provinces.len() as u8];
        b.extend(self.provinces.iter().flat_map(|p| p.to_le_bytes()));
        b.extend(self.troops.to_le_bytes());
        b.extend(self.taken.to_le_bytes());
        b.push(self.supply);
        let l = self.landing.unwrap_or(Landing { body: 0, port: 0, distance: 0, from: 0, to: 0, sea: 0 });
        b.extend(l.body.to_le_bytes());
        b.extend(l.port.to_le_bytes());
        b.extend(l.distance.to_le_bytes());
        b.extend(l.from.to_le_bytes());
        b.extend(l.to.to_le_bytes());
        b.extend(l.sea.to_le_bytes());
        match self.crossing {
            Some(c) => {
                b.push(1 + c.kind as u8);
                b.extend(c.from.to_le_bytes());
                b.extend(c.to.to_le_bytes());
                b.extend(c.length.to_le_bytes());
            }
            None => b.push(0),
        }
        b.extend(self.sailing.to_le_bytes());
        b.extend(self.lost.to_le_bytes());
        b.extend(self.pace.to_le_bytes());
        b.extend(self.front.to_le_bytes());
        b.extend(self.strength.to_le_bytes());
        b.push(self.fuel_pct);
        b.push(self.horses as u8);
        b.extend(self.sent.to_le_bytes());
        b.extend(self.idle.to_le_bytes());
        b.extend(self.thrusts.iter().flat_map(|t| t.to_le_bytes()));
        b.extend([self.expedition as u8, self.reinforce as u8, self.defense as u8]);
        b.extend(self.anchor.to_le_bytes());
        b.into_iter()
    }
}

/// Startowa armia tubylcza prowincji (w tysięcznych osoby) z jej wielkości i infrastruktury startowej.
pub fn natives(area: u32, infrastructure: u8) -> u64 {
    area as u64 * NATIVES_PER_TILE * MILLI * NATIVES_INFRA_PCT[(infrastructure as usize).min(3)] / 100
}

/// Koszt kafla niczyjego na nizinie (tysięczne osoby) przy gęstości tubylców `density` (tysięczne na kafel).
pub fn native_cost(density: u64) -> u64 {
    NATIVE_TILE_COST * MILLI + density * NATIVE_DEFENSE_PCT / 100
}

/// Losowa liczba z przedziału `min..=max` ze skrótu `key`.
pub fn roll(key: u64, min: u64, max: u64) -> u64 {
    min + key % (max - min + 1)
}

/// Kolejność kafla frontu (mniej = wcześniej): teren (%), nasze kafle wokół (0–8) i skrót `key`
/// (`front_key`) jako „zawirowanie taktyczne”.
pub fn front_score(terrain_pct: u64, around: u32, key: u64) -> i64 {
    (terrain_pct + key % (TURBULENCE + 1)) as i64 - SURROUND_BONUS * around as i64
}

/// Koszt kafla obrońcy przy jego gęstości wojska `density` (tysięczne osoby na kafel) i współczynnikach
/// siły obrońcy i atakującego (%).
pub fn enemy_cost(density: u64, defender_pct: u64, attacker_pct: u64) -> u64 {
    ENEMY_TILE_COST * MILLI + density * DEFENSE_PCT / 100 * defender_pct / attacker_pct.max(1)
}

/// Kolejność kafli frontu: deterministyczny skrót (SplitMix64) kafla i ticka – fala idzie równo po
/// całym froncie zamiast od jednego brzegu.
pub fn front_key(tile: usize, tick: u32) -> u64 {
    let mut z = (tile as u64) ^ ((tick as u64) << 32) ^ FRONT_SALT;
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Dlaczego nie można zaatakować (interfejs pokazuje powód).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum AttackBlock {
    /// Przed końcem wyboru startu albo gracz bez państwa.
    NotPlaying,
    /// Front przeprawy ma już tyle wojska, ile przeprawa przepuści (`Crossing::max_troops`).
    Crossing,
    /// Cała prowincja już nasza.
    Owned,
    /// Brak styku naszego terytorium z żadną z prowincji (ani przez rzekę, jezioro, morze).
    NoContact,
    /// Za dużo frontów naraz (więcej – od wyższego centrum dowodzenia).
    Limit,
    /// Za dużo prowincji na jednym froncie (więcej – od wyższego centrum dowodzenia).
    Provinces,
    /// Desant morski: do centrum dowodzenia 2 jeden naraz, od 3 bez limitu.
    Landing,
    /// Desant morski wymaga naszego portu nad tym samym morzem.
    NoPort,
    /// Przyczółek odcięty od kraju bez wojska – atak z niego dopiero po desancie z posiłkami.
    Cutoff,
    /// Za mało paliwa na rejs.
    NoFuel,
    /// Prowincje należą do różnych trwających frontów.
    OtherFront,
    /// Za mało wojska.
    NoTroops,
}

/// Atak na prowincję dla ramki prowincji (z punktu widzenia oglądającego).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AttackReport {
    /// `None` – można atakować (albo dosłać wojsko do trwającego frontu).
    pub blocked: Option<AttackBlock>,
    /// Kafle prowincji do zajęcia: niczyje i innych graczy.
    pub neutral_tiles: u32,
    pub enemy_tiles: u32,
    /// Szacunkowy koszt kafla na nizinie (osoby wojska): niczyjego (tubylcy) i cudzego (przy obecnym
    /// zaopatrzeniu).
    pub neutral_cost: f64,
    pub enemy_cost: f64,
    /// Armia tubylcza, która została w prowincji (osoby).
    pub natives: f64,
    /// Wojsko dostępne do wysłania (osoby) – z kraju albo z garnizonu przyczółka (`from_garrison`).
    pub available: f64,
    pub from_garrison: bool,
    /// Trwający front oglądającego z tą prowincją: wojsko (osoby), zajęte kafle, prowincje frontu.
    pub troops: Option<f64>,
    pub taken: u32,
    pub front: Vec<u16>,
    /// Umocnienia obrońcy w prowincji (poziom, 0 – brak) – wliczone w `enemy_cost`.
    pub fortifications: u8,
    /// Desant morski (gdy prowincja jest osiągalna tylko z portu).
    pub landing: Option<LandingReport>,
    /// Przeprawa przez rzekę albo góry (front zaczyna się tylko przez nią).
    pub crossing: Option<CrossingReport>,
}

/// Desant morski dla interfejsu: port wypłynięcia, czas rejsu, paliwo i limit wojska.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct LandingReport {
    pub port: u16,
    pub days: f64,
    /// Paliwo na rejs 1 000 wojska (sztuki).
    pub fuel_per_1000: f64,
    /// Najwięcej wojska na desancie (poziom portu) i ile przewiezie obecne paliwo (osoby).
    pub max_troops: f64,
    pub fuel_troops: f64,
}

/// Płynący desant na mapie (widoczny dla wszystkich): państwo, wojsko, kafle początku i końca rejsu
/// i ticki wypłynięcia (po zaokrętowaniu, `EMBARK_TICKS`) i przybicia (stałe przez cały rejs – raport
/// zmienia się tylko przy posiłkach).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SailingReport {
    pub nation: u8,
    pub troops: f64,
    pub from_x: u32,
    pub from_y: u32,
    pub to_x: u32,
    pub to_y: u32,
    pub depart: u32,
    pub arrive: u32,
    /// Tury ładowania przed `depart` (pierścień na mapie): zaokrętowanie desantu albo przygotowanie przeprawy.
    pub embark: u32,
    /// Droga po morzu (x, y kolejnych punktów od portu do brzegu).
    pub path: Vec<[u32; 2]>,
}

/// Walczący front na mapie (widoczny dla wszystkich): flaga atakującego i „ile vs ile” przy linii frontu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FrontMark {
    /// Państwo atakujące i jego wojsko na froncie (osoby).
    pub nation: u8,
    pub troops: f64,
    /// Główny obrońca (najwięcej kafli frontu; 0 – tubylcy) i jego wojsko: państwo – w kraju razem z
    /// przeciwnym frontem (wspólny front ma jeden znacznik), tubylcy – w prowincjach frontu.
    pub enemy: u8,
    pub enemy_troops: f64,
    /// Kafel linii frontu najbliżej jej środka.
    pub x: u32,
    pub y: u32,
}

/// Pokój albo wojna oglądającego z innym udziałowcem prowincji (w tej prowincji): `war` – fronty zajmują
/// sobie kafle; `declared` – wypowiedzieliśmy (możemy wrócić do pokoju), `attacked` – wypowiedział on
/// (wojna, dopóki on jej nie odwoła).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct StanceReport {
    pub player: crate::protocol::PlayerId,
    pub war: bool,
    pub declared: bool,
    pub attacked: bool,
}

/// Trwający front dla paska zasobów.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AttackInfo {
    pub provinces: Vec<u16>,
    pub troops: f64,
    pub taken: u32,
}

/// Strona walki na froncie (obrońca w prowincjach frontu).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FrontSide {
    pub player: u16,
    /// Wojsko (osoby): atakujący – na tym froncie, obrońca – w kraju (broni całego państwa).
    pub troops: f64,
    /// Zaopatrzenie wojskowe w użyciu (`None` – bez zaopatrzenia).
    pub supply: Option<Supply>,
    /// Kafle prowincji frontu tej strony.
    pub tiles: u32,
}

/// Przeciwny front wspólnego frontu (`FrontReport::opponent`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FrontOpponent {
    /// Jego armia na froncie (`troops`) i jego kafle w prowincjach wspólnego frontu (`tiles`).
    pub side: FrontSide,
    pub lost: f64,
    pub pace: f64,
    pub strength: f64,
    pub fuel: f64,
}

/// Aktywny front oglądającego (jego albo na jego prowincje) – panel frontów.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FrontReport {
    pub provinces: Vec<u16>,
    /// Armia obrony prowincji (atakujący – obrońca), nie atak.
    pub defense: bool,
    /// Przyczółek odcięty od kraju – posiłki tylko desantem.
    pub expedition: bool,
    pub attacker: FrontSide,
    /// Inni właściciele kafli prowincji frontu (pusta lista – ziemia niczyja).
    pub defenders: Vec<FrontSide>,
    /// Część prowincji frontu zajęta przez atakującego (0–1), kafle niczyje i tubylcy (osoby).
    pub occupation: f64,
    pub neutral_tiles: u32,
    pub natives: f64,
    /// Desant morski i tury rejsu, które zostały (0 – na lądzie).
    pub landing: bool,
    pub sailing_days: f64,
    /// Przeprawa frontu (rzeka, góry) – limit wojska; `sailing_days` – przygotowanie i przejście.
    pub crossing: Option<CrossingKind>,
    /// Straty atakującego na tym froncie (osoby) i tempo (× – stosunek sił, `PACE_*`).
    pub lost: f64,
    pub pace: f64,
    /// Środek frontu (kafel) – „Idź do”.
    pub center_x: f64,
    pub center_y: f64,
    /// Czy atakuje oglądający.
    pub mine: bool,
    /// Wspólny front: przeciwny front w tych samych albo sąsiednich prowincjach (jego armia, straty, tempo).
    pub opponent: Option<FrontOpponent>,
    /// Siła zaopatrzenia wojska frontu (× – mnożnik ataku: bez zaopatrzenia `NO_SUPPLY_PCT`, lekkie, standardowe,
    /// ciężkie według `MILITARY` i centrum dowodzenia), pokrycie paliwem (0–1, przyspiesza front) i konie zamiast paliwa.
    pub strength: f64,
    pub fuel: f64,
    pub horses: bool,
    /// Dni bez zdobyczy i posiłków (zastój – po `STALL_TICKS` turach front się kończy).
    pub idle_days: f64,
    /// Kafle kotła w prowincjach frontu (okrążone, poza atakującym) – bronią się słabiej (`POCKET_DEFENSE_PCT`).
    pub pocket_tiles: u32,
}
