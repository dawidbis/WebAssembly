//! Rynek surowców (docs/gdd/rynek.md): handel automatyczny raz dziennie.
//!
//! Gracz ustawia **bilans** każdego surowca (−100% import … 0 … +100% eksport). Codziennie, dla każdego
//! surowca osobno, niezależnie od produkcji fabryk:
//!
//! - **przepustowość handlu** państwa (jednostki na dzień, ta sama dla każdego surowca) = porty
//!   (`PORT_CAPACITY` × poziomy × pokrycie obsady) + fabryki (`FACTORY_CAPACITY` × poziomy);
//! - **oferta** = przepustowość × eksport% (najwyżej cały zapas), **zapotrzebowanie** = przepustowość × import%;
//! - importerzy po kolei (kolejność przesuwa się co dzień) biorą od najtańszej oferty z dostawą
//!   (`Route`: lądem po grafie prowincji, morzem port – port, cło tranzytowe), dopóki jest tańsza od
//!   **handlarzy spoza mapy** (`TRADER_SELL_PCT`% ceny + ich dostawa) i starcza złota; reszta – od handlarzy;
//! - niesprzedaną ofertę kupują handlarze po `TRADER_BUY_PCT`% ceny;
//! - z handlarzami (handel morski) handluje tylko państwo z co najmniej jednym portem;
//! - **cena** idzie za docelową z popytu i podaży (średnie kroczące zamówień) – `Market::update`.
//!
//! Wszystko w liczbach całkowitych i w ustalonej kolejności (determinizm). Ceny w surowym złocie
//! (w interfejsie ÷10) za jednostkę × `PRICE_SCALE`; ilości w tysięcznych (`MILLI`); koszty dostawy
//! w częściach na milion wartości (`PPM`).

use std::{cmp::Reverse, collections::BinaryHeap};

use crate::{
    economy::{Economy, MILLI},
    goods::{BASE_PRICE, GOLD, RESOURCE_COUNT},
    mapgen::ProvinceLink,
};

/// Grupy surowców (jak w legendzie mapy zasobów) – tylko porządek w oknie rynku.
pub const GROUP_COUNT: usize = 6;
pub const GROUP_NAMES: [&str; GROUP_COUNT] = ["Rolnictwo", "Hodowla i połowy", "Las", "Energia", "Metale", "Chemia"];
/// Grupa każdego surowca (kolejność `RESOURCE_NAMES`).
#[rustfmt::skip]
pub const GROUP: [u8; RESOURCE_COUNT] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // zboża … kauczuk
    1, 1, 1, 1, 1, 1, 1,             // bydło … futra
    2,                               // drewno
    3, 3, 3,                         // węgiel kamienny, brunatny, ropa
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4,    // rudy żelaza … złoto (złoto poza rynkiem)
    5, 5, 5, 5, 5,                   // nawozy, cement, saletra, siarka, sól
];
/// Przepustowość handlu na dzień (jednostki każdego surowca): `PORT_CAPACITY` za poziom portu × pokrycie
/// obsady + `FACTORY_CAPACITY` za poziom fabryki.
pub const PORT_CAPACITY: u64 = 5;
pub const FACTORY_CAPACITY: u64 = 1;

/// Cena: surowe złoto za jednostkę × `PRICE_SCALE`.
pub const PRICE_SCALE: u64 = 1000;
/// Cena docelowa = bazowa × √((popyt + L) / (podaż + L)) w granicach `PRICE_MIN_PCT`–`PRICE_MAX_PCT`%
/// bazowej; L – płynność (`LIQUIDITY`, tysięczne jednostki na dzień). Cena bieżąca zbliża się do docelowej
/// o 1/`PRICE_STEP` dziennie, popyt i podaż to średnie kroczące z ok. `AVERAGE_DAYS` dni.
pub const LIQUIDITY: u64 = 20 * MILLI;
pub const PRICE_MIN_PCT: u64 = 25;
pub const PRICE_MAX_PCT: u64 = 400;
pub const PRICE_STEP: i64 = 10;
pub const AVERAGE_DAYS: u64 = 10;
/// Udział w rynku: średnia krocząca wartości eksportu z ok. tylu dni.
pub const SHARE_DAYS: u64 = 30;

