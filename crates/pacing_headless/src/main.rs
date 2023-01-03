use gumdrop::Options;
use pacing_core::{
    config::{CLASSES, RACES},
    lingo::generate_name,
    mechanics::{Player, Simulation, Stats, StatsBuilder},
    Rand, SliceExt,
};
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Options)]
struct Args {
    #[options(help = "print this message")]
    help: bool,

    #[options(help = "generate a new character")]
    generate: bool,

    #[options(help = "run simulation for character", meta = "PATH", required)]
    character: PathBuf,
}

trait Print: std::fmt::Display {
    fn len(&self) -> usize;
}

impl Print for &str {
    fn len(&self) -> usize {
        str::len(self)
    }
}

impl Print for i32 {
    fn len(&self) -> usize {
        (*self as usize).len()
    }
}

impl Print for usize {
    fn len(&self) -> usize {
        count_digits(*self)
    }
}

const fn count_digits(num: usize) -> usize {
    let (mut len, mut n) = (1, 1);
    while len < 20 {
        n *= 10;
        if n > num {
            return len;
        }
        len += 1
    }
    len
}

fn main() {
    let rng = Rand::seed(
        std::time::SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap()
            .as_millis() as _,
    );

    let mut stats = StatsBuilder::default();

    let stats = stats.roll(&rng);

    let name = generate_name(None, &rng);
    let race = RACES.choice(&rng).clone();
    let class = CLASSES.choice(&rng).clone();

    let player = Player::new(name, race, class, stats);

    let mut simulation = Simulation::new(player);

    simulation.time_scale = 100.0;

    loop {
        simulation.tick(&rng);

        if let Some(task) = &simulation.player.task {
            println!("{}", task.description);
        }

        while !simulation.player.task_bar.is_done() {
            simulation.tick(&rng);
            if let Some(task) = &simulation.player.task {
                std::thread::sleep({
                    // Duration::from_secs(1)
                    let dt = task.duration.as_secs_f32() / simulation.time_scale;
                    Duration::from_secs_f32(dt)
                });
            }
        }
    }

    // let args = Args::parse_args_default_or_exit();

    // if args.generate {
    //     let mut stats_builder = StatsBuilder::default();
    //     let stats = stats_builder.roll(&rng);
    //     print_stats(stats);
    //     return;
    // }
}
