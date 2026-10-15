//! Nazwy prowincji (koniec fazy 3, `polish`): przymiotnik + rzeczownik ze słownika, z zgodnym rodzajem
//! („Stare Pole”, „Stara Wola”, „Stary Gród”), czasem w szyku „Marchia Wschodnia”.
//!
//! Dobór słów to **wagi**: z cech prowincji (`Features`: biom, zalesienie, wyżyna/nizina, wielkość,
//! wyspa, morze, góry, rzeka, jezioro, zasób, położenie na mapie) powstaje zbiór znaczników. Każde
//! słowo ma wymagania (`need` – wszystkie, `any` – choć jeden), wykluczenia (`avoid`) i premie (`like`):
//! słowo niespełniające wymagań ma wagę 0, tematyczne – dużą, ogólne – małą, a każda pasująca premia
//! ją zwiększa. Przymiotnik o tym samym temacie co rzeczownik ma wagę obniżoną („Leśna Puszcza”).
//! Nazwy na mapie są niepowtarzalne. Własny strumień losowości – nic innego się nie zmienia.

use std::collections::BTreeSet;

use crate::{Biome, MapData, Terrain, util::Rng};

const NAME_SALT: u64 = 0x2C1F_9A3B_6E57_D411;

// --- Znaczniki cech prowincji (bity) ---
const COAST: u64 = 1 << 0;
const RIVER: u64 = 1 << 1;
const LAKE: u64 = 1 << 2;
const MOUNT: u64 = 1 << 3;
const ISLAND: u64 = 1 << 4;
const UPLAND: u64 = 1 << 5;
const LOWLAND: u64 = 1 << 6;
/// Zalesienie ≥ 45% / ≥ 20% / < 8%.
const FOREST: u64 = 1 << 7;
const WOODED: u64 = 1 << 8;
const OPEN: u64 = 1 << 9;
const LARGE: u64 = 1 << 10;
const SMALL: u64 = 1 << 11;
const NORTH: u64 = 1 << 12;
const SOUTH: u64 = 1 << 13;
const EAST: u64 = 1 << 14;
const WEST: u64 = 1 << 15;
const CENTER: u64 = 1 << 16;
// Biomy i ich grupy.
const JUNGLE: u64 = 1 << 17;
const SAVANNA: u64 = 1 << 18;
const DESERT: u64 = 1 << 19;
const STEPPE: u64 = 1 << 20;
const MEDIT: u64 = 1 << 21;
const SUBTROP: u64 = 1 << 22;
const OCEANIC: u64 = 1 << 23;
const CONTI: u64 = 1 << 24;
const BOREAL: u64 = 1 << 25;
const TUNDRA: u64 = 1 << 26;
const ICE: u64 = 1 << 27;
const HOT: u64 = 1 << 28;
const DRY: u64 = 1 << 29;
const COLD: u64 = 1 << 30;
const WET: u64 = 1 << 31;
// Zasoby.
const R_GOLD: u64 = 1 << 32;
const R_IRON: u64 = 1 << 33;
const R_COAL: u64 = 1 << 34;
const R_COPPER: u64 = 1 << 35;
const R_METAL: u64 = 1 << 36;
const R_OIL: u64 = 1 << 37;
const R_GRAIN: u64 = 1 << 38;
const R_CROP: u64 = 1 << 39;
const R_CATTLE: u64 = 1 << 40;
const R_HORSE: u64 = 1 << 41;
const R_SHEEP: u64 = 1 << 42;
const R_FUR: u64 = 1 << 43;
const R_WOOD: u64 = 1 << 44;
const R_STONE: u64 = 1 << 45;
const R_MINERAL: u64 = 1 << 46;
const R_URAN: u64 = 1 << 47;
const SIDES: u64 = NORTH | SOUTH | EAST | WEST;
const ORES: u64 = R_IRON | R_COAL | R_COPPER | R_METAL | R_GOLD | R_URAN | R_MINERAL | R_STONE;