/// Części na milion.
pub const PPM: u64 = 1_000_000;
/// Dostawa lądem: odcinek kosztuje `LAND_PPM` × długość / przekątna mapy × współczynnik infrastruktury
/// (`INFRA_PCT[poziom]`) prowincji, przez którą biegnie.
pub const LAND_PPM: u64 = 500_000;
pub const INFRA_PCT: [u64; 6] = [100, 100, 80, 65, 50, 40];
/// Dostawa morzem port – port: `SEA_PPM` × długość / przekątna × średni współczynnik portów
/// (`PORT_PCT[poziom]`), do tego opłata portowa po każdej stronie (`PORT_FEE_PPM[poziom]`).
pub const SEA_PPM: u64 = 250_000;
pub const PORT_PCT: [u64; 4] = [100, 100, 75, 50];
pub const PORT_FEE_PPM: [u64; 4] = [30_000, 30_000, 20_000, 10_000];
/// Cło tranzytowe: za każde obce państwo na trasie lądem (poza eksporterem i importerem) – trafia do
/// jego skarbca.
pub const TRANSIT_PPM: u64 = 50_000;
/// Handlarze spoza mapy (handel morski – tylko z portem): kupują niesprzedaną nadwyżkę po
/// `TRADER_BUY_PCT`% ceny, sprzedają po `TRADER_SELL_PCT`% ceny plus dostawa morzem od krawędzi mapy do portu.
pub const TRADER_BUY_PCT: u64 = 60;
pub const TRADER_SELL_PCT: u64 = 160;

/// Czy surowiec jest na rynku (złoto – nie, trafia od razu do skarbca).
pub fn tradable(resource: usize) -> bool {
    resource != GOLD as usize
}

/// Cena bazowa (surowe złoto × `PRICE_SCALE`).
pub fn base_price(resource: usize) -> u64 {
    BASE_PRICE[resource] as u64 * 10 * PRICE_SCALE
}

/// Wartość `amount` (tysięczne jednostki) po cenie `price` (surowe złoto).
pub fn value(amount: u64, price: u64) -> u64 {
    (amount as u128 * price as u128 / (MILLI * PRICE_SCALE) as u128) as u64
}

/// Ceny i średnie zamówień – wspólne dla wszystkich graczy (w hashu stanu).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Market {
    pub price: [u64; RESOURCE_COUNT],
    /// Cena dzień wcześniej (trend w interfejsie).
    pub prev_price: [u64; RESOURCE_COUNT],
    /// Średnie kroczące zamówień graczy (tysięczne jednostki na dzień): popyt i podaż.
    pub demand: [u64; RESOURCE_COUNT],
    pub supply: [u64; RESOURCE_COUNT],
    /// Dni handlu (przesuwa kolejność importerów).
    pub day: u32,
    /// Obrót wczoraj (surowe złoto): sprzedaż graczy (graczom i handlarzom) i zakupy u handlarzy.
    pub turnover: [u64; RESOURCE_COUNT],
}

impl Default for Market {
    fn default() -> Self {
        let price = std::array::from_fn(base_price);
        Market {
            price,
            prev_price: price,
            demand: [0; RESOURCE_COUNT],
            supply: [0; RESOURCE_COUNT],
            day: 0,
            turnover: [0; RESOURCE_COUNT],
        }
    }
}

impl Market {
    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        let mut b = Vec::with_capacity(4 + 32 * RESOURCE_COUNT);
        for a in [&self.price, &self.prev_price, &self.demand, &self.supply, &self.turnover] {
            b.extend(a.iter().flat_map(|v| v.to_le_bytes()));
        }
        b.extend(self.day.to_le_bytes());
        b.into_iter()
    }

    /// Koniec dnia handlu: średnie popytu i podaży z dzisiejszych zamówień, cena o krok bliżej docelowej.
    pub fn update(&mut self, demand: &[u64; RESOURCE_COUNT], supply: &[u64; RESOURCE_COUNT]) {
        self.prev_price = self.price;
        for r in (0..RESOURCE_COUNT).filter(|&r| tradable(r)) {
            self.demand[r] = ema(self.demand[r], demand[r], AVERAGE_DAYS);
            self.supply[r] = ema(self.supply[r], supply[r], AVERAGE_DAYS);
            let target = target_price(r, self.demand[r], self.supply[r]);
            let step = (target as i64 - self.price[r] as i64) / PRICE_STEP;
            self.price[r] = (self.price[r] as i64 + step) as u64;
        }
        self.day = self.day.wrapping_add(1);
    }
}

