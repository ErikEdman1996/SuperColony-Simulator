use crate::{config::Tuning, world::Rng};

pub const ARENA_W: f32 = 1040.0;
pub const ARENA_H: f32 = 590.0;
const REACH: f32 = 8.0;
// A cell matches melee reach: a 3x3 query covers every possible hit while
// avoiding unnecessarily large buckets in dense battles.
const CELL: f32 = REACH;
const GRID_W: usize = (ARENA_W / CELL) as usize + 1;
const GRID_H: usize = (ARENA_H / CELL) as usize + 1;

#[derive(Clone)]
pub struct Ant {
    pub x: f32,
    pub y: f32,
    pub previous_x: f32,
    pub previous_y: f32,
    pub angle: f32,
    pub health: f32,
    pub team: usize,
    cooldown: f32,
}

pub struct Battle {
    pub zone: usize,
    pub defender: usize,
    pub ants: Vec<Ant>,
    pub initial: [usize; 2],
    pub alive: [usize; 2],
    pub starvation: [usize; 2],
    pub elapsed: f32,
    pub winner: Option<usize>,
    pub settled: bool,
    grid: Vec<Vec<usize>>,
    damage: Vec<f32>,
    movement: Vec<(f32, f32)>,
    fired: Vec<bool>,
}

impl Battle {
    pub fn new(zone: usize, defender: usize, armies: [usize; 2], seed: u64, t: &Tuning) -> Self {
        let mut rng = Rng(seed);
        let mut ants = Vec::with_capacity(armies.iter().sum());
        for (team, &count) in armies.iter().enumerate() {
            for _ in 0..count {
                let depth = rng.unit() as f32 * 200.0;
                let x = if team == 0 {
                    70.0 + depth
                } else {
                    ARENA_W - 70.0 - depth
                };
                let y = 55.0 + rng.unit() as f32 * (ARENA_H - 110.0);
                ants.push(Ant {
                    x,
                    y,
                    previous_x: x,
                    previous_y: y,
                    angle: if team == 0 { 0.0 } else { std::f32::consts::PI },
                    health: t.ant_health,
                    team,
                    cooldown: rng.unit() as f32 * t.attack_interval,
                });
            }
        }
        let len = ants.len();
        Self {
            zone,
            defender,
            ants,
            initial: armies,
            alive: armies,
            starvation: [0, 0],
            elapsed: 0.0,
            winner: None,
            settled: false,
            grid: (0..GRID_W * GRID_H).map(|_| Vec::new()).collect(),
            damage: vec![0.0; len],
            movement: vec![(0.0, 0.0); len],
            fired: vec![false; len],
        }
    }

    /// Remove deployed ants only if colony losses have exhausted its non-deployed ants.
    pub fn reconcile_population(&mut self, populations: [usize; 2]) {
        for (team, population) in populations.into_iter().enumerate() {
            let mut excess = self.alive[team].saturating_sub(population);
            for ant in self.ants.iter_mut().rev() {
                if excess == 0 {
                    break;
                }
                if ant.team == team && ant.health > 0.0 {
                    ant.health = 0.0;
                    self.alive[team] -= 1;
                    self.starvation[team] += 1;
                    excess -= 1;
                }
            }
        }
    }