/// Waga słowa bez wymagań (ogólnego) i tematycznego; premia za każdą pasującą cechę z `like`.
const GENERIC: u32 = 2;
const THEMED: u32 = 8;
const LIKE: u32 = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gender {
    M,
    F,
    N,
}
use Gender::{F, M, N};

/// Słowo słownika: formy (rzeczownik – jedna, przymiotnik – m, ż, n), rodzaj (rzeczownik) i reguły.
#[derive(Clone, Copy)]
struct Word {
    forms: [&'static str; 3],
    gender: Gender,
    /// Wszystkie wymagane / co najmniej jedna z / żadna z / premie.
    need: u64,
    any: u64,
    avoid: u64,
    like: u64,
}

const fn noun(text: &'static str, gender: Gender, need: u64, any: u64, avoid: u64, like: u64) -> Word {
    Word { forms: [text, text, text], gender, need, any, avoid, like }
}

const fn adj(forms: [&'static str; 3], need: u64, any: u64, avoid: u64, like: u64) -> Word {
    Word { forms, gender: M, need, any, avoid, like }
}

#[rustfmt::skip]
const NOUNS: &[Word] = &[
    // Ogólne.
    noun("Pole", N, 0, 0, ICE | DESERT | JUNGLE | FOREST | MOUNT, OPEN | LOWLAND | R_GRAIN),
    noun("Wola", F, 0, 0, ICE | DESERT, 0),
    noun("Gród", M, 0, 0, 0, LARGE | RIVER),
    noun("Dwór", M, 0, 0, ICE | TUNDRA, R_GRAIN | R_HORSE),
    noun("Osada", F, 0, 0, 0, SMALL),
    noun("Kraina", F, 0, 0, 0, LARGE),
    noun("Ziemia", F, 0, 0, 0, LARGE),
    noun("Włość", F, 0, 0, ICE | DESERT, 0),
    noun("Siedlisko", N, 0, 0, ICE, SMALL),
    noun("Grodzisko", N, 0, 0, ICE, UPLAND | RIVER),
    noun("Folwark", M, 0, 0, ICE | DESERT | TUNDRA | JUNGLE | MOUNT, R_GRAIN | R_CATTLE | OPEN),
    noun("Sioło", N, 0, 0, ICE | DESERT, SMALL),
    noun("Marchia", F, 0, 0, 0, SIDES | LARGE),
    noun("Księstwo", N, 0, 0, SMALL, LARGE),
    noun("Rubież", F, 0, SIDES, 0, MOUNT | COAST),
    noun("Ostoja", F, 0, 0, 0, FOREST | MOUNT),
    noun("Uroczysko", N, 0, 0, ICE | DESERT, FOREST | LAKE),
    noun("Dolina", F, 0, RIVER | MOUNT | UPLAND, 0, RIVER),
    noun("Łąka", F, 0, 0, DESERT | ICE | JUNGLE | FOREST | MOUNT, OPEN | LOWLAND | R_CATTLE | R_HORSE),
    noun("Sad", M, 0, MEDIT | CONTI | SUBTROP | OCEANIC, FOREST, R_CROP),
    // Ukształtowanie.
    noun("Równina", F, LOWLAND, 0, MOUNT, OPEN | LARGE),
    noun("Nizina", F, LOWLAND, 0, MOUNT, RIVER),
    noun("Wyżyna", F, UPLAND, 0, 0, LARGE),
    noun("Wzgórze", N, UPLAND, 0, 0, SMALL),
    noun("Pogórze", N, MOUNT, 0, 0, UPLAND),
    noun("Podgórze", N, MOUNT, 0, 0, LOWLAND),
    noun("Przełęcz", F, MOUNT, 0, 0, UPLAND),
    noun("Grań", F, MOUNT, 0, 0, UPLAND | COLD),
    noun("Turnia", F, MOUNT, 0, 0, UPLAND),
    noun("Skała", F, MOUNT, 0, 0, R_STONE),
    // Woda.
    noun("Wybrzeże", N, COAST, 0, 0, LARGE),
    noun("Przystań", F, COAST, 0, 0, SMALL),
    noun("Zatoka", F, COAST, 0, 0, 0),
    noun("Port", M, COAST, 0, 0, RIVER),
    noun("Mierzeja", F, COAST | LOWLAND, 0, MOUNT, OPEN),
    noun("Cypel", M, COAST, 0, 0, SMALL),
    noun("Pomorze", N, COAST, 0, 0, LARGE),
    noun("Wyspa", F, ISLAND, 0, 0, 0),
    noun("Ostrów", M, ISLAND, 0, 0, SMALL),
    noun("Kępa", F, RIVER, 0, 0, SMALL | LOWLAND),
    noun("Bród", M, RIVER, 0, 0, 0),
    noun("Łęg", M, RIVER, 0, DRY | COLD, WOODED | LOWLAND),
    noun("Nadbrzeże", N, RIVER, 0, 0, 0),
    noun("Rozlewisko", N, RIVER, 0, MOUNT, LAKE | LOWLAND),
    noun("Powiśle", N, RIVER, 0, 0, LOWLAND | LARGE),
    noun("Pojezierze", N, LAKE, 0, 0, LARGE),
    noun("Toń", F, LAKE, 0, 0, 0),
    noun("Zalew", M, LAKE, 0, 0, COAST),
    // Las.
    noun("Puszcza", F, FOREST, 0, DRY, LARGE),
    noun("Bór", M, FOREST, 0, HOT, BOREAL),
    noun("Knieja", F, FOREST, 0, HOT | DRY, 0),
    noun("Dąbrowa", F, WOODED, 0, COLD | DRY | JUNGLE, CONTI | OCEANIC),
    noun("Gaj", M, WOODED, 0, ICE | TUNDRA, SMALL),
    noun("Zalesie", N, WOODED, 0, 0, 0),
    noun("Tartak", M, R_WOOD, 0, 0, FOREST),
    // Klimat.
    noun("Dżungla", F, JUNGLE, 0, 0, FOREST),
    noun("Gąszcz", M, FOREST, 0, COLD, JUNGLE),
    noun("Sawanna", F, SAVANNA, 0, 0, OPEN),
    noun("Step", M, STEPPE, 0, 0, OPEN | LARGE),
    noun("Pustynia", F, DESERT, 0, 0, LARGE),
    noun("Wydma", F, DESERT, 0, 0, COAST | SMALL),
    noun("Oaza", F, DESERT, RIVER | LAKE, 0, 0),
    noun("Pustkowie", N, 0, DESERT | TUNDRA | ICE, 0, OPEN),
    noun("Tundra", F, TUNDRA, 0, 0, OPEN),
    noun("Zmarzlina", F, 0, TUNDRA | ICE, 0, 0),
    noun("Śnieżnik", M, COLD, MOUNT | UPLAND, 0, 0),
    noun("Kra", F, ICE | COAST, 0, 0, 0),
    noun("Tajga", F, BOREAL, 0, 0, FOREST),
    noun("Winnica", F, MEDIT, 0, FOREST, UPLAND),
    // Zasoby.
    noun("Pastwisko", N, 0, R_CATTLE | R_SHEEP | R_HORSE, FOREST, OPEN),
    noun("Stadnina", F, R_HORSE, 0, 0, OPEN),
    noun("Kopalnia", F, 0, ORES, 0, MOUNT | UPLAND),
    noun("Huta", F, 0, R_IRON | R_METAL | R_COPPER, 0, R_COAL),
    noun("Żupa", F, R_MINERAL, 0, 0, 0),
    noun("Kamieniołom", M, R_STONE, 0, 0, UPLAND),
];

#[rustfmt::skip]
const ADJECTIVES: &[Word] = &[
    // Ogólne.
    adj(["Stary", "Stara", "Stare"], 0, 0, 0, 0),
    adj(["Nowy", "Nowa", "Nowe"], 0, 0, 0, 0),
    adj(["Cichy", "Cicha", "Ciche"], 0, 0, 0, FOREST | LAKE),
    adj(["Dziki", "Dzika", "Dzikie"], 0, 0, 0, FOREST | JUNGLE | MOUNT | ISLAND),
    adj(["Jasny", "Jasna", "Jasne"], 0, 0, FOREST | BOREAL, OPEN),
    adj(["Ciemny", "Ciemna", "Ciemne"], 0, 0, OPEN | DESERT, FOREST | BOREAL | JUNGLE),
    adj(["Wolny", "Wolna", "Wolne"], 0, 0, 0, OPEN),
    adj(["Święty", "Święta", "Święte"], 0, 0, 0, LAKE | MOUNT),
    adj(["Królewski", "Królewska", "Królewskie"], 0, 0, SMALL, LARGE | R_GOLD),
    adj(["Książęcy", "Książęca", "Książęce"], 0, 0, 0, LARGE),
    adj(["Odległy", "Odległa", "Odległe"], 0, 0, CENTER, ISLAND | SIDES),
    adj(["Wielki", "Wielka", "Wielkie"], LARGE, 0, 0, 0),
    adj(["Mały", "Mała", "Małe"], SMALL, 0, 0, 0),
    adj(["Szeroki", "Szeroka", "Szerokie"], 0, LARGE | LOWLAND | OPEN, MOUNT | SMALL, 0),
    adj(["Długi", "Długa", "Długie"], 0, LARGE | RIVER | COAST, SMALL, 0),
    adj(["Zielony", "Zielona", "Zielone"], 0, 0, DESERT | ICE | TUNDRA | STEPPE | DRY, WOODED | WET | OCEANIC),
    adj(["Czarny", "Czarna", "Czarne"], 0, 0, ICE | DESERT, FOREST | BOREAL | R_COAL | R_OIL),
    adj(["Biały", "Biała", "Białe"], 0, ICE | TUNDRA | R_STONE | MOUNT | COLD, JUNGLE, COLD),
    adj(["Złoty", "Złota", "Złote"], 0, R_GOLD | R_GRAIN | SAVANNA | STEPPE | DESERT, 0, R_GOLD),
    adj(["Srebrny", "Srebrna", "Srebrne"], 0, LAKE | RIVER | COLD | R_METAL, 0, 0),
    adj(["Rudy", "Ruda", "Rude"], 0, R_IRON | R_COPPER | DESERT, 0, 0),
    adj(["Modry", "Modra", "Modre"], 0, LAKE | COAST, 0, 0),
    // Położenie na mapie.
    adj(["Północny", "Północna", "Północne"], NORTH, 0, 0, 0),
    adj(["Południowy", "Południowa", "Południowe"], SOUTH, 0, 0, 0),
    adj(["Wschodni", "Wschodnia", "Wschodnie"], EAST, 0, 0, 0),
    adj(["Zachodni", "Zachodnia", "Zachodnie"], WEST, 0, 0, 0),
    adj(["Środkowy", "Środkowa", "Środkowe"], CENTER, 0, 0, LARGE),
    // Ukształtowanie.
    adj(["Górny", "Górna", "Górne"], 0, UPLAND | MOUNT, LOWLAND, 0),
    adj(["Dolny", "Dolna", "Dolne"], 0, LOWLAND | RIVER, UPLAND, 0),
    adj(["Wysoki", "Wysoka", "Wysokie"], 0, UPLAND | MOUNT, LOWLAND, 0),
    adj(["Niski", "Niska", "Niskie"], LOWLAND, 0, MOUNT, 0),
    adj(["Płaski", "Płaska", "Płaskie"], LOWLAND, 0, MOUNT | UPLAND, OPEN),
    adj(["Skalny", "Skalna", "Skalne"], MOUNT, 0, 0, R_STONE),
    adj(["Górski", "Górska", "Górskie"], MOUNT, 0, 0, UPLAND),
    adj(["Stromy", "Stroma", "Strome"], MOUNT, 0, 0, UPLAND),
    adj(["Kamienny", "Kamienna", "Kamienne"], 0, MOUNT | UPLAND | R_STONE, 0, 0),
    // Woda.
    adj(["Nadmorski", "Nadmorska", "Nadmorskie"], COAST, 0, 0, 0),
    adj(["Morski", "Morska", "Morskie"], COAST, 0, 0, ISLAND),
    adj(["Słony", "Słona", "Słone"], 0, COAST | R_MINERAL, 0, DESERT),
    adj(["Bursztynowy", "Bursztynowa", "Bursztynowe"], COAST, 0, HOT, COLD | BOREAL),
    adj(["Nadrzeczny", "Nadrzeczna", "Nadrzeczne"], RIVER, 0, 0, 0),
    adj(["Bystry", "Bystra", "Bystre"], RIVER, 0, 0, MOUNT),
    adj(["Jeziorny", "Jeziorna", "Jeziorne"], LAKE, 0, 0, 0),
    adj(["Mokry", "Mokra", "Mokre"], 0, RIVER | LAKE | WET, DRY, LOWLAND),
    adj(["Wyspiarski", "Wyspiarska", "Wyspiarskie"], ISLAND, 0, 0, 0),
    adj(["Samotny", "Samotna", "Samotne"], ISLAND, 0, 0, SMALL),
    // Las.
    adj(["Leśny", "Leśna", "Leśne"], 0, FOREST | WOODED, 0, 0),
    adj(["Borowy", "Borowa", "Borowe"], FOREST, 0, HOT, BOREAL),
    adj(["Dębowy", "Dębowa", "Dębowe"], WOODED, 0, COLD | JUNGLE | DRY, CONTI),
    adj(["Sosnowy", "Sosnowa", "Sosnowe"], 0, BOREAL | WOODED, HOT, BOREAL),
    adj(["Świerkowy", "Świerkowa", "Świerkowe"], BOREAL, 0, 0, FOREST),
    adj(["Bezleśny", "Bezleśna", "Bezleśne"], OPEN, 0, DESERT | ICE, 0),
    // Klimat.
    adj(["Gorący", "Gorąca", "Gorące"], HOT, 0, 0, DRY),
    adj(["Parny", "Parna", "Parne"], 0, JUNGLE | SUBTROP, 0, 0),
    adj(["Suchy", "Sucha", "Suche"], DRY, 0, 0, 0),
    adj(["Piaszczysty", "Piaszczysta", "Piaszczyste"], 0, DESERT | COAST, FOREST | COLD, DESERT),
    adj(["Spalony", "Spalona", "Spalone"], DESERT, 0, 0, 0),
    adj(["Wietrzny", "Wietrzna", "Wietrzne"], 0, STEPPE | COAST | MOUNT | OPEN, FOREST, 0),
    adj(["Mroźny", "Mroźna", "Mroźne"], COLD, 0, 0, 0),
    adj(["Lodowy", "Lodowa", "Lodowe"], ICE, 0, 0, 0),
    adj(["Śnieżny", "Śnieżna", "Śnieżne"], COLD, 0, 0, MOUNT),
    adj(["Mglisty", "Mglista", "Mgliste"], 0, OCEANIC | LAKE, DRY | HOT, OCEANIC),
    adj(["Deszczowy", "Deszczowa", "Deszczowe"], 0, OCEANIC | JUNGLE, DRY, 0),
    adj(["Słoneczny", "Słoneczna", "Słoneczne"], 0, MEDIT | SAVANNA | STEPPE | HOT, COLD | OCEANIC, 0),
    adj(["Oliwny", "Oliwna", "Oliwne"], MEDIT, 0, 0, 0),
    adj(["Winny", "Winna", "Winne"], MEDIT, 0, 0, UPLAND),
    // Zasoby.
    adj(["Żelazny", "Żelazna", "Żelazne"], R_IRON, 0, 0, 0),
    adj(["Miedziany", "Miedziana", "Miedziane"], R_COPPER, 0, 0, 0),
    adj(["Węglowy", "Węglowa", "Węglowe"], R_COAL, 0, 0, 0),
    adj(["Smolny", "Smolna", "Smolne"], 0, R_OIL | R_WOOD, HOT, BOREAL),
    adj(["Solny", "Solna", "Solne"], R_MINERAL, 0, 0, 0),
    adj(["Kruszcowy", "Kruszcowa", "Kruszcowe"], 0, R_METAL | R_URAN, 0, 0),
    adj(["Żytni", "Żytnia", "Żytnie"], R_GRAIN, 0, HOT, 0),
    adj(["Koński", "Końska", "Końskie"], R_HORSE, 0, 0, 0),
    adj(["Owczy", "Owcza", "Owcze"], R_SHEEP, 0, 0, 0),
    adj(["Bobrowy", "Bobrowa", "Bobrowe"], R_FUR, 0, 0, RIVER | LAKE),
];

/// Próby wylosowania niepowtarzalnej nazwy, zanim dostanie numer porządkowy („Stare Pole II”).
const TRIES: usize = 24;

/// Waga słowa dla prowincji o cechach `f` (0 = nie pasuje).
fn weight(w: &Word, f: u64) -> u32 {
    if w.need & f != w.need || (w.any != 0 && w.any & f == 0) || w.avoid & f != 0 {
        return 0;
    }
    let base = if w.need | w.any == 0 { GENERIC } else { THEMED };
    base + LIKE * (w.like & f).count_ones()
}

/// Losowanie z wagami (`None`, gdy wszystkie wagi są zerowe).
fn pick<'a>(rng: &mut Rng, words: impl Iterator<Item = (&'a Word, u32)> + Clone) -> Option<&'a Word> {
    let total: u32 = words.clone().map(|(_, w)| w).sum();
    if total == 0 {
        return None;
    }
    let mut r = rng.below(total as usize) as u32;
    for (word, w) in words {
        if r < w {
            return Some(word);
        }
        r -= w;
    }
    None
}