/// Średnia krocząca (wykładnicza, ok. `days` dni).
pub fn ema(avg: u64, today: u64, days: u64) -> u64 {
    (avg * (days - 1) + today) / days
}

/// Cena docelowa surowca przy średnim popycie i podaży.
pub fn target_price(resource: usize, demand: u64, supply: u64) -> u64 {
    let base = base_price(resource);
    let ratio = (demand + LIQUIDITY) as u128 * (PPM * PPM) as u128 / (supply + LIQUIDITY) as u128;
    let sqrt = ratio.isqrt() as u64; // √(popyt / podaż) × PPM
    (base as u128 * sqrt as u128 / PPM as u128)
        .clamp((base * PRICE_MIN_PCT / 100) as u128, (base * PRICE_MAX_PCT / 100) as u128) as u64
}

/// Handel jednego państwa: ustawienia suwaków, udział w rynku i wczorajsze transakcje.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trade {
    /// Bilans surowców (−100 import … +100 eksport, % przepustowości handlu).
    pub settings: [i8; RESOURCE_COUNT],
    /// Średnia krocząca wartości eksportu (surowe złoto na dzień) – udział w rynku – i eksport dzisiaj.
    pub exports: u64,
    pub exports_today: u64,
    /// Średnia krocząca wartości eksportu każdego surowca – udział w rynku surowca.
    pub resource_exports: [u64; RESOURCE_COUNT],
    /// Wczorajsze transakcje (raport; stan gry zmieniły już zapasy i złoto).
    pub deals: Vec<Deal>,
}

impl Default for Trade {
    fn default() -> Self {
        Trade {
            settings: [0; RESOURCE_COUNT],
            exports: 0,
            exports_today: 0,
            resource_exports: [0; RESOURCE_COUNT],
            deals: Vec::new(),
        }
    }
}

impl Trade {
    /// Ustawienie bilansu surowca (%).
    pub fn setting(&self, resource: usize) -> i8 {
        self.settings[resource]
    }

    pub fn hash_bytes(&self) -> impl Iterator<Item = u8> {
        let mut b: Vec<u8> = self.settings.iter().map(|&v| v as u8).collect();
        b.extend(self.exports.to_le_bytes());
        b.extend(self.exports_today.to_le_bytes());
        b.extend(self.resource_exports.iter().flat_map(|v| v.to_le_bytes()));
        b.into_iter()
    }
}

/// Jedna transakcja (z punktu widzenia państwa, które ją zapisało).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deal {
    pub resource: u8,
    /// Kupno (import) albo sprzedaż (eksport).
    pub buy: bool,
    /// Drugie państwo (0 – handlarze spoza mapy).
    pub partner: u8,
    /// Ilość (tysięczne jednostki), wartość po cenie rynkowej i koszt dostawy z cłem (surowe złoto).
    pub amount: u64,
    pub value: u64,
    pub delivery: u64,
    /// Państwa, którym importer zapłacił cło tranzytowe.
    pub transit: Vec<u8>,
}

/// Trasa dostawy od eksportera do importera: koszt w częściach na milion wartości (z cłem) i państwa,
/// które dostają cło tranzytowe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub cost_ppm: u64,
    pub transit: Vec<u8>,
}

/// Zamówienie jednego państwa na jeden surowiec (tysięczne jednostki).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Order {
    pub nation: u8,
    pub amount: u64,
}

/// Zamówienia na jeden surowiec w jednym dniu: oferty eksporterów, zapotrzebowanie importerów i od którego
/// importera zaczyna się kolejka (przesuwa się co dzień).
#[derive(Clone, Copy, Debug)]
pub struct Book<'a> {
    pub resource: u8,
    pub price: u64,
    pub offers: &'a [Order],
    pub demands: &'a [Order],
    pub start: usize,
}

