extern crate getopts;
#[macro_use]
extern crate log;
extern crate rand;
extern crate crossbeam;

mod game;
mod simulator;
mod strategies {
    pub mod examples;
    pub mod cheating;
}
mod info;

use getopts::Options;
use std::str::FromStr;

struct SimpleLogger;
impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        metadata.level() <= log::LogLevel::Trace
    }

    fn log(&self, record: &log::LogRecord) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }
}


fn print_usage(program: &str, opts: Options) {
    print!("{}", opts.usage(&format!("Usage: {} [options]", program)));
}


fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("l", "loglevel", "Log level, one of 'trace', 'debug', 'info', 'warn', and 'error'", "LOGLEVEL");
    opts.optopt("n", "ntrials", "Number of games to simulate", "NTRIALS");
    opts.optopt("t", "nthreads", "Number of threads to use for simulation", "NTHREADS");
    opts.optopt("s", "seed", "Seed for PRNG (can only be used with n=1)", "SEED");
    opts.optopt("p", "nplayers", "Number of players", "NPLAYERS");
    opts.optflag("h", "help", "Print this help menu");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => {
            print_usage(&program, opts);
            panic!(f.to_string())
        }
    };
    if matches.opt_present("h") {
        return print_usage(&program, opts);
    }
    if !matches.free.is_empty() {
        return print_usage(&program, opts);
    }

    let log_level_str : &str = &matches.opt_str("l").unwrap_or("info".to_string());
    let log_level = match log_level_str {
        "trace" => { log::LogLevelFilter::Trace }
        "debug" => { log::LogLevelFilter::Debug }
        "info"  => { log::LogLevelFilter::Info }
        "warn"  => { log::LogLevelFilter::Warn }
        "error" => { log::LogLevelFilter::Error }
        _       => {
            print_usage(&program, opts);
            panic!("Unexpected log level argument {}", log_level_str);
        }
    };

    log::set_logger(|max_log_level| {
        max_log_level.set(log_level);
        Box::new(SimpleLogger)
    }).unwrap();

    let n = u32::from_str(&matches.opt_str("n").unwrap_or("1".to_string())).unwrap();

    let seed = matches.opt_str("s").map(|seed_str| { u32::from_str(&seed_str).unwrap() });

    let n_threads = u32::from_str(&matches.opt_str("t").unwrap_or("1".to_string())).unwrap();

    let n_players = u32::from_str(&matches.opt_str("p").unwrap_or("4".to_string())).unwrap();
    let hand_size = match n_players {
        2 => 5,
        3 => 5,
        4 => 4,
        5 => 4,
        _ => { panic!("There should be 2 to 5 players, not {}", n_players); }
    };

    let opts = game::GameOptions {
        num_players: n_players,
        hand_size: hand_size,
        num_hints: 8,
        num_lives: 3,
        // hanabi rules are a bit ambiguous about whether you can give hints that match 0 cards
        allow_empty_hints: false,
    };

    // TODO: make this configurable
    // let strategy_config = strategies::examples::RandomStrategyConfig {
    //     hint_probability: 0.4,
    //     play_probability: 0.2,
    // };
    let strategy_config = strategies::cheating::CheatingStrategyConfig::new();
    simulator::simulate(&opts, &strategy_config, seed, n, n_threads);
}