/// Nazwa dla prowincji o cechach `f`: rzeczownik i przymiotnik według wag (przymiotnik o tym samym
/// temacie co rzeczownik – waga ×¼, o tym samym rdzeniu – wcale), w co czwartej nazwie szyk
/// „Marchia Wschodnia”.
fn compose(rng: &mut Rng, f: u64) -> String {
    let noun = pick(rng, NOUNS.iter().map(|w| (w, weight(w, f)))).unwrap_or(&NOUNS[0]);
    let theme = noun.need | noun.any;
    let adjective = pick(
        rng,
        ADJECTIVES.iter().map(|w| {
            let wt = weight(w, f);
            // Ten sam rdzeń co rzeczownik („Wolna Wola”) – nigdy; ten sam temat („Leśna Puszcza”) – rzadko.
            let same_root = w.forms[0].chars().take(3).eq(noun.forms[0].chars().take(3));
            (
                w,
                if same_root {
                    0
                } else if (w.need | w.any) & theme != 0 {
                    wt.div_ceil(4)
                } else {
                    wt
                },
            )
        }),
    )
    .unwrap_or(&ADJECTIVES[0]);
    let a = adjective.forms[noun.gender as usize];
    let n = noun.forms[0];
    if rng.below(4) == 0 { format!("{n} {a}") } else { format!("{a} {n}") }
}

