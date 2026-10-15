//! Nazwy prowincji (koniec fazy 3, `polish`): przymiotnik + rzeczownik ze słownika, z zgodnym rodzajem
//! („Stare Pole”, „Stara Wola”, „Stary Gród”), czasem w szyku „Wola Wielka”. Rzeczowniki i przymiotniki
//! tematyczne dobierane według stałych właściwości prowincji (morze, rzeka, jezioro, góry, biom), reszta
//! z puli ogólnej. Nazwy na mapie są niepowtarzalne. Własny strumień losowości – nic innego się nie zmienia.

use std::collections::BTreeSet;

use crate::{Biome, MapData, util::Rng};

const NAME_SALT: u64 = 0x2C1F_9A3B_6E57_D411;

#[derive(Clone, Copy)]
enum Gender {
    M,
    F,
    N,
}
use Gender::{F, M, N};

/// Rzeczownik i jego rodzaj.
type Noun = (&'static str, Gender);
/// Przymiotnik w rodzaju męskim, żeńskim i nijakim.
type Adjective = [&'static str; 3];

const NOUNS: &[Noun] = &[
    ("Pole", N),
    ("Wola", F),
    ("Gród", M),
    ("Dwór", M),
    ("Gaj", M),
    ("Dolina", F),
    ("Kraina", F),
    ("Ziemia", F),
    ("Osada", F),
    ("Wieś", F),
    ("Polana", F),
    ("Rubież", F),
    ("Marchia", F),
    ("Księstwo", N),
    ("Hrabstwo", N),
    ("Włość", F),
    ("Siedlisko", N),
    ("Grodzisko", N),
    ("Ostoja", F),
    ("Uroczysko", N),
    ("Łąka", F),
    ("Błonie", N),
    ("Równina", F),
    ("Nizina", F),
    ("Wrzosowisko", N),
    ("Zagroda", F),
    ("Folwark", M),
    ("Sioło", N),
    ("Przysiółek", M),
    ("Jar", M),
    ("Wzgórze", N),
    ("Kopiec", M),
];
const COAST_NOUNS: &[Noun] =
    &[("Wybrzeże", N), ("Przystań", F), ("Zatoka", F), ("Port", M), ("Pomorze", N), ("Mierzeja", F), ("Cypel", M)];
const RIVER_NOUNS: &[Noun] =
    &[("Bród", M), ("Powiśle", N), ("Kępa", F), ("Ostrów", M), ("Rozlewisko", N), ("Nadbrzeże", N), ("Łęg", M)];
const LAKE_NOUNS: &[Noun] = &[("Pojezierze", N), ("Toń", F), ("Ostrów", M), ("Rozlewisko", N), ("Zalew", M)];
const MOUNTAIN_NOUNS: &[Noun] =
    &[("Podgórze", N), ("Zagórze", N), ("Pogórze", N), ("Przełęcz", F), ("Skała", F), ("Turnia", F), ("Grań", F)];
const FOREST_NOUNS: &[Noun] =
    &[("Puszcza", F), ("Bór", M), ("Dąbrowa", F), ("Zalesie", N), ("Przylesie", N), ("Knieja", F), ("Gąszcz", M)];
const DRY_NOUNS: &[Noun] = &[("Pustkowie", N), ("Step", M), ("Oaza", F), ("Wydma", F), ("Pustynia", F), ("Sawanna", F)];
const COLD_NOUNS: &[Noun] = &[("Pustkowie", N), ("Tundra", F), ("Kra", F), ("Zmarzlina", F), ("Śnieżnik", M)];

const ADJECTIVES: &[Adjective] = &[
    ["Stary", "Stara", "Stare"],
    ["Nowy", "Nowa", "Nowe"],
    ["Wielki", "Wielka", "Wielkie"],
    ["Mały", "Mała", "Małe"],
    ["Górny", "Górna", "Górne"],
    ["Dolny", "Dolna", "Dolne"],
    ["Zielony", "Zielona", "Zielone"],
    ["Czarny", "Czarna", "Czarne"],
    ["Biały", "Biała", "Białe"],
    ["Złoty", "Złota", "Złote"],
    ["Srebrny", "Srebrna", "Srebrne"],
    ["Cichy", "Cicha", "Ciche"],
    ["Dziki", "Dzika", "Dzikie"],
    ["Jasny", "Jasna", "Jasne"],
    ["Ciemny", "Ciemna", "Ciemne"],
    ["Wysoki", "Wysoka", "Wysokie"],
    ["Szeroki", "Szeroka", "Szerokie"],
    ["Długi", "Długa", "Długie"],
    ["Kamienny", "Kamienna", "Kamienne"],
    ["Wietrzny", "Wietrzna", "Wietrzne"],
    ["Słoneczny", "Słoneczna", "Słoneczne"],
    ["Królewski", "Królewska", "Królewskie"],
    ["Książęcy", "Książęca", "Książęce"],
    ["Wolny", "Wolna", "Wolne"],
    ["Święty", "Święta", "Święte"],
    ["Północny", "Północna", "Północne"],
    ["Południowy", "Południowa", "Południowe"],
    ["Wschodni", "Wschodnia", "Wschodnie"],
    ["Zachodni", "Zachodnia", "Zachodnie"],
    ["Modry", "Modra", "Modre"],
    ["Rudy", "Ruda", "Rude"],
    ["Dumny", "Dumna", "Dumne"],
    ["Odległy", "Odległa", "Odległe"],
];
const COAST_ADJECTIVES: &[Adjective] = &[
    ["Nadmorski", "Nadmorska", "Nadmorskie"],
    ["Bursztynowy", "Bursztynowa", "Bursztynowe"],
    ["Słony", "Słona", "Słone"],
    ["Morski", "Morska", "Morskie"],
];
const WATER_ADJECTIVES: &[Adjective] = &[
    ["Nadrzeczny", "Nadrzeczna", "Nadrzeczne"],
    ["Mokry", "Mokra", "Mokre"],
    ["Wodny", "Wodna", "Wodne"],
    ["Bystry", "Bystra", "Bystre"],
];
const MOUNTAIN_ADJECTIVES: &[Adjective] =
    &[["Górski", "Górska", "Górskie"], ["Skalny", "Skalna", "Skalne"], ["Stromy", "Stroma", "Strome"]];
const FOREST_ADJECTIVES: &[Adjective] =
    &[["Leśny", "Leśna", "Leśne"], ["Dębowy", "Dębowa", "Dębowe"], ["Sosnowy", "Sosnowa", "Sosnowe"]];
const DRY_ADJECTIVES: &[Adjective] =
    &[["Suchy", "Sucha", "Suche"], ["Piaszczysty", "Piaszczysta", "Piaszczyste"], ["Gorący", "Gorąca", "Gorące"]];
const COLD_ADJECTIVES: &[Adjective] =
    &[["Mroźny", "Mroźna", "Mroźne"], ["Lodowy", "Lodowa", "Lodowe"], ["Śnieżny", "Śnieżna", "Śnieżne"]];

/// Próby wylosowania niepowtarzalnej nazwy, zanim dostanie numer porządkowy („Stare Pole II”).
const TRIES: usize = 24;

/// Nadaje nazwę każdej prowincji (`Province::name`). Wymaga ostatecznej numeracji prowincji.
pub fn assign(map: &mut MapData, seed: u32) {
    let mut rng = Rng::new(seed as u64 ^ NAME_SALT);
    let mut used = BTreeSet::new();
    for pr in &mut map.provinces {
        let biome = pr.biome;
        let forest = biome == Biome::Rainforest as u8 || biome == Biome::Boreal as u8 || biome == Biome::Taiga as u8;
        let dry = biome == Biome::Desert as u8 || biome == Biome::Steppe as u8 || biome == Biome::Savanna as u8;
        let cold = biome == Biome::Tundra as u8 || biome == Biome::IceSheet as u8;
        // Pule tematyczne według stałych właściwości prowincji.
        let mut nouns: Vec<&[Noun]> = Vec::new();
        let mut adjectives: Vec<&[Adjective]> = Vec::new();
        for (has, n, a) in [
            (pr.coastal, COAST_NOUNS, COAST_ADJECTIVES),
            (pr.river, RIVER_NOUNS, WATER_ADJECTIVES),
            (pr.lake, LAKE_NOUNS, WATER_ADJECTIVES),
            (pr.mountains, MOUNTAIN_NOUNS, MOUNTAIN_ADJECTIVES),
            (forest, FOREST_NOUNS, FOREST_ADJECTIVES),
            (dry, DRY_NOUNS, DRY_ADJECTIVES),
            (cold, COLD_NOUNS, COLD_ADJECTIVES),
        ] {
            if has {
                nouns.push(n);
                adjectives.push(a);
            }
        }
        let mut name = String::new();
        for _ in 0..TRIES {
            name = compose(&mut rng, &nouns, &adjectives);
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

/// Jedna nazwa: rzeczownik (w połowie przypadków z puli tematycznej) i przymiotnik w jego rodzaju
/// (w 40% tematyczny); w co czwartej nazwie szyk „Wola Wielka”.
fn compose(rng: &mut Rng, nouns: &[&'static [Noun]], adjectives: &[&'static [Adjective]]) -> String {
    let (noun, gender) = if !nouns.is_empty() && rng.below(2) == 0 {
        let pool = nouns[rng.below(nouns.len())];
        pool[rng.below(pool.len())]
    } else {
        NOUNS[rng.below(NOUNS.len())]
    };
    let adjective = if !adjectives.is_empty() && rng.below(5) < 2 {
        let pool = adjectives[rng.below(adjectives.len())];
        pool[rng.below(pool.len())]
    } else {
        ADJECTIVES[rng.below(ADJECTIVES.len())]
    };
    let adjective = adjective[gender as usize];
    if rng.below(4) == 0 { format!("{noun} {adjective}") } else { format!("{adjective} {noun}") }
}

fn roman(n: usize) -> String {
    const R: [(usize, &str); 7] = [(10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"), (0, ""), (0, "")];
    let mut n = n;
    let mut out = String::new();
    for (v, s) in R {
        while v > 0 && n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjectives_agree_with_the_noun() {
        let mut rng = Rng::new(7);
        for _ in 0..500 {
            let name = compose(&mut rng, &[COAST_NOUNS], &[COAST_ADJECTIVES]);
            let words: Vec<&str> = name.split(' ').collect();
            assert_eq!(words.len(), 2, "{name}");
            let (noun, gender) = NOUNS
                .iter()
                .chain(COAST_NOUNS)
                .find(|(n, _)| words.contains(n))
                .copied()
                .unwrap_or_else(|| panic!("brak rzeczownika w {name}"));
            let adjective = words.iter().find(|w| **w != noun).unwrap();
            let forms = ADJECTIVES.iter().chain(COAST_ADJECTIVES).find(|f| f.contains(adjective)).unwrap();
            assert_eq!(forms[gender as usize], *adjective, "{name}: zły rodzaj przymiotnika");
        }
    }

    #[test]
    fn roman_numerals() {
        assert_eq!([2, 3, 4, 9, 14].map(roman), ["II", "III", "IV", "IX", "XIV"]);
    }
}
