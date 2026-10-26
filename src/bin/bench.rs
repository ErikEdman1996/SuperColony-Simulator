//! A genuinely headless benchmark: no Macroquad window or graphics context.
use supercolony::{battle::Battle, config::Tuning, sim::STEP};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let count = args
        .get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10000);
    let ticks = args
        .get(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(900);
    if !(2..=1_000_000).contains(&count) || ticks == 0 {
        eprintln!(
            "Usage: cargo run --release --bin bench -- <total ants: 2..1000000> <ticks: positive>"
        );
        std::process::exit(2);
    }
    let tuning = Tuning::load().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let mut battle = Battle::new(0, 1, [count / 2, count - count / 2], 42, &tuning);
    let mut times = Vec::with_capacity(ticks);
    let mut contact_times = Vec::new();
    for _ in 0..ticks {
        let start = std::time::Instant::now();
        battle.tick(STEP, &tuning);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        times.push(elapsed);
        if battle.alive != battle.initial {
            contact_times.push(elapsed);
        }
        if battle.winner.is_some() {
            break;
        }
    }
    println!(
        "Headless combat only; total agents={count}, ticks={}, remaining={:?}",
        times.len(),
        battle.alive
    );
    report("All ticks", &mut times);
    if !contact_times.is_empty() {
        report("After first casualty", &mut contact_times);
    }
    println!("Rendering, windowing, and economy are NOT included. 30 Hz tick budget: 33.33 ms.");
}

fn report(name: &str, times: &mut [f64]) {
    times.sort_by(f64::total_cmp);
    let p = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
    println!(
        "{name}: median={:.3} ms, p95={:.3} ms, p99={:.3} ms, max={:.3} ms",
        p(0.5),
        p(0.95),
        p(0.99),
        times[times.len() - 1]
    );
}
