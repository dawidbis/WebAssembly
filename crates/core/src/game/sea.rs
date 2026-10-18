//! Trasy morskie desantów: odległość po kaflach morza od portu i najkrótsza droga. Ruch na 8 kierunków
//! (prosto `STEP`, na skos `DIAG` – ok. √2 razy dłużej), na skos tylko między dwoma kaflami tego samego
//! zbiornika (bez przecinania cypli). Odległości liczy Dijkstra z kubełkami (koszty to małe liczby
//! całkowite) – jeden przebieg na port daje drogę do każdego brzegu. Pola są wyliczane z niezmiennej mapy,
//! więc trzymane poza hashem: kilka ostatnio używanych portów (`CACHE`).

use std::{cell::RefCell, rc::Rc};

use super::{Game, Water};

/// Koszt kroku prosto i na skos (`STEP` = jeden kafel drogi).
pub const STEP: u32 = 5;
pub const DIAG: u32 = 7;
/// Ile pól odległości (portów) pamiętać.
const CACHE: usize = 8;
/// Droga rejsu na mapie: co tyle kafli punkt (plus ostatni).
const PATH_EVERY: usize = 3;

/// Odległość od portu (kafel lądu `origin`) do każdego kafla jego morza (`body`), w jednostkach `STEP`
/// (`u32::MAX` – nieosiągalny); indeks – numer kafla w zbiorniku (`Water::local`).
pub(super) struct SeaField {
    pub origin: u32,
    pub body: u32,
    pub dist: Vec<u32>,
}

/// Ostatnio używane pola odległości (najnowsze na końcu).
#[derive(Default)]
pub(super) struct SeaCache(RefCell<Vec<Rc<SeaField>>>);

const DIRS: [(i64, i64, u32); 8] = [
    (1, 0, STEP),
    (-1, 0, STEP),
    (0, 1, STEP),
    (0, -1, STEP),
    (1, 1, DIAG),
    (-1, 1, DIAG),
    (1, -1, DIAG),
    (-1, -1, DIAG),
];

impl Water {
    /// Sąsiad kafla `i` w kierunku (dx, dy) w tym samym zbiorniku (na skos – oba boki też w zbiorniku).
    fn sea_step(&self, i: usize, dx: i64, dy: i64, w: usize, h: usize) -> Option<usize> {
        let b = self.body[i];
        let (x, y) = ((i % w) as i64 + dx, (i / w) as i64 + dy);
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            return None;
        }
        let j = y as usize * w + x as usize;
        let side = |k: usize| self.body[k] == b;
        (side(j) && (dx == 0 || dy == 0 || (side(y as usize * w + i % w) && side((i / w) * w + x as usize))))
            .then_some(j)
    }

    /// Pole odległości od portu na kaflu lądu `origin` (start z kafli morza stykających się z nim bokiem).
    fn sea_field(&self, origin: usize, w: usize, h: usize) -> Option<SeaField> {
        let body = self.sea_next_to(origin, w, h)?;
        let tiles = &self.tiles[body as usize - 1];
        let mut dist = vec![u32::MAX; tiles.len()];
        // Kubełki według odległości modulo (największy krok + 1).
        let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); DIAG as usize + 1];
        let slots = buckets.len() as u32;
        let (x, y) = (origin % w, origin / w);
        let mut queued = 0usize;
        for (ok, j) in [
            (x > 0, origin.wrapping_sub(1)),
            (x + 1 < w, origin + 1),
            (y > 0, origin.wrapping_sub(w)),
            (y + 1 < h, origin + w),
        ] {
            if ok && self.body[j] == body && dist[self.local[j] as usize] != 0 {
                dist[self.local[j] as usize] = 0;
                buckets[0].push(j as u32);
                queued += 1;
            }
        }
        let mut d = 0u32;
        while queued > 0 {
            let slot = (d % slots) as usize;
            while let Some(i) = buckets[slot].pop() {
                queued -= 1;
                let i = i as usize;
                if dist[self.local[i] as usize] != d {
                    continue;
                }
                for &(dx, dy, cost) in &DIRS {
                    let Some(j) = self.sea_step(i, dx, dy, w, h) else { continue };
                    let k = self.local[j] as usize;
                    if d + cost < dist[k] {
                        dist[k] = d + cost;
                        buckets[((d + cost) % slots) as usize].push(j as u32);
                        queued += 1;
                    }
                }
            }
            d += 1;
        }
        Some(SeaField { origin: origin as u32, body, dist })
    }

    /// Droga od portu do kafla morza `end` (kafle morza po kolei, co `PATH_EVERY` plus ostatni) – z pola
    /// odległości, krok zawsze do sąsiada, z którego przyszła najkrótsza droga.
    fn sea_path(&self, field: &SeaField, end: usize, w: usize, h: usize) -> Vec<u32> {
        let mut back = vec![end as u32];
        let mut i = end;
        let mut d = field.dist[self.local[end] as usize];
        while d > 0 && d != u32::MAX {
            let Some((j, next)) = DIRS.iter().find_map(|&(dx, dy, cost)| {
                let j = self.sea_step(i, dx, dy, w, h)?;
                let dj = field.dist[self.local[j] as usize];
                (dj != u32::MAX && dj + cost == d).then_some((j, dj))
            }) else {
                break;
            };
            back.push(j as u32);
            (i, d) = (j, next);
        }
        back.reverse();
        let last = back.len() - 1;
        back.into_iter().enumerate().filter(|&(k, _)| k % PATH_EVERY == 0 || k == last).map(|(_, t)| t).collect()
    }
}

impl Game {
    /// Pole odległości po morzu od portu na kaflu `origin` (z pamięci albo policzone i zapamiętane).
    pub(super) fn sea_field(&self, origin: usize) -> Option<Rc<SeaField>> {
        let mut cache = self.sea.0.borrow_mut();
        if let Some(k) = cache.iter().position(|f| f.origin as usize == origin) {
            let f = cache.remove(k);
            cache.push(f.clone());
            return Some(f);
        }
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        let field = Rc::new(self.water.sea_field(origin, w, h)?);
        if cache.len() >= CACHE {
            cache.remove(0);
        }
        cache.push(field.clone());
        Some(field)
    }

    /// Odległość (kafle drogi, `None` – nieosiągalny) od portu z pola `field` do kafla morza `sea`.
    pub(super) fn sea_distance(&self, field: &SeaField, sea: usize) -> Option<u32> {
        (self.water.body[sea] == field.body)
            .then(|| field.dist[self.water.local[sea] as usize])
            .filter(|&d| d != u32::MAX)
            .map(|d| d.div_ceil(STEP) + 1)
    }

    /// Droga rejsu od portu `origin` do kafla morza `sea` (kafle co kilka).
    pub(super) fn sea_route(&self, origin: usize, sea: usize) -> Vec<u32> {
        let (w, h) = (self.map.width as usize, self.map.height as usize);
        self.sea_field(origin).map_or_else(Vec::new, |f| self.water.sea_path(&f, sea, w, h))
    }

    /// Kafle morza w zasięgu `range` (kafle drogi) od portu na kaflu `origin` – zasięg desantu na mapie.
    pub fn sea_reach(&self, origin: usize, range: u32) -> Vec<u32> {
        let Some(f) = self.sea_field(origin) else { return Vec::new() };
        let limit = (range.saturating_sub(1)).saturating_mul(STEP);
        self.water.tiles[f.body as usize - 1]
            .iter()
            .zip(&f.dist)
            .filter(|&(_, &d)| d != u32::MAX && d <= limit)
            .map(|(&t, _)| t)
            .collect()
    }
}