/// Dopasowanie jednego surowca (`book.resource`) po cenie `book.price`: importerzy (`demands`, w kolejności
/// od `start`) biorą od najtańszej oferty z dostawą (`route(eksporter, importer)`; remis – większy udział
/// w rynku, potem niższy numer państwa), póki jest tańsza od handlarzy (`trader_ppm[importer]` – ich
/// dostawa; `None` – państwo bez portu, bez handlu z handlarzami) i starcza złota; reszta od handlarzy.
/// Niesprzedane oferty państw z portem kupują handlarze. Zmienia zapasy,
/// złoto, księgi i zapisuje transakcje w `economy[n - 1].trade.deals`.
pub fn match_resource(
    book: Book,
    trader_ppm: &[Option<u64>],
    route: &dyn Fn(u8, u8) -> Option<Route>,
    economy: &mut [Economy],
) {
    let Book { resource, price, offers, demands, start } = book;
    let r = resource as usize;
    let mut left: Vec<u64> = offers.iter().map(|o| o.amount).collect();
    for k in 0..demands.len() {
        let d = demands[(start + k) % demands.len()];
        let i = d.nation as usize - 1;
        let trader_unit =
            trader_ppm[i].map(|t| price as u128 * TRADER_SELL_PCT as u128 * (PPM + t) as u128 / (100 * PPM) as u128);
        // Oferty innych państw z trasą, od najtańszej z dostawą.
        let mut options: Vec<(u64, Reverse<u64>, u8, usize, Route)> = offers
            .iter()
            .enumerate()
            .filter(|&(j, o)| o.nation != d.nation && left[j] > 0)
            .filter_map(|(j, o)| {
                let route = route(o.nation, d.nation)?;
                Some((route.cost_ppm, Reverse(economy[o.nation as usize - 1].trade.exports), o.nation, j, route))
            })
            .collect();
        options.sort_by_key(|o| (o.0, o.1, o.2));
        let mut want = d.amount;
        for (cost, _, exporter, j, route) in options {
            if want == 0 {
                break;
            }
            let unit = price as u128 * (PPM + cost) as u128 / PPM as u128;
            if trader_unit.is_some_and(|t| unit >= t) {
                break;
            }
            let amount = want.min(left[j]).min(affordable(economy[i].gold, unit));
            if amount == 0 {
                break;
            }
            let v = value(amount, price);
            let delivery = (v as u128 * cost as u128 / PPM as u128) as u64;
            let duty = v * TRANSIT_PPM / PPM;
            let e = exporter as usize - 1;
            economy[i].gold -= (v + delivery).min(economy[i].gold);
            economy[i].stock[r] += amount;
            economy[i].month.purchases += v + delivery;
            economy[e].gold += v;
            economy[e].stock[r] -= amount.min(economy[e].stock[r]);
            economy[e].month.sales += v;
            economy[e].trade.exports_today += v;
            for &t in &route.transit {
                economy[t as usize - 1].gold += duty;
                economy[t as usize - 1].month.duties += duty;
            }
            economy[i].trade.deals.push(Deal {
                resource,
                buy: true,
                partner: exporter,
                amount,
                value: v,
                delivery,
                transit: route.transit.clone(),
            });
            economy[e].trade.deals.push(Deal {
                resource,
                buy: false,
                partner: d.nation,
                amount,
                value: v,
                delivery: 0,
                transit: Vec::new(),
            });
            left[j] -= amount;
            want -= amount;
        }
        // Reszta od handlarzy spoza mapy (tylko z portem).
        let (Some(trader_unit), Some(trader)) = (trader_unit, trader_ppm[i]) else { continue };
        let amount = want.min(affordable(economy[i].gold, trader_unit));
        if amount > 0 {
            let v = value(amount, price) * TRADER_SELL_PCT / 100;
            let delivery = (v as u128 * trader as u128 / PPM as u128) as u64;
            economy[i].gold -= (v + delivery).min(economy[i].gold);
            economy[i].stock[r] += amount;
            economy[i].month.purchases += v + delivery;
            economy[i].trade.deals.push(Deal {
                resource,
                buy: true,
                partner: 0,
                amount,
                value: v,
                delivery,
                transit: Vec::new(),
            });
        }
    }
    // Niesprzedane oferty – handlarzom (tylko państwa z portem).
    for (j, o) in offers.iter().enumerate().filter(|&(j, _)| left[j] > 0) {
        let e = o.nation as usize - 1;
        if trader_ppm[e].is_none() {
            continue;
        }
        let amount = left[j].min(economy[e].stock[r]);
        let v = value(amount, price) * TRADER_BUY_PCT / 100;
        if v == 0 {
            continue;
        }
        economy[e].stock[r] -= amount;
        economy[e].gold += v;
        economy[e].month.sales += v;
        economy[e].trade.exports_today += v;
        economy[e].trade.deals.push(Deal {
            resource,
            buy: false,
            partner: 0,
            amount,
            value: v,
            delivery: 0,
            transit: Vec::new(),
        });
    }
}