/// Cechy prowincji (znaczniki) z mapy: biom, zalesienie, wyżyna/nizina, wielkość, wyspa, sąsiedztwa,
/// zasób i położenie środka na mapie.
fn features(map: &MapData) -> Vec<u64> {
    let n = map.provinces.len();
    let (mut forest, mut highland) = (vec![0u32; n + 1], vec![0u32; n + 1]);
    for (i, &p) in map.province.iter().enumerate() {
        if p == 0 {
            continue;
        }
        forest[p as usize] += (map.forest[i] >= 128) as u32;
        highland[p as usize] += (map.terrain[i] == Terrain::Highlands as u8) as u32;
    }
    let graph = crate::province_graph(map);
    let mean = map.provinces.iter().map(|p| p.area as u64).sum::<u64>() / n.max(1) as u64;
    let (w, h) = (map.width.max(1) as u64, map.height.max(1) as u64);
    map.provinces
        .iter()
        .map(|pr| {
            let id = pr.id as usize;
            let area = pr.area.max(1) as u64;
            let mut f = 0;
            f |= if pr.coastal { COAST } else { 0 };
            f |= if pr.river { RIVER } else { 0 };
            f |= if pr.lake { LAKE } else { 0 };
            f |= if pr.mountains { MOUNT } else { 0 };
            // Wyspa: prowincja bez sąsiadów po lądzie (jedyna na swoim lądzie), nad morzem.
            f |= if pr.coastal && graph[id - 1].is_empty() { ISLAND } else { 0 };
            let hi = highland[id] as u64 * 100 / area;
            f |= if hi > 50 {
                UPLAND
            } else if hi < 15 {
                LOWLAND
            } else {
                0
            };
            let fo = forest[id] as u64 * 100 / area;
            f |= if fo >= 45 {
                FOREST | WOODED
            } else if fo >= 20 {
                WOODED
            } else if fo < 8 {
                OPEN
            } else {
                0
            };
            f |= if area * 100 > mean * 112 {
                LARGE
            } else if area * 100 < mean * 88 {
                SMALL
            } else {
                0
            };
            // Położenie: skrajne 30% mapy w każdą stronę, środek – w środkowych 40% w obu osiach.
            let (x, y) = (pr.center_x as u64 * 100 / w, pr.center_y as u64 * 100 / h);
            f |= if y < 30 {
                NORTH
            } else if y > 70 {
                SOUTH
            } else {
                0
            };
            f |= if x < 30 {
                WEST
            } else if x > 70 {
                EAST
            } else {
                0
            };
            f |= if (30..=70).contains(&x) && (30..=70).contains(&y) { CENTER } else { 0 };
            f | biome_tags(pr.biome) | resource_tags(pr.resource)
        })
        .collect()
}

