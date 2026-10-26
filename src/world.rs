use crate::config::Tuning;

pub const MAP_W: usize = 216;
pub const MAP_H: usize = 116;
pub const COLONIES: usize = 5;

#[derive(Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

pub struct Zone {
    pub owner: Option<usize>,
    pub food: f64,
    pub initial_food: f64,
    pub center: (f32, f32),
    pub neighbors: Vec<usize>,
    pub tiles: usize,
}

pub struct World {
    pub tiles: Vec<Option<usize>>,
    pub zones: Vec<Zone>,
    pub revision: u64,
}

fn land(x: f32, y: f32) -> bool {
    // An original, stylised world silhouette. No OpenFront code or assets.
    let continents = [
        (0.20, 0.26, 0.18, 0.17),
        (0.14, 0.18, 0.10, 0.12),
        (0.28, 0.43, 0.055, 0.13),
        (0.32, 0.66, 0.085, 0.22),
        (0.40, 0.13, 0.065, 0.09),
        (0.54, 0.29, 0.10, 0.11),
        (0.57, 0.53, 0.095, 0.22),
        (0.73, 0.28, 0.22, 0.17),
        (0.74, 0.46, 0.07, 0.14),
        (0.83, 0.51, 0.07, 0.11),
        (0.86, 0.76, 0.095, 0.10),
        (0.94, 0.40, 0.023, 0.10),
    ];
    let coast = 0.10 * (x * 89.0 + y * 41.0).sin() + 0.06 * (y * 137.0 - x * 73.0).cos();
    continents
        .iter()
        .any(|&(cx, cy, rx, ry)| ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) < 1.0 + coast)
}

impl World {
    pub fn new(seed: u64, tuning: &Tuning) -> Self {
        let mut rng = Rng(seed);
        let mut seeds = Vec::new();
        for row in 0..9 {
            for col in 0..17 {
                let x = (col as f32 + 0.25 + rng.unit() as f32 * 0.5) / 17.0;
                let y = (row as f32 + 0.25 + rng.unit() as f32 * 0.5) / 9.0;
                if land(x, y) {
                    seeds.push((x * MAP_W as f32, y * MAP_H as f32));
                }
            }
        }
        let mut tiles = vec![None; MAP_W * MAP_H];
        // Flood from zone seeds: each zone is connected, including near coasts.
        let mut queue = std::collections::VecDeque::new();
        for (id, &(x, y)) in seeds.iter().enumerate() {
            let tile = y as usize * MAP_W + x as usize;
            tiles[tile] = Some(id);
            queue.push_back(tile);
        }
        while let Some(i) = queue.pop_front() {
            let x = i % MAP_W;
            let y = i / MAP_W;
            for n in adjacent(x, y) {
                if tiles[n].is_none()
                    && land(
                        (n % MAP_W) as f32 / MAP_W as f32,
                        (n / MAP_W) as f32 / MAP_H as f32,
                    )
                {
                    tiles[n] = tiles[i];
                    queue.push_back(n);
                }
            }
        }
        let mut zones: Vec<Zone> = seeds
            .iter()
            .map(|&center| {
                let food = tuning.zone_food_min
                    + rng.unit() * (tuning.zone_food_max - tuning.zone_food_min);
                Zone {
                    owner: None,
                    food,
                    initial_food: food,
                    center,
                    neighbors: Vec::new(),
                    tiles: 0,
                }
            })
            .collect();
        for (i, tile) in tiles.iter().enumerate() {
            if let Some(id) = *tile {
                zones[id].tiles += 1;
                for n in adjacent(i % MAP_W, i / MAP_W) {
                    if let Some(other) = tiles[n] {
                        if id != other && !zones[id].neighbors.contains(&other) {
                            zones[id].neighbors.push(other);
                        }
                    }
                }
            }
        }
        Self {
            tiles,
            zones,
            revision: 0,
        }
    }

    pub fn zone_at(&self, x: f32, y: f32) -> Option<usize> {
        if x < 0.0 || y < 0.0 || x >= MAP_W as f32 || y >= MAP_H as f32 {
            return None;
        }
        self.tiles[y as usize * MAP_W + x as usize]
    }

    pub fn can_expand(&self, zone: usize, owner: usize) -> bool {
        self.zones.get(zone).is_some_and(|z| {
            z.owner != Some(owner)
                && z.neighbors
                    .iter()
                    .any(|&n| self.zones[n].owner == Some(owner))
        })
    }

    pub fn claim(&mut self, zone: usize, owner: usize) {
        self.zones[zone].owner = Some(owner);
        self.revision += 1;
    }

    pub fn owned_count(&self, owner: usize) -> usize {
        self.zones.iter().filter(|z| z.owner == Some(owner)).count()
    }

    pub fn remaining_food(&self, owner: usize) -> f64 {
        self.zones
            .iter()
            .filter(|z| z.owner == Some(owner))
            .map(|z| z.food)
            .sum()
    }

    pub fn seed_rivals(&mut self, player_zone: usize) {
        let mut chosen = vec![player_zone];
        // One nearby rival makes the combat test accessible; others are spread out.
        for colony in 1..COLONIES {
            let candidate = if colony == 1 {
                self.zones[player_zone].neighbors.first().copied()
            } else {
                (0..self.zones.len())
                    .filter(|i| !chosen.contains(i) && !self.zones[*i].neighbors.is_empty())
                    .max_by(|&a, &b| {
                        let distance = |id: usize| {
                            chosen
                                .iter()
                                .map(|&c| {
                                    let p = self.zones[id].center;
                                    let q = self.zones[c].center;
                                    (p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)
                                })
                                .fold(f32::INFINITY, f32::min)
                        };
                        distance(a).total_cmp(&distance(b))
                    })
            };
            if let Some(id) = candidate {
                if !chosen.contains(&id) {
                    self.claim(id, colony);
                    chosen.push(id);
                }
            }
        }
    }
}

fn adjacent(x: usize, y: usize) -> impl Iterator<Item = usize> {
    [
        (x > 0).then(|| y * MAP_W + x - 1),
        (x + 1 < MAP_W).then(|| y * MAP_W + x + 1),
        (y > 0).then(|| (y - 1) * MAP_W + x),
        (y + 1 < MAP_H).then(|| (y + 1) * MAP_W + x),
    ]
    .into_iter()
    .flatten()
}
