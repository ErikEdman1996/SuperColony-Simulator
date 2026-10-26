use macroquad::prelude::*;
use supercolony::{
    battle::{ARENA_H, ARENA_W},
    config::Tuning,
    sim::{Game, View, NAMES, STEP},
    world::{MAP_H, MAP_W},
};

const BG: Color = color_u8!(12, 20, 24, 255);
const PANEL: Color = color_u8!(19, 31, 35, 255);
const LINE: Color = color_u8!(43, 61, 63, 255);
const INK: Color = color_u8!(228, 234, 219, 255);
const MUTED: Color = color_u8!(132, 156, 153, 255);
const ACCENT: Color = color_u8!(197, 224, 135, 255);
const RED: Color = color_u8!(228, 138, 109, 255);
const COLORS: [Color; 5] = [
    ACCENT,
    color_u8!(221, 133, 104, 255),
    color_u8!(162, 147, 213, 255),
    color_u8!(101, 189, 173, 255),
    color_u8!(219, 180, 98, 255),
];

fn window_conf() -> Conf {
    Conf {
        window_title: "Supercolony | local prototype".into(),
        window_width: 1440,
        window_height: 900,
        high_dpi: true,
        sample_count: 1,
        ..Default::default()
    }
}

fn label(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(text, x, y, size, color);
}

fn button(text: &str, rect: Rect, active: bool, enabled: bool) -> bool {
    let hover = rect.contains(Vec2::from(mouse_position()));
    let fill = if active {
        ACCENT
    } else if hover && enabled {
        LINE
    } else {
        PANEL
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.0,
        if active { ACCENT } else { LINE },
    );
    let color = if active {
        BG
    } else if enabled {
        INK
    } else {
        MUTED
    };
    let width = measure_text(text, None, 17, 1.0).width;
    label(
        text,
        rect.x + (rect.w - width) * 0.5,
        rect.y + rect.h * 0.65,
        17.0,
        color,
    );
    enabled && hover && is_mouse_button_pressed(MouseButton::Left)
}

fn bar(rect: Rect, fraction: f32, color: Color) {
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, BG);
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w * fraction.clamp(0.0, 1.0),
        rect.h,
        color,
    );
}

fn wrapped(text: &str, x: f32, mut y: f32, width: f32, size: u16, color: Color) -> f32 {
    let mut line = String::new();
    for word in text.split_whitespace() {
        let next = if line.is_empty() {
            word.into()
        } else {
            format!("{line} {word}")
        };
        if measure_text(&next, None, size, 1.0).width > width && !line.is_empty() {
            label(&line, x, y, size as f32, color);
            y += size as f32 * 1.35;
            line = word.into();
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        label(&line, x, y, size as f32, color);
        y += size as f32 * 1.35;
    }
    y
}

fn format_num(number: f64) -> String {
    if number >= 10000.0 {
        format!("{:.1}k", number / 1000.0)
    } else {
        format!("{number:.0}")
    }
}

struct MapArt {
    texture: Texture2D,
    revision: u64,
    seed: u64,
}

impl MapArt {
    fn new(game: &Game) -> Self {
        let texture =
            Texture2D::from_rgba8(MAP_W as u16, MAP_H as u16, &vec![0; MAP_W * MAP_H * 4]);
        texture.set_filter(FilterMode::Nearest);
        let mut art = Self {
            texture,
            revision: u64::MAX,
            seed: game.seed,
        };
        art.update(game);
        art
    }

    fn update(&mut self, game: &Game) {
        if self.revision == game.world.revision && self.seed == game.seed {
            return;
        }
        let mut image = Image::gen_image_color(MAP_W as u16, MAP_H as u16, BLANK);
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let i = y * MAP_W + x;
                let Some(id) = game.world.tiles[i] else {
                    continue;
                };
                let zone = &game.world.zones[id];
                let base = zone.owner.map_or_else(
                    || {
                        let k = ((id * 17) % 13) as f32 / 255.0;
                        Color::new(0.24 + k, 0.33 + k, 0.30 + k, 1.0)
                    },
                    |owner| {
                        let c = COLORS[owner];
                        Color::new(c.r * 0.66, c.g * 0.66, c.b * 0.66, 1.0)
                    },
                );
                let edge = (x > 0 && game.world.tiles[i - 1] != Some(id))
                    || (y > 0 && game.world.tiles[i - MAP_W] != Some(id));
                let color = if edge {
                    Color::new(base.r * 0.62, base.g * 0.62, base.b * 0.62, 1.0)
                } else {
                    base
                };
                image.set_pixel(x as u32, y as u32, color);
            }
        }
        self.texture.update(&image);
        self.revision = game.world.revision;
        self.seed = game.seed;
    }
}

