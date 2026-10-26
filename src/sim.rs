use crate::{
    battle::Battle,
    config::Tuning,
    world::{World, COLONIES},
};

pub const STEP: f32 = 1.0 / 30.0;
pub const NAMES: [&str; COLONIES] = [
    "Your colony",
    "Copper colony",
    "Iris colony",
    "Jade colony",
    "Ochre colony",
];

pub struct Colony {
    pub population: u32,
    pub food: f64,
    pub harvest: f64,
    pub upkeep: f64,
    pub births: u32,
    pub combat_losses: u32,
    pub starvation_losses: u32,
    birth_credit: f64,
    starvation_credit: f64,
}

pub struct Expansion {
    pub zone: usize,
    pub elapsed: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    World,
    Battle,
}

pub struct Game {
    pub tuning: Tuning,
    pub seed: u64,
    pub world: World,
    pub colonies: Vec<Colony>,
    pub started: bool,
    pub time: f64,
    pub expansion: Option<Expansion>,
    pub battle: Option<Battle>,
    pub view: View,
    pub message: String,
    pub events: Vec<String>,
}

impl Game {
    pub fn new(seed: u64, tuning: Tuning) -> Self {
        let colonies = (0..COLONIES)
            .map(|_| Colony {
                population: tuning.starting_population,
                food: tuning.starting_stored_food,
                harvest: 0.0,
                upkeep: 0.0,
                births: 0,
                combat_losses: 0,
                starvation_losses: 0,
                birth_credit: 0.0,
                starvation_credit: 0.0,
            })
            .collect();
        Self {
            world: World::new(seed, &tuning),
            tuning,
            seed,
            colonies,
            started: false,
            time: 0.0,
            expansion: None,
            battle: None,
            view: View::World,
            message: "Choose a starting zone to place your queen.".into(),
            events: vec!["All colonies begin with identical ants and supplies.".into()],
        }
    }

    fn log(&mut self, message: String) {
        self.message = message.clone();
        self.events.insert(0, message);
        self.events.truncate(5);
    }

    pub fn choose_zone(&mut self, zone: usize) {
        if !self.started {
            if self.world.zones[zone].neighbors.is_empty() {
                self.message = "Choose a zone with a land neighbour.".into();
                return;
            }
            self.world.claim(zone, 0);
            self.world.seed_rivals(zone);
            self.started = true;
            self.log(format!(
                "Queen established in zone {:02}. Choose a neighbouring zone.",
                zone + 1
            ));
            return;
        }
        if self.colonies[0].population == 0 {
            self.message = "Your colony has died out. Press R to start a new world.".into();
        } else if self.expansion.is_some() || self.battle.as_ref().is_some_and(|b| !b.settled) {
            self.message = "Finish the current expansion or battle first.".into();
        } else if self.world.can_expand(zone, 0) {
            self.expansion = Some(Expansion { zone, elapsed: 0.0 });
            self.log(format!("Expanding toward zone {:02}...", zone + 1));
        } else {
            self.message = "Select a zone sharing a border with your territory.".into();
        }
    }

    pub fn deployed(&self, colony: usize) -> usize {
        self.battle.as_ref().filter(|b| !b.settled).map_or(0, |b| {
            if colony == 0 {
                b.alive[0]
            } else if colony == b.defender {
                b.alive[1]
            } else {
                0
            }
        })
    }

    pub fn army_size(&self, colony: usize) -> usize {
        let pop = self.colonies[colony].population;
        if pop == 0 {
            0
        } else {
            ((pop as f64 * self.tuning.army_fraction as f64).floor() as usize).max(1)
        }
    }

    pub fn tick(&mut self) {
        if !self.started || self.colonies[0].population == 0 {
            return;
        }
        self.time += STEP as f64;
        self.economy(STEP as f64);
        if self.colonies[0].population == 0 {
            self.expansion = None;
            self.log("Your colony has died out. Press R for a new world.".into());
        }

        let mut finished_expansion = None;
        if let Some(expansion) = &mut self.expansion {
            expansion.elapsed += STEP;
            if expansion.elapsed >= self.tuning.expansion_seconds {
                finished_expansion = Some(expansion.zone);
            }
        }
        if let Some(zone) = finished_expansion {
            self.expansion = None;
            if let Some(defender) = self.world.zones[zone].owner {
                let armies = [self.army_size(0), self.army_size(defender)];
                self.battle = Some(Battle::new(
                    zone,
                    defender,
                    armies,
                    self.seed.wrapping_add((self.time * 30.0) as u64),
                    &self.tuning,
                ));
                self.view = View::Battle;
                self.log(format!(
                    "Contact! {} ants meet {} defenders.",
                    armies[0], armies[1]
                ));
            } else {
                self.world.claim(zone, 0);
                self.log(format!(
                    "Zone {:02} claimed. Its food is now available to harvest.",
                    zone + 1
                ));
            }
        }

        let mut result = None;
        if let Some(battle) = &mut self.battle {
            if !battle.settled {
                battle.reconcile_population([
                    self.colonies[0].population as usize,
                    self.colonies[battle.defender].population as usize,
                ]);
                let deaths = battle.tick(STEP, &self.tuning);
                for (team, colony) in [0, battle.defender].into_iter().enumerate() {
                    self.colonies[colony].population = self.colonies[colony]
                        .population
                        .saturating_sub(deaths[team]);
                    self.colonies[colony].combat_losses += deaths[team];
                }
                if let Some(winner) = battle.winner {
                    battle.settled = true;
                    result = Some((winner, battle.zone));
                }
            }
        }
        if let Some((winner, zone)) = result {
            if winner == 0 {
                self.world.claim(zone, 0);
                self.log(format!(
                    "Victory. Zone {:02} captured; survivors return to the colony.",
                    zone + 1
                ));
            } else {
                self.log(format!(
                    "Attack repelled. Zone {:02} remains with its defenders.",
                    zone + 1
                ));
            }
        }
    }