fn biome_tags(biome: u8) -> u64 {
    match biome {
        b if b == Biome::Rainforest as u8 => JUNGLE | HOT | WET,
        b if b == Biome::Savanna as u8 => SAVANNA | HOT | DRY,
        b if b == Biome::Desert as u8 => DESERT | HOT | DRY,
        b if b == Biome::Steppe as u8 => STEPPE | DRY,
        b if b == Biome::Mediterranean as u8 => MEDIT | HOT,
        b if b == Biome::Subtropical as u8 => SUBTROP | HOT | WET,
        b if b == Biome::Oceanic as u8 => OCEANIC | WET,
        b if b == Biome::HotSummer as u8 || b == Biome::WarmSummer as u8 => CONTI,
        b if b == Biome::Boreal as u8 => BOREAL,
        b if b == Biome::Taiga as u8 => BOREAL | COLD,
        b if b == Biome::Tundra as u8 => TUNDRA | COLD,
        b if b == Biome::IceSheet as u8 => ICE | COLD,
        _ => 0,
    }
}

/// Znaczniki zasobu (numer z `RESOURCE_NAMES`).
fn resource_tags(resource: u8) -> u64 {
    match crate::resources::RESOURCE_NAMES.get(resource as usize).copied().unwrap_or("") {
        "Złoto" => R_GOLD,
        "Rudy żelaza" => R_IRON,
        "Węgiel kamienny" | "Węgiel brunatny" => R_COAL,
        "Miedź" => R_COPPER,
        "Mangan" | "Chrom" | "Nikiel" | "Wolfram" | "Molibden" | "Ołów" | "Cynk" | "Aluminium" | "Cyna" => R_METAL,
        "Ropa naftowa" => R_OIL,
        "Uran" => R_URAN,
        "Zboża" | "Ryż" | "Ziemniaki" => R_GRAIN,
        "Bawełna" | "Włókna roślinne" | "Tytoń" | "Trzcina cukrowa" | "Kawa" | "Herbata" | "Kauczuk" => R_CROP,
        "Bydło" => R_CATTLE,
        "Konie" => R_HORSE,
        "Wełna" => R_SHEEP,
        "Futra" => R_FUR,
        "Drewno" => R_WOOD,
        "Cement" => R_STONE,
        "Surowce nawozowe" | "Saletra" | "Siarka" => R_MINERAL,
        _ => 0,
    }
}