fn map_transform(area: Rect, zoom: f32, pan: Vec2) -> (Vec2, f32) {
    let scale = (area.w / MAP_W as f32).min(area.h / MAP_H as f32) * 0.94 * zoom;
    let origin = vec2(area.x + area.w * 0.5, area.y + area.h * 0.5)
        - vec2(MAP_W as f32, MAP_H as f32) * scale * 0.5
        + pan;
    (origin, scale)
}

fn world_view(
    game: &mut Game,
    art: &mut MapArt,
    area: Rect,
    zoom: f32,
    pan: Vec2,
) -> Option<usize> {
    draw_rectangle(area.x, area.y, area.w, area.h, color_u8!(15, 30, 36, 255));
    // Clip map drawing during zoom, so it cannot cover the interface.
    let camera = Camera2D {
        // On-screen Camera2D applies an additional Y inversion. Positive zoom
        // keeps map drawing in the same top-left coordinates as mouse picking.
        zoom: vec2(2.0 / area.w, 2.0 / area.h),
        viewport: Some((
            area.x as i32,
            (screen_height() - area.y - area.h) as i32,
            area.w as i32,
            area.h as i32,
        )),
        ..Camera2D::from_display_rect(area)
    };
    set_camera(&camera);
    let (origin, scale) = map_transform(area, zoom, pan);
    for longitude in 0..=12 {
        let x = origin.x + longitude as f32 * MAP_W as f32 / 12.0 * scale;
        draw_line(
            x,
            origin.y,
            x,
            origin.y + MAP_H as f32 * scale,
            1.0,
            color_u8!(25, 43, 49, 255),
        );
    }
    for latitude in 0..=6 {
        let y = origin.y + latitude as f32 * MAP_H as f32 / 6.0 * scale;
        draw_line(
            origin.x,
            y,
            origin.x + MAP_W as f32 * scale,
            y,
            1.0,
            color_u8!(25, 43, 49, 255),
        );
    }
    art.update(game);
    draw_texture_ex(
        &art.texture,
        origin.x,
        origin.y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(MAP_W as f32 * scale, MAP_H as f32 * scale)),
            ..Default::default()
        },
    );
    let mouse = Vec2::from(mouse_position());
    let hovered = if area.contains(mouse) {
        game.world
            .zone_at((mouse.x - origin.x) / scale, (mouse.y - origin.y) / scale)
    } else {
        None
    };
    let ready = game.expansion.is_none()
        && !game.battle.as_ref().is_some_and(|b| !b.settled)
        && game.colonies[0].population > 0;
    for (id, zone) in game.world.zones.iter().enumerate() {
        let center = origin + vec2(zone.center.0, zone.center.1) * scale;
        if game.started && ready && game.world.can_expand(id, 0) {
            draw_circle_lines(center.x, center.y, 5.0, 1.5, ACCENT);
            draw_circle(center.x, center.y, 1.4, ACCENT);
        }
        if let Some(owner) = zone.owner {
            draw_circle(center.x, center.y, 6.5, BG);
            draw_circle(center.x, center.y, 3.5, COLORS[owner]);
        }
    }
    if let Some(id) = hovered {
        // Highlight just the hovered zone using a batched translucent tile overlay.
        for (i, tile) in game.world.tiles.iter().enumerate() {
            if *tile == Some(id) {
                draw_rectangle(
                    origin.x + (i % MAP_W) as f32 * scale,
                    origin.y + (i / MAP_W) as f32 * scale,
                    scale,
                    scale,
                    Color::new(0.90, 0.98, 0.80, 0.15),
                );
            }
        }
    }
    if let Some(expansion) = &game.expansion {
        let z = &game.world.zones[expansion.zone];
        let p = origin + vec2(z.center.0, z.center.1) * scale;
        draw_rectangle(p.x - 44.0, p.y - 29.0, 88.0, 17.0, BG);
        bar(
            Rect::new(p.x - 40.0, p.y - 25.0, 80.0, 9.0),
            expansion.elapsed / game.tuning.expansion_seconds,
            ACCENT,
        );
        label(
            &format!(
                "{:.1}s",
                (game.tuning.expansion_seconds - expansion.elapsed).max(0.0)
            ),
            p.x - 13.0,
            p.y - 34.0,
            15.0,
            INK,
        );
    }
    if let Some(battle) = &game.battle {
        if !battle.settled {
            let p = game.world.zones[battle.zone].center;
            let p = origin + vec2(p.0, p.1) * scale;
            draw_circle_lines(
                p.x,
                p.y,
                12.0 + (get_time() as f32 * 4.0).sin() * 2.0,
                2.0,
                RED,
            );
        }
    }
    set_default_camera();
    label(
        "WORLD / TERRITORY SURVEY",
        area.x + 20.0,
        area.y + 28.0,
        17.0,
        MUTED,
    );
    label(
        "SCROLL  zoom    RIGHT DRAG  pan    HOME  reset",
        area.x + 20.0,
        area.bottom() - 18.0,
        15.0,
        MUTED,
    );
    let legend_x = area.right() - 190.0;
    draw_circle(legend_x, area.bottom() - 23.0, 4.0, ACCENT);
    label(
        "Available neighbour",
        legend_x + 12.0,
        area.bottom() - 18.0,
        15.0,
        MUTED,
    );
    if let Some(zone) = hovered {
        if is_mouse_button_pressed(MouseButton::Left) {
            game.choose_zone(zone);
        }
    }
    hovered
}