/// Ile tysięcznych jednostki kupi `gold` przy cenie z dostawą `unit` (surowe złoto × `PRICE_SCALE`).
fn affordable(gold: u64, unit: u128) -> u64 {
    if unit == 0 {
        return u64::MAX;
    }
    (gold as u128 * (MILLI * PRICE_SCALE) as u128 / unit).min(u64::MAX as u128) as u64
}

/// Sieć dróg do liczenia tras: graf prowincji, środki prowincji (kafle), właściciel prowincji (0 – brak),
/// poziom infrastruktury i przekątna mapy.
pub struct Network<'a> {
    pub graph: &'a [Vec<ProvinceLink>],
    pub centers: Vec<(i64, i64)>,
    pub owner: Vec<u8>,
    pub infra: Vec<u8>,
    pub diagonal: u64,
}

impl Network<'_> {
    /// Koszt odcinka lądem między sąsiednimi prowincjami (pół w każdej, według jej infrastruktury).
    fn segment(&self, p: usize, q: usize) -> u64 {
        let (a, b) = (self.centers[p], self.centers[q]);
        let len = (((a.0 - b.0).pow(2) + (a.1 - b.1).pow(2)) as u64 * PPM).isqrt(); // kafle × 1000
        let pct = INFRA_PCT[self.infra[p].min(5) as usize] + INFRA_PCT[self.infra[q].min(5) as usize];
        (LAND_PPM as u128 * len as u128 * pct as u128 / (200 * 1000 * self.diagonal.max(1)) as u128) as u64
    }

    /// Najtańsze trasy lądem od zbioru prowincji `roots` (np. fabryk importera) do wszystkich prowincji
    /// (Dijkstra). Wybór trasy omija obce państwa (krok do obcego państwa – jak cło), koszt w `land` – bez
    /// ceł, które dolicza `Network::route`.
    pub fn tree(&self, roots: &[u16], nation: u8) -> Tree {
        let n = self.graph.len();
        let mut tree = Tree { land: vec![u64::MAX; n], root: vec![0; n], nations: vec![Vec::new(); n] };
        let mut best = vec![u64::MAX; n];
        let mut heap = BinaryHeap::new();
        for &p in roots {
            let k = p as usize - 1;
            if best[k] != 0 {
                best[k] = 0;
                tree.land[k] = 0;
                tree.root[k] = p;
                tree.nations[k] = owner_set(&[], self.owner[k]);
                heap.push(Reverse((0u64, k)));
            }
        }
        while let Some(Reverse((d, p))) = heap.pop() {
            if d > best[p] {
                continue;
            }
            for link in &self.graph[p] {
                let q = link.province as usize - 1;
                let (o, prev) = (self.owner[q], self.owner[p]);
                let toll = if o != 0 && o != nation && o != prev { TRANSIT_PPM } else { 0 };
                let step = self.segment(p, q);
                let next = d + step + toll;
                if next < best[q] {
                    best[q] = next;
                    tree.land[q] = tree.land[p] + step;
                    tree.root[q] = tree.root[p];
                    tree.nations[q] = owner_set(&tree.nations[p], o);
                    heap.push(Reverse((next, q)));
                }
            }
        }
        tree
    }
}

/// Zbiór państw `set` z dopisanym `owner` (rosnąco, bez zera i powtórzeń).
fn owner_set(set: &[u8], owner: u8) -> Vec<u8> {
    let mut out = set.to_vec();
    if owner != 0
        && let Err(k) = out.binary_search(&owner)
    {
        out.insert(k, owner);
    }
    out
}

/// Najtańsze trasy od zbioru prowincji: koszt lądem bez ceł (`u64::MAX` – brak trasy), prowincja-korzeń
/// (0 – brak) i państwa na trasie (rosnąco).
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub land: Vec<u64>,
    pub root: Vec<u16>,
    pub nations: Vec<Vec<u8>>,
}

impl Tree {
    /// Koszt lądem z prowincji `province` do korzenia z cłem tranzytowym (bez eksportera i importera)
    /// i państwa, które je dostają; `None` – brak trasy.
    pub fn leg(&self, province: u16, exporter: u8, importer: u8) -> Option<(u64, Vec<u8>)> {
        let k = province as usize - 1;
        if self.land[k] == u64::MAX {
            return None;
        }
        let transit: Vec<u8> = self.nations[k].iter().copied().filter(|&n| n != exporter && n != importer).collect();
        Some((self.land[k] + TRANSIT_PPM * transit.len() as u64, transit))
    }
}