/// Nadaje nazwę każdej prowincji (`Province::name`). Wymaga ostatecznej numeracji prowincji i zasobów.
pub fn assign(map: &mut MapData, seed: u32) {
    let features = features(map);
    let mut rng = Rng::new(seed as u64 ^ NAME_SALT);
    let mut used = BTreeSet::new();
    for (pr, &f) in map.provinces.iter_mut().zip(&features) {
        let mut name = String::new();
        for _ in 0..TRIES {
            name = compose(&mut rng, f);
            if !used.contains(&name) {
                break;
            }
        }
        if used.contains(&name) {
            let base = name.clone();
            let mut k = 2;
            while used.contains(&name) {
                name = format!("{base} {}", roman(k));
                k += 1;
            }
        }
        used.insert(name.clone());
        pr.name = name;
    }
}

fn roman(n: usize) -> String {
    const R: [(usize, &str); 5] = [(10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut n = n;
    let mut out = String::new();
    for (v, s) in R {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Słowo o danym tekście (dowolna forma).
    fn find(words: &[Word], text: &str) -> Word {
        *words.iter().find(|w| w.forms.contains(&text)).unwrap_or_else(|| panic!("brak słowa {text}"))
    }

    #[test]
    fn adjectives_agree_with_the_noun_and_fit_the_province() {
        let mut rng = Rng::new(7);
        for f in [
            COAST | DESERT | HOT | DRY | OPEN | SOUTH,
            MOUNT | UPLAND | COLD | TUNDRA | NORTH,
            FOREST | WOODED | BOREAL | RIVER,
        ] {
            for _ in 0..300 {
                let name = compose(&mut rng, f);
                let words: Vec<&str> = name.split(' ').collect();
                assert_eq!(words.len(), 2, "{name}");
                let noun_text = *words.iter().find(|t| NOUNS.iter().any(|w| w.forms[0] == **t)).unwrap();
                let noun = find(NOUNS, noun_text);
                let adjective = find(ADJECTIVES, words.iter().find(|t| **t != noun_text).unwrap());
                assert!(
                    adjective.forms[noun.gender as usize] == words[0]
                        || adjective.forms[noun.gender as usize] == words[1],
                    "{name}: zły rodzaj"
                );
                assert!(weight(&noun, f) > 0 && weight(&adjective, f) > 0, "{name}: słowo nie pasuje do cech");
            }
        }
    }

    #[test]
    fn themed_words_win_and_impossible_ones_never_appear() {
        let mut rng = Rng::new(3);
        let desert = DESERT | HOT | DRY | OPEN;
        let names: Vec<String> = (0..400).map(|_| compose(&mut rng, desert)).collect();
        assert!(
            names.iter().all(|n| !n.contains("Zielon") && !n.contains("Lodow") && !n.contains("Puszcza")),
            "sprzeczne słowa"
        );
        let themed = names
            .iter()
            .filter(|n| {
                ["Pustynia", "Wydma", "Pustkowie", "Suchy", "Sucha", "Suche", "Spalon", "Gorąc", "Piaszczyst"]
                    .iter()
                    .any(|w| n.contains(w))
            })
            .count();
        assert!(themed * 2 > names.len(), "większość nazw pustynnych: {themed}/400");
        // Oaza tylko z wodą.
        assert!(names.iter().all(|n| !n.contains("Oaza")));
        assert_eq!(weight(&find(NOUNS, "Oaza"), desert | RIVER), THEMED);
    }

    #[test]
    fn every_word_can_appear_somewhere() {
        // Każde słowo ma zestaw cech, przy którym ma wagę > 0 (wymagania nie są ze sobą sprzeczne).
        for w in NOUNS.iter().chain(ADJECTIVES) {
            let f = w.need | (w.any & w.any.wrapping_neg());
            assert!(weight(w, f) > 0, "{} nie może się pojawić", w.forms[0]);
        }
    }

    #[test]
    fn roman_numerals() {
        assert_eq!([2, 3, 4, 9, 14].map(roman), ["II", "III", "IV", "IX", "XIV"]);
    }
}