fn ant_texture() -> Texture2D {
    // One shared sprite atlas entry. The renderer batches ants, not one draw call per ant.
    let mut image = Image::gen_image_color(48, 32, BLANK);
    for y in 0..32 {
        for x in 0..48 {
            let px = x as f32;
            let py = y as f32;
            let ellipse = |cx: f32, cy: f32, rx: f32, ry: f32| {
                ((px - cx) / rx).powi(2) + ((py - cy) / ry).powi(2) <= 1.0
            };
            let body = ellipse(13.0, 16.0, 9.0, 6.0)
                || ellipse(26.0, 16.0, 5.5, 4.0)
                || ellipse(35.0, 16.0, 5.5, 5.0);
            let segment = |ax: f32, ay: f32, bx: f32, by: f32| {
                let vx = bx - ax;
                let vy = by - ay;
                let t = (((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
                (px - ax - t * vx).powi(2) + (py - ay - t * vy).powi(2) < 0.9
            };
            let legs = [
                (21.0, 12.0, 17.0, 4.0),
                (25.0, 12.0, 27.0, 3.0),
                (29.0, 12.0, 35.0, 5.0),
                (21.0, 20.0, 17.0, 28.0),
                (25.0, 20.0, 27.0, 29.0),
                (29.0, 20.0, 35.0, 27.0),
                (38.0, 13.0, 44.0, 9.0),
                (38.0, 19.0, 44.0, 23.0),
            ]
            .iter()
            .any(|&(a, b, c, d)| segment(a, b, c, d));
            if body || legs {
                image.set_pixel(x, y, WHITE);
            }
        }
    }
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Linear);
    texture
}

fn battle_view(game: &Game, texture: &Texture2D, area: Rect, alpha: f32) {
    draw_rectangle(area.x, area.y, area.w, area.h, color_u8!(31, 35, 29, 255));
    let Some(battle) = &game.battle else {
        return;
    };
    let scale = ((area.w - 48.0) / ARENA_W).min((area.h - 130.0) / ARENA_H);
    let origin = vec2(
        area.x + (area.w - ARENA_W * scale) * 0.5,
        area.y + 78.0 + (area.h - 130.0 - ARENA_H * scale) * 0.5,
    );
    // Static deterministic ground details, with no collision/physics overhead.
    for i in 0..220 {
        let x = ((i * 617 + 97) % 1031) as f32;
        let y = ((i * 337 + 43) % 587) as f32;
        draw_circle(
            origin.x + x * scale,
            origin.y + y * scale,
            (1 + (i % 3)) as f32 * scale,
            color_u8!(46, 48, 37, 255),
        );
    }
    draw_rectangle_lines(
        origin.x,
        origin.y,
        ARENA_W * scale,
        ARENA_H * scale,
        1.0,
        LINE,
    );
    draw_line(
        origin.x + ARENA_W * 0.5 * scale,
        origin.y,
        origin.x + ARENA_W * 0.5 * scale,
        origin.y + ARENA_H * scale,
        1.0,
        color_u8!(52, 54, 41, 255),
    );
    // Corpses first, then live agents. All share one texture.
    for alive_pass in [false, true] {
        for (i, ant) in battle.ants.iter().enumerate() {
            let alive = ant.health > 0.0;
            if alive != alive_pass {
                continue;
            }
            let x = if alive && battle.winner.is_none() {
                ant.previous_x + (ant.x - ant.previous_x) * alpha
            } else {
                ant.x
            };
            let y = if alive && battle.winner.is_none() {
                ant.previous_y + (ant.y - ant.previous_y) * alpha
            } else {
                ant.y
            };
            let wiggle = if alive && battle.winner.is_none() {
                (battle.elapsed * 24.0 + i as f32 * 1.7).sin() * 0.11
            } else {
                0.0
            };
            let color = if alive {
                COLORS[if ant.team == 0 { 0 } else { battle.defender }]
            } else {
                color_u8!(88, 75, 53, 140)
            };
            draw_texture_ex(
                texture,
                origin.x + x * scale - 6.5 * scale,
                origin.y + y * scale - 4.3 * scale,
                color,
                DrawTextureParams {
                    dest_size: Some(vec2(13.0 * scale, 8.6 * scale)),
                    rotation: ant.angle + wiggle,
                    ..Default::default()
                },
            );
        }
    }
    label(
        &format!("CONTACT / ZONE {:02}", battle.zone + 1),
        area.x + 20.0,
        area.y + 29.0,
        17.0,
        MUTED,
    );
    label(
        &format!("YOUR ARMY    {} / {}", battle.alive[0], battle.initial[0]),
        area.x + 20.0,
        area.y + 57.0,
        21.0,
        ACCENT,
    );
    let enemy_text = format!("{} / {}    DEFENDERS", battle.alive[1], battle.initial[1]);
    let width = measure_text(&enemy_text, None, 21, 1.0).width;
    label(
        &enemy_text,
        area.right() - 20.0 - width,
        area.y + 57.0,
        21.0,
        COLORS[battle.defender],
    );
    label(
        "AUTOMATIC COMBAT  /  No orders required. The colony economy remains live.",
        area.x + 20.0,
        area.bottom() - 20.0,
        15.0,
        MUTED,
    );
    if let Some(winner) = battle.winner {
        let w = area.w.min(510.0) - 60.0;
        let rect = Rect::new(
            area.x + (area.w - w) * 0.5,
            area.y + area.h * 0.43,
            w,
            106.0,
        );
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, BG);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, LINE);
        label(
            if winner == 0 {
                "TERRITORY SECURED"
            } else {
                "ATTACK REPELLED"
            },
            rect.x + 24.0,
            rect.y + 40.0,
            27.0,
            if winner == 0 { ACCENT } else { RED },
        );
        label(
            "Survivors returned. Press TAB for the world map.",
            rect.x + 24.0,
            rect.y + 77.0,
            17.0,
            MUTED,
        );
    }
}

