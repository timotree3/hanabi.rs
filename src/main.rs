extern crate rand;
#[macro_use]
extern crate log;

mod game;
mod strategies;
mod info;

#[allow(unused_imports)]
use log::LogLevel::{Trace, Debug, Info, Warn, Error};

struct SimpleLogger;
impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        metadata.level() <= Debug
    }

    fn log(&self, record: &log::LogRecord) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }
}

fn main() {
    log::set_logger(|max_log_level| {
        max_log_level.set(log::LogLevelFilter::Trace);
        Box::new(SimpleLogger)
    }).unwrap();

    let opts = game::GameOptions {
        num_players: 4,
        hand_size: 4,
        num_hints: 8,
        num_lives: 3,
    };
    let n = 1;
    // strategies::simulate(&opts, &strategies::AlwaysDiscard, n);
    // strategies::simulate(&opts, &strategies::AlwaysPlay, n);
    strategies::simulate(&opts, &strategies::RandomStrategy, n);
}