    pub fn tick(&mut self, dt: f32, t: &Tuning) -> [u32; 2] {
        if self.winner.is_some() {
            return [0, 0];
        }
        self.elapsed += dt;
        for cell in &mut self.grid {
            cell.clear();
        }
        self.damage.fill(0.0);
        self.movement.fill((0.0, 0.0));
        self.fired.fill(false);
        let mut centers = [(0.0, 0.0); 2];
        for (i, ant) in self.ants.iter().enumerate() {
            if ant.health <= 0.0 {
                continue;
            }
            self.grid[cell_index(ant.x, ant.y)].push(i);
            centers[ant.team].0 += ant.x;
            centers[ant.team].1 += ant.y;
        }
        for (team, center) in centers.iter_mut().enumerate() {
            if self.alive[team] > 0 {
                center.0 /= self.alive[team] as f32;
                center.1 /= self.alive[team] as f32;
            }
        }
        // Snapshot decisions, then apply movement/damage together. Neither team gets
        // first-strike priority from its position in the ant array.
        for i in 0..self.ants.len() {
            let ant = &self.ants[i];
            if ant.health <= 0.0 {
                continue;
            }
            let gx = (ant.x / CELL) as usize;
            let gy = (ant.y / CELL) as usize;
            let mut nearest = None;
            let mut closest = f32::INFINITY;
            let mut separation = (0.0, 0.0);
            for y in gy.saturating_sub(1)..=(gy + 1).min(GRID_H - 1) {
                for x in gx.saturating_sub(1)..=(gx + 1).min(GRID_W - 1) {
                    for &j in &self.grid[y * GRID_W + x] {
                        if i == j {
                            continue;
                        }
                        let other = &self.ants[j];
                        let dx = other.x - ant.x;
                        let dy = other.y - ant.y;
                        let d2 = dx * dx + dy * dy;
                        if other.team != ant.team && d2 < closest {
                            closest = d2;
                            nearest = Some(j);
                        }
                        if d2 > 0.001 && d2 < 25.0 {
                            separation.0 -= dx / d2;
                            separation.1 -= dy / d2;
                        }
                    }
                }
            }
            let goal = nearest
                .map(|j| (self.ants[j].x, self.ants[j].y))
                .unwrap_or_else(|| {
                    let center = centers[1 - ant.team];
                    // Keep a broad advancing front instead of sending the whole
                    // army into one point. Gentle convergence still finds stragglers.
                    (center.0, ant.y + (center.1 - ant.y) * 0.2)
                });
            let dx = goal.0 - ant.x;
            let dy = goal.1 - ant.y;
            let distance = (dx * dx + dy * dy).sqrt().max(0.001);
            let advance = if closest <= REACH * REACH {
                0.0
            } else {
                t.ant_speed
            };
            let sx = (separation.0 * 22.0).clamp(-t.ant_speed, t.ant_speed);
            let sy = (separation.1 * 22.0).clamp(-t.ant_speed, t.ant_speed);
            self.movement[i] = (dx / distance * advance + sx, dy / distance * advance + sy);
            if closest <= REACH * REACH && ant.cooldown <= 0.0 {
                if let Some(j) = nearest {
                    self.damage[j] += t.attack_damage;
                    self.fired[i] = true;
                }
            }
        }
        let mut deaths = [0, 0];
        for (i, ant) in self.ants.iter_mut().enumerate() {
            if ant.health <= 0.0 {
                continue;
            }
            if self.fired[i] {
                ant.cooldown = t.attack_interval;
            }
            ant.cooldown -= dt;
            let (vx, vy) = self.movement[i];
            ant.previous_x = ant.x;
            ant.previous_y = ant.y;
            ant.x = (ant.x + vx * dt).clamp(12.0, ARENA_W - 12.0);
            ant.y = (ant.y + vy * dt).clamp(12.0, ARENA_H - 12.0);
            if vx * vx + vy * vy > 0.1 {
                ant.angle = vy.atan2(vx);
            }
            ant.health -= self.damage[i];
            if ant.health <= 0.0 {
                self.alive[ant.team] -= 1;
                deaths[ant.team] += 1;
            }
        }
        if self.alive[0] == 0 || self.alive[1] == 0 {
            // Defender retains the zone on mutual annihilation.
            self.winner = Some(if self.alive[0] > 0 { 0 } else { 1 });
        }
        deaths
    }
}

fn cell_index(x: f32, y: f32) -> usize {
    ((y / CELL) as usize).min(GRID_H - 1) * GRID_W + ((x / CELL) as usize).min(GRID_W - 1)
}
