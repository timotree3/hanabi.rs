extern crate rand;
#[macro_use]
extern crate log;

mod game;
mod strategies;
mod info;


struct SimpleLogger;
impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        // metadata.level() <= log::LogLevel::Warn
        metadata.level() <= log::LogLevel::Info
        // metadata.level() <= log::LogLevel::Debug
    }

    fn log(&self, record: &log::LogRecord) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }
}

fn main() {
    log::set_logger(|max_log_level| {
        max_log_level.set(log::LogLevelFilter::Info);
        Box::new(SimpleLogger)
    });

    let opts = game::GameOptions {
        num_players: 4,
        hand_size: 4,
        num_hints: 8,
        num_lives: 3,
    };
    strategies::simulate(&opts, &strategies::AlwaysPlay, 100);
}