fn sidebar(game: &Game, hovered: Option<usize>, area: Rect) {
    draw_rectangle(area.x, area.y, area.w, area.h, PANEL);
    let x = area.x + 22.0;
    let w = area.w - 44.0;
    let mut y = area.y + 29.0;
    label("COLONY LEDGER", x, y, 16.0, MUTED);
    y += 39.0;
    label(
        &format_num(game.colonies[0].population as f64),
        x,
        y,
        39.0,
        INK,
    );
    label("LIVING ANTS", x + 104.0, y - 4.0, 14.0, MUTED);
    y += 35.0;
    let rows = [
        ("Ready attack army", format!("{}", game.army_size(0))),
        ("Deployed", format!("{}", game.deployed(0))),
        (
            "Zones controlled",
            format!("{} / {}", game.world.owned_count(0), game.world.zones.len()),
        ),
        ("Stored food", format_num(game.colonies[0].food)),
        (
            "Food in territory",
            format_num(game.world.remaining_food(0)),
        ),
        ("Harvest / sec", format!("+{:.1}", game.colonies[0].harvest)),
        ("Upkeep / sec", format!("-{:.1}", game.colonies[0].upkeep)),
    ];
    for (name, value) in rows {
        label(name, x, y, 16.0, MUTED);
        let width = measure_text(&value, None, 17, 1.0).width;
        label(&value, area.right() - 22.0 - width, y, 17.0, INK);
        y += 27.0;
    }
    y += 3.0;
    draw_line(x, y, x + w, y, 1.0, LINE);
    y += 29.0;
    if game.view == View::Battle {
        if let Some(b) = &game.battle {
            label("BATTLE REPORT", x, y, 16.0, MUTED);
            y += 29.0;
            label(&format!("Elapsed   {:.1}s", b.elapsed), x, y, 18.0, INK);
            y += 27.0;
            for (team, name) in [(0, "Your army"), (1, "Defenders")] {
                label(
                    &format!("{name}: {} surviving", b.alive[team]),
                    x,
                    y,
                    16.0,
                    COLORS[if team == 0 { 0 } else { b.defender }],
                );
                y += 20.0;
                bar(
                    Rect::new(x, y, w, 5.0),
                    b.alive[team] as f32 / b.initial[team].max(1) as f32,
                    COLORS[if team == 0 { 0 } else { b.defender }],
                );
                y += 27.0;
            }
            y = wrapped("Every ant on the field is a simulated individual. Casualties are deducted from the colony once.",x,y,w,16,MUTED);
        }
    } else if let Some(id) = hovered {
        let zone = &game.world.zones[id];
        label(&format!("ZONE {:02}", id + 1), x, y, 16.0, MUTED);
        y += 30.0;
        label(
            zone.owner.map_or("Unoccupied", |o| NAMES[o]),
            x,
            y,
            22.0,
            zone.owner.map_or(INK, |o| COLORS[o]),
        );
        y += 30.0;
        label(
            &format!("{} food remaining", format_num(zone.food)),
            x,
            y,
            18.0,
            INK,
        );
        y += 16.0;
        bar(
            Rect::new(x, y, w, 5.0),
            (zone.food / zone.initial_food) as f32,
            ACCENT,
        );
        y += 30.0;
        y = wrapped(
            if !game.started {
                "Click to establish your queen here."
            } else if game.world.can_expand(id, 0) {
                "Click to expand here. Occupied territory triggers a battle after the claim timer."
            } else {
                "Expansion requires a shared land border with your colony."
            },
            x,
            y,
            w,
            16,
            MUTED,
        );
    } else {
        label("FIELD NOTES", x, y, 16.0, MUTED);
        y += 28.0;
        y = wrapped(
            if !game.started {
                "Choose any connected land zone. Your queen and four rival colonies will begin together."
            } else {
                "Hover a zone to inspect it. Ringed zones share a border with your colony and can be selected next."
            },
            x,
            y,
            w,
            16,
            INK,
        );
    }
    y += 15.0;
    draw_line(x, y, x + w, y, 1.0, LINE);
    y += 28.0;
    label("RECENT ACTIVITY", x, y, 16.0, MUTED);
    y += 28.0;
    for event in game.events.iter().take(3) {
        if y > area.bottom() - 65.0 {
            break;
        }
        y = wrapped(event, x, y, w, 15, INK) + 12.0;
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    #[cfg(target_os = "linux")]
    {
        // Macroquad has already created a current OpenGL context on this thread.
        // Report its driver so software rendering isn't mistaken for a swarm limit.
        unsafe {
            let renderer = macroquad::miniquad::gl::glGetString(0x1F01); // GL_RENDERER
            if !renderer.is_null() {
                eprintln!(
                    "OpenGL renderer: {}",
                    std::ffi::CStr::from_ptr(renderer.cast()).to_string_lossy()
                );
            }
        }
    }
    let args: Vec<String> = std::env::args().collect();
    let tuning = match Tuning::load() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Invalid tuning.toml: {e}");
            std::process::exit(2);
        }
    };
    let capture = args.get(1).is_some_and(|s| s == "--capture");
    let capture_battle = capture && args.get(2).is_some_and(|s| s == "battle");
    let seed = args
        .iter()
        .position(|s| s == "--seed")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });
    let mut game = Game::new(seed, tuning.clone());
    if capture {
        let start = game
            .world
            .zones
            .iter()
            .enumerate()
            .filter(|(_, z)| z.neighbors.len() >= 3)
            .min_by(|(_, a), (_, b)| {
                let d = |z: &supercolony::world::Zone| {
                    (z.center.0 - 116.0).powi(2) + (z.center.1 - 40.0).powi(2)
                };
                d(a).total_cmp(&d(b))
            })
            .map(|(i, _)| i)
            .unwrap();
        game.choose_zone(start);
        if capture_battle {
            let target = game
                .world
                .zones
                .iter()
                .position(|z| z.owner == Some(1))
                .unwrap();
            game.choose_zone(target);
            for _ in 0..480 {
                game.tick();
            }
        } else {
            let target = (0..game.world.zones.len())
                .find(|&i| game.world.can_expand(i, 0) && game.world.zones[i].owner.is_none())
                .unwrap();
            game.choose_zone(target);
            for _ in 0..90 {
                game.tick();
            }
        }
    }
    let mut art = MapArt::new(&game);
    let ants = ant_texture();
    let mut accumulator = 0.0;
    let mut paused = false;
    let mut speed = 1.0;
    let mut zoom = 1.0;
    let mut pan = Vec2::ZERO;
    let mut previous_mouse = Vec2::ZERO;
    let mut frames = 0;
    let mut sim_ms = 0.0f64;
    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::Space) {
            paused = !paused;
        }
        if is_key_pressed(KeyCode::Tab) && game.battle.is_some() {
            game.view = if game.view == View::World {
                View::Battle
            } else {
                View::World
            };
        }
        if is_key_pressed(KeyCode::R) {
            game = Game::new(game.seed.wrapping_add(1), tuning.clone());
            accumulator = 0.0;
            pan = Vec2::ZERO;
            zoom = 1.0;
            paused = false;
        }
        if is_key_pressed(KeyCode::Home) {
            pan = Vec2::ZERO;
            zoom = 1.0;
        }
        let w = screen_width();
        let h = screen_height();
        if w < 900.0 || h < 650.0 {
            clear_background(BG);
            label(
                "Please enlarge the window to at least 900 x 650.",
                30.0,
                80.0,
                25.0,
                INK,
            );
            next_frame().await;
            continue;
        }
        let side_w = 300.0;
        let area = Rect::new(24.0, 162.0, w - side_w - 60.0, h - 260.0);
        let sidebar_area = Rect::new(w - side_w - 24.0, 162.0, side_w, h - 260.0);
        let mouse = Vec2::from(mouse_position());
        if game.view == View::World && area.contains(mouse) {
            let wheel = mouse_wheel().1;
            if wheel != 0.0 {
                let (old_origin, old_scale) = map_transform(area, zoom, pan);
                let world_mouse = (mouse - old_origin) / old_scale;
                zoom = (zoom * (1.0 + wheel * 0.12)).clamp(1.0, 4.0);
                let (new_origin, new_scale) = map_transform(area, zoom, pan);
                pan += mouse - (new_origin + world_mouse * new_scale);
            }
            if is_mouse_button_down(MouseButton::Right) {
                pan += mouse - previous_mouse;
            }
        }
        previous_mouse = mouse;
        if !paused && !capture {
            accumulator += (get_frame_time().min(0.25)) * speed;
            let start = std::time::Instant::now();
            let mut steps = 0;
            while accumulator >= STEP && steps < 12 {
                game.tick();
                accumulator -= STEP;
                steps += 1;
            }
            // Discard excess wall-clock debt rather than taking unstable giant steps.
            if steps == 12 {
                accumulator = accumulator.min(STEP);
            }
            if steps > 0 {
                sim_ms = sim_ms * 0.9 + start.elapsed().as_secs_f64() * 1000.0 / steps as f64 * 0.1;
            }
        }
        clear_background(BG);
        label("SUPER", 24.0, 48.0, 34.0, INK);
        label("COLONY", 117.0, 48.0, 34.0, ACCENT);
        label(
            "A SMALL BEGINNING. A WORLD TO CLAIM.",
            25.0,
            74.0,
            14.0,
            MUTED,
        );
        label(
            "LOCAL PROTOTYPE  /  NATIVE RUST",
            w - 324.0,
            38.0,
            16.0,
            MUTED,
        );
        label(
            &format!(
                "SEED {}    |    {:02}:{:02}",
                game.seed,
                game.time as u64 / 60,
                game.time as u64 % 60
            ),
            w - 324.0,
            65.0,
            15.0,
            MUTED,
        );
        draw_line(24.0, 94.0, w - 24.0, 94.0, 1.0, LINE);
        if button(
            "01  WORLD",
            Rect::new(24.0, 110.0, 146.0, 35.0),
            game.view == View::World,
            true,
        ) {
            game.view = View::World;
        }
        if button(
            "02  ATTACK",
            Rect::new(178.0, 110.0, 146.0, 35.0),
            game.view == View::Battle,
            game.battle.is_some(),
        ) {
            game.view = View::Battle;
        }
        if button(
            if paused { "RESUME" } else { "PAUSE" },
            Rect::new(w - 350.0, 110.0, 100.0, 35.0),
            false,
            true,
        ) {
            paused = !paused;
        }
        for (i, s) in [1.0, 2.0, 4.0].into_iter().enumerate() {
            if button(
                &format!("{s:.0}x"),
                Rect::new(w - 240.0 + i as f32 * 48.0, 110.0, 42.0, 35.0),
                speed == s,
                true,
            ) {
                speed = s;
            }
        }
        if button("NEW", Rect::new(w - 88.0, 110.0, 64.0, 35.0), false, true) {
            game = Game::new(game.seed.wrapping_add(1), tuning.clone());
            accumulator = 0.0;
            pan = Vec2::ZERO;
            zoom = 1.0;
            paused = false;
        }
        let hovered = if game.view == View::World {
            world_view(&mut game, &mut art, area, zoom, pan)
        } else {
            battle_view(
                &game,
                &ants,
                area,
                if paused || capture {
                    1.0
                } else {
                    (accumulator / STEP).clamp(0.0, 1.0)
                },
            );
            None
        };
        sidebar(&game, hovered, sidebar_area);
        draw_rectangle_lines(area.x, area.y, area.w, area.h, 1.0, LINE);
        let status = Rect::new(24.0, h - 81.0, w - 48.0, 37.0);
        draw_rectangle(status.x, status.y, status.w, status.h, PANEL);
        let prefix = if paused {
            "PAUSED  /  "
        } else if !game.started {
            "FOUND YOUR COLONY  /  "
        } else {
            ""
        };
        label(
            &format!("{prefix}{}", game.message),
            status.x + 14.0,
            status.y + 24.0,
            17.0,
            if paused { RED } else { INK },
        );
        label(
            "SPACE  pause    TAB  switch view    R  new world    ESC  quit",
            24.0,
            h - 18.0,
            14.0,
            MUTED,
        );
        let metrics = format!(
            "{} FPS  /  {:.2} ms tick  /  provisional rules",
            get_fps(),
            sim_ms
        );
        let width = measure_text(&metrics, None, 14, 1.0).width;
        label(&metrics, w - 24.0 - width, h - 18.0, 14.0, MUTED);
        if !game.started {
            let rect = Rect::new(area.x + 24.0, area.y + area.h - 122.0, 415.0, 66.0);
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, BG);
            label(
                "Every supercolony starts with one queen.",
                rect.x + 16.0,
                rect.y + 26.0,
                21.0,
                INK,
            );
            label(
                "Select a land zone. Time begins when you settle.",
                rect.x + 16.0,
                rect.y + 49.0,
                16.0,
                MUTED,
            );
        }
        if capture && frames == 60 {
            let output = args.get(3).map(String::as_str).unwrap_or("capture.png");
            get_screen_data().export_png(output);
            println!("Captured {output}");
            break;
        }
        frames += 1;
        next_frame().await;
    }
}