/// Port do handlu morskiego: prowincja, kafel (miejsce portu), zbiornik morza i poziom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Port {
    pub province: u16,
    pub x: i64,
    pub y: i64,
    pub body: u32,
    pub level: u8,
}

/// Koszt rejsu port – port (z opłatami portowymi); `None` – różne morza.
pub fn sea_leg(from: &Port, to: &Port, diagonal: u64) -> Option<u64> {
    if from.body != to.body {
        return None;
    }
    let len = (((from.x - to.x).pow(2) + (from.y - to.y).pow(2)) as u64 * PPM).isqrt();
    let pct = PORT_PCT[from.level.min(3) as usize] + PORT_PCT[to.level.min(3) as usize];
    let sail = (SEA_PPM as u128 * len as u128 * pct as u128 / (200 * 1000 * diagonal.max(1)) as u128) as u64;
    Some(sail + PORT_FEE_PPM[from.level.min(3) as usize] + PORT_FEE_PPM[to.level.min(3) as usize])
}

/// Dostawa handlarzy spoza mapy do portu: rejs od najbliższej krawędzi mapy i opłata portowa.
pub fn trader_sea_leg(port: &Port, width: i64, height: i64, diagonal: u64) -> u64 {
    let edge = port.x.min(port.y).min(width - 1 - port.x).min(height - 1 - port.y).max(0) as u64;
    let pct = PORT_PCT[port.level.min(3) as usize];
    (SEA_PPM as u128 * edge as u128 * 1000 * pct as u128 / (100 * 1000 * diagonal.max(1)) as u128) as u64
        + PORT_FEE_PPM[port.level.min(3) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goods::id::SALT;

    #[test]
    fn land_route_costs_distance_infrastructure_and_transit() {
        let link = |p: u16| ProvinceLink { province: p, river: false };
        // Prowincje 1–2–3–4 w linii co 10 kafli; 2 i 3 – państwo 3 (tranzyt), 4 – importer 2.
        let graph = vec![vec![link(2)], vec![link(1), link(3)], vec![link(2), link(4)], vec![link(3)]];
        let network = Network {
            graph: &graph,
            centers: vec![(0, 0), (10, 0), (20, 0), (30, 0)],
            owner: vec![1, 3, 3, 2],
            infra: vec![1, 1, 1, 1],
            diagonal: 100,
        };
        let tree = network.tree(&[4], 2);
        assert_eq!(tree.land, vec![150_000, 100_000, 50_000, 0], "50% × 10 / 100 na odcinek");
        assert_eq!(tree.root, vec![4; 4]);
        assert_eq!(tree.leg(1, 1, 2), Some((200_000, vec![3])), "cło jednego państwa");
        // Lepsza infrastruktura – taniej.
        let network = Network { infra: vec![5, 5, 5, 5], ..network };
        assert_eq!(network.tree(&[4], 2).land[0], 150_000 * INFRA_PCT[5] / 100);
        // Morze: ten sam zbiornik, opłaty portowe po obu stronach.
        let port = |x: i64, body: u32| Port { province: 1, x, y: 0, body, level: 1 };
        assert_eq!(sea_leg(&port(0, 1), &port(40, 1), 100), Some(100_000 + 2 * PORT_FEE_PPM[1]));
        assert_eq!(sea_leg(&port(0, 1), &port(40, 2), 100), None, "różne morza");
    }

    #[test]
    fn price_follows_demand_and_supply_within_bounds() {
        let r = SALT as usize;
        let base = base_price(r);
        assert_eq!(target_price(r, 5 * MILLI, 5 * MILLI), base);
        assert!(target_price(r, 40 * MILLI, 10 * MILLI) > base);
        assert!(target_price(r, 0, 40 * MILLI) < base);
        assert_eq!(target_price(r, 1_000_000 * MILLI, 0), base * PRICE_MAX_PCT / 100);
        assert_eq!(target_price(r, 0, 1_000_000 * MILLI), base * PRICE_MIN_PCT / 100);
        // Krok dziennie: 1/10 drogi do docelowej.
        let mut m = Market::default();
        let mut demand = [0; RESOURCE_COUNT];
        demand[r] = 1_000_000 * MILLI;
        m.update(&demand, &[0; RESOURCE_COUNT]);
        let target = target_price(r, m.demand[r], 0);
        assert_eq!(m.price[r], base + (target - base) / 10);
        assert_eq!(m.prev_price[r], base);
        assert_eq!(m.price[GOLD as usize], base_price(GOLD as usize), "złoto poza rynkiem");
    }

    fn rich(gold: u64, salt: u64) -> Economy {
        let mut e = Economy { gold, ..Default::default() };
        e.stock[SALT as usize] = salt * MILLI;
        e
    }

    #[test]
    fn importer_buys_cheapest_delivery_pays_transit_and_traders_take_the_rest() {
        let r = SALT as usize;
        let price = base_price(r);
        // Państwa: 1 – eksporter, 2 – importer, 3 – tranzyt.
        let mut economy = vec![rich(0, 10), rich(1_000_000, 0), rich(0, 0)];
        let route = |e: u8, i: u8| (e == 1 && i == 2).then(|| Route { cost_ppm: 100_000, transit: vec![3] });
        let offers = [Order { nation: 1, amount: 5 * MILLI }];
        let demands = [Order { nation: 2, amount: 3 * MILLI }];
        let book = Book { resource: SALT, price, offers: &offers, demands: &demands, start: 0 };
        match_resource(book, &[Some(0), Some(0), Some(0)], &route, &mut economy);
        let v = value(3 * MILLI, price);
        assert_eq!(economy[1].stock[r], 3 * MILLI);
        assert_eq!(economy[1].gold, 1_000_000 - v - v / 10, "cena + 10% dostawy");
        assert_eq!(economy[2].gold, v * TRANSIT_PPM / PPM, "cło tranzytowe");
        // Reszta oferty (2) – handlarzom po 60% ceny.
        let rest = value(2 * MILLI, price) * TRADER_BUY_PCT / 100;
        assert_eq!(economy[0].gold, v + rest);
        assert_eq!(economy[0].stock[r], 5 * MILLI);
        assert_eq!(economy[0].month.sales, v + rest);
        assert_eq!(economy[1].trade.deals.len(), 1);
        assert_eq!(economy[0].trade.deals.iter().map(|d| d.partner).collect::<Vec<_>>(), vec![2, 0]);
    }

    #[test]
    fn traders_sell_when_players_are_dearer_or_gold_runs_out() {
        let r = SALT as usize;
        let price = base_price(r);
        // Dostawa od gracza droższa niż handlarze (160% ceny) – kupno u handlarzy, ile starczy złota.
        let mut economy = vec![rich(0, 10), rich(0, 0)];
        let unit = price * TRADER_SELL_PCT / 100 / PRICE_SCALE; // surowe złoto za jednostkę
        economy[1].gold = unit * 2;
        let route = |_: u8, _: u8| Some(Route { cost_ppm: 700_000, transit: Vec::new() });
        let offers = [Order { nation: 1, amount: 5 * MILLI }];
        let demands = [Order { nation: 2, amount: 5 * MILLI }];
        let book = Book { resource: SALT, price, offers: &offers, demands: &demands, start: 0 };
        match_resource(book, &[Some(0), Some(0)], &route, &mut economy);
        assert_eq!(economy[1].stock[r], 2 * MILLI, "na tyle starczyło złota");
        assert_eq!(economy[1].gold, 0);
        assert_eq!(economy[1].trade.deals[0].partner, 0, "od handlarzy");
        assert_eq!(economy[0].trade.deals[0].partner, 0, "eksporter sprzedał handlarzom");
    }

    #[test]
    fn groups_follow_the_resource_legend() {
        use crate::goods::{RESOURCE_NAMES, id};
        assert_eq!(GROUP[id::RUBBER as usize], 0);
        for (r, name) in RESOURCE_NAMES.iter().enumerate() {
            let g = GROUP[r] as usize;
            assert!(g < GROUP_COUNT, "{name}");
        }
        assert_eq!(GROUP[id::WOOD as usize], 2);
        assert_eq!(GROUP[id::OIL as usize], 3);
        assert_eq!(GROUP[id::TIN as usize], 4);
        assert_eq!(GROUP[id::SALT as usize], 5);
        assert_eq!(GROUP[id::FURS as usize], 1);
    }
}