    fn economy(&mut self, dt: f64) {
        for id in 0..self.colonies.len() {
            let deployed = self.deployed(id) as u32;
            let colony = &mut self.colonies[id];
            if colony.population == 0 {
                continue;
            }
            let owned = self.world.owned_count(id);
            let workers = colony.population.saturating_sub(deployed);
            let per_zone = if owned == 0 {
                0.0
            } else {
                (workers as f64 * self.tuning.harvest_per_ant_per_second / owned as f64)
                    .min(self.tuning.harvest_limit_per_zone_per_second)
            };
            let mut harvested = 0.0;
            for zone in &mut self.world.zones {
                if zone.owner == Some(id) {
                    let amount = (per_zone * dt).min(zone.food);
                    zone.food -= amount;
                    harvested += amount;
                }
            }
            colony.harvest = harvested / dt;
            colony.food += harvested;
            colony.upkeep = colony.population as f64 * self.tuning.upkeep_per_ant_per_second;
            let required = colony.upkeep * dt;
            let paid = required.min(colony.food);
            colony.food -= paid;
            if paid < required {
                // Eight unfed ant-seconds cause one death. Fractional losses are
                // carried across ticks, rather than rounding away small deficits.
                colony.starvation_credit += (required - paid)
                    / (self.tuning.upkeep_per_ant_per_second * self.tuning.starvation_seconds);
                let deaths = (colony.starvation_credit.floor() as u32).min(colony.population);
                colony.starvation_credit -= deaths as f64;
                colony.population -= deaths;
                colony.starvation_losses += deaths;
            }
            if owned > 0 && colony.population > 0 {
                colony.birth_credit = (colony.birth_credit + self.tuning.births_per_second * dt)
                    .min(self.tuning.births_per_second.max(1.0));
                let reserve = colony.upkeep * self.tuning.birth_reserve_seconds;
                let affordable =
                    ((colony.food - reserve).max(0.0) / self.tuning.food_per_birth).floor() as u32;
                let births = (colony.birth_credit.floor() as u32).min(affordable);
                colony.food -= births as f64 * self.tuning.food_per_birth;
                colony.population += births;
                colony.births += births;
                colony.birth_credit -= births as f64;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> Tuning {
        toml::from_str(include_str!("../tuning.toml")).unwrap()
    }
    fn started() -> Game {
        let mut game = Game::new(1234, tuning());
        let zone = game
            .world
            .zones
            .iter()
            .position(|z| z.neighbors.len() >= 2)
            .unwrap();
        game.choose_zone(zone);
        game
    }

    #[test]
    fn equal_initial_conditions_and_repeatable_food() {
        let game = started();
        let other = started();
        for c in &game.colonies {
            assert_eq!(c.population, game.tuning.starting_population);
            assert_eq!(c.food, game.tuning.starting_stored_food);
        }
        for (a, b) in game.world.zones.iter().zip(&other.world.zones) {
            assert_eq!(a.food, b.food);
            assert!(a.food >= game.tuning.zone_food_min && a.food <= game.tuning.zone_food_max);
        }
    }

    #[test]
    fn expansion_is_adjacent_and_only_one_at_a_time() {
        let mut game = started();
        let choices: Vec<_> = (0..game.world.zones.len())
            .filter(|&i| game.world.can_expand(i, 0))
            .collect();
        game.choose_zone(choices[0]);
        game.choose_zone(choices[1]);
        assert_eq!(game.expansion.as_ref().unwrap().zone, choices[0]);
        assert_eq!(game.world.owned_count(0), 1);
    }

    #[test]
    fn unoccupied_zone_is_only_claimed_after_timer() {
        let mut game = started();
        let zone = (0..game.world.zones.len())
            .find(|&i| game.world.can_expand(i, 0) && game.world.zones[i].owner.is_none())
            .unwrap();
        game.choose_zone(zone);
        game.tick();
        assert_eq!(game.world.zones[zone].owner, None);
        for _ in 0..190 {
            game.tick();
        }
        assert_eq!(game.world.zones[zone].owner, Some(0));
    }

    #[test]
    fn food_is_finite_and_population_is_conserved() {
        let mut game = started();
        let initial_food: f64 = game.world.zones.iter().map(|z| z.food).sum::<f64>()
            + game.colonies.iter().map(|c| c.food).sum::<f64>();
        for _ in 0..9000 {
            game.tick();
        }
        let final_food: f64 = game.world.zones.iter().map(|z| z.food).sum::<f64>()
            + game.colonies.iter().map(|c| c.food).sum::<f64>();
        assert!(final_food < initial_food);
        for zone in &game.world.zones {
            assert!(zone.food >= 0.0 && zone.food <= zone.initial_food);
        }
        for c in &game.colonies {
            assert!(c.food >= 0.0);
            assert_eq!(
                c.population,
                game.tuning.starting_population + c.births - c.starvation_losses - c.combat_losses
            );
        }
    }

    #[test]
    fn battle_casualties_are_accounted_once() {
        let mut game = started();
        let zone = game
            .world
            .zones
            .iter()
            .position(|z| z.owner == Some(1))
            .unwrap();
        game.choose_zone(zone);
        for _ in 0..6000 {
            game.tick();
            if game.battle.as_ref().is_some_and(|b| b.settled) {
                break;
            }
        }
        let battle = game.battle.as_ref().unwrap();
        assert!(battle.settled, "Battle must resolve without a stalemate");
        for (team, id) in [0, battle.defender].into_iter().enumerate() {
            assert_eq!(
                battle.initial[team] - battle.alive[team] - battle.starvation[team],
                game.colonies[id].combat_losses as usize
            );
            let c = &game.colonies[id];
            assert_eq!(
                c.population,
                game.tuning.starting_population + c.births - c.combat_losses - c.starvation_losses
            );
        }
        let losses = game.colonies[0].combat_losses;
        game.view = View::World;
        for _ in 0..60 {
            game.tick();
        }
        assert_eq!(game.colonies[0].combat_losses, losses);
    }

    #[test]
    fn no_food_eventually_kills_colony_without_negative_stocks() {
        let mut game = started();
        for z in &mut game.world.zones {
            z.food = 0.0;
        }
        for c in &mut game.colonies {
            c.food = 0.0;
        }
        for _ in 0..6000 {
            game.tick();
        }
        assert_eq!(game.colonies[0].population, 0);
        assert_eq!(game.colonies[0].food, 0.0);
    }

    #[test]
    fn starvation_cannot_leave_more_deployed_ants_than_population() {
        let t = tuning();
        let mut battle = Battle::new(0, 1, [100, 100], 42, &t);
        battle.reconcile_population([30, 0]);
        assert_eq!(battle.alive, [30, 0]);
        assert_eq!(battle.starvation, [70, 100]);
        assert_eq!(battle.tick(STEP, &t), [0, 0]);
        assert_eq!(battle.winner, Some(0));
    }

    #[test]
    fn all_zone_tiles_are_connected_and_adjacency_is_symmetric() {
        let game = started();
        for (id, zone) in game.world.zones.iter().enumerate() {
            assert!(zone.tiles > 0);
            for &n in &zone.neighbors {
                assert!(game.world.zones[n].neighbors.contains(&id));
            }
            let start = game
                .world
                .tiles
                .iter()
                .position(|z| *z == Some(id))
                .unwrap();
            let mut visited = std::collections::HashSet::from([start]);
            let mut queue = std::collections::VecDeque::from([start]);
            while let Some(i) = queue.pop_front() {
                let x = i % crate::world::MAP_W;
                let y = i / crate::world::MAP_W;
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0
                        || ny < 0
                        || nx >= crate::world::MAP_W as i32
                        || ny >= crate::world::MAP_H as i32
                    {
                        continue;
                    }
                    let n = ny as usize * crate::world::MAP_W + nx as usize;
                    if game.world.tiles[n] == Some(id) && visited.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
            assert_eq!(visited.len(), zone.tiles);
        }
    }

    #[test]
    fn view_switches_do_not_change_simulation_results() {
        let mut a = started();
        let mut b = started();
        let target = a
            .world
            .zones
            .iter()
            .position(|z| z.owner == Some(1))
            .unwrap();
        a.choose_zone(target);
        b.choose_zone(target);
        for _ in 0..1200 {
            a.view = View::World;
            b.view = View::Battle;
            a.tick();
            b.tick();
        }
        for (x, y) in a.colonies.iter().zip(&b.colonies) {
            assert_eq!(x.population, y.population);
            assert_eq!(x.food, y.food);
            assert_eq!(x.combat_losses, y.combat_losses);
        }
        assert_eq!(
            a.battle.as_ref().unwrap().alive,
            b.battle.as_ref().unwrap().alive
        );
    }

    #[test]
    fn invalid_tuning_is_rejected() {
        let mut t = tuning();
        assert!(t.validate().is_ok());
        t.army_fraction = 1.1;
        assert!(t.validate().is_err());
        t.army_fraction = f32::NAN;
        assert!(t.validate().is_err());
        t = tuning();
        t.food_per_birth = 0.0;
        assert!(t.validate().is_err());
        t = tuning();
        t.zone_food_min = t.zone_food_max + 1.0;
        assert!(t.validate().is_err());
    }
}
