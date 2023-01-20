mod game;
mod helpers;
mod json_output;
mod simulator;
mod strategy;
mod strategies {
    pub mod cheating;
    pub mod examples;
    mod hat_helpers;
    pub mod information;
}

use getopts::Options;
use std::str::FromStr;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

fn print_usage(program: &str, opts: Options) {
    print!("{}", opts.usage(&format!("Usage: {program} [options]")));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt(
        "n",
        "ntrials",
        "Number of games to simulate (default 1)",
        "NTRIALS",
    );
    opts.optopt(
        "o",
        "output",
        "Number of games after which to print an update",
        "OUTPUT_FREQ",
    );
    opts.optopt(
        "j",
        "json-output",
        "Pattern for the JSON output file. '%s' will be replaced by the seed.",
        "FILE_PATTERN",
    );
    opts.optopt(
        "t",
        "nthreads",
        "Number of threads to use for simulation (default 1)",
        "NTHREADS",
    );
    opts.optopt("s", "seed", "Seed for PRNG (default random)", "SEED");
    opts.optopt("p", "nplayers", "Number of players", "NPLAYERS");
    opts.optopt(
        "g",
        "strategy",
        "Which strategy to use.  One of 'random', 'cheat', and 'info'",
        "STRATEGY",
    );
    opts.optflag("h", "help", "Print this help menu");
    opts.optflag(
        "",
        "results-table",
        "Print a table of results for each strategy",
    );
    opts.optflag(
        "",
        "write-results-table",
        "Update the results table in README.md",
    );
    opts.optflag(
        "",
        "losses-only",
        "When saving JSON outputs, save lost games only",
    );
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            print_usage(&program, opts);
            panic!("{}", f)
        }
    };
    if matches.opt_present("h") {
        return print_usage(&program, opts);
    }
    if !matches.free.is_empty() {
        return print_usage(&program, opts);
    }
    if matches.opt_present("write-results-table") {
        return write_results_table();
    }
    if matches.opt_present("results-table") {
        return print!("{}", get_results_table());
    }

    // Register logging controlled by RUST_LOG=
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);
    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .or_else(|_| tracing_subscriber::EnvFilter::try_new("info"))
        .unwrap();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    let n_trials = u32::from_str(matches.opt_str("n").as_deref().unwrap_or("1")).unwrap();
    let seed = matches
        .opt_str("s")
        .map(|seed_str| u64::from_str(&seed_str).unwrap());
    let progress_info = matches
        .opt_str("o")
        .map(|freq_str| u32::from_str(&freq_str).unwrap());
    let n_threads = u32::from_str(matches.opt_str("t").as_deref().unwrap_or("1")).unwrap();
    let n_players = u32::from_str(matches.opt_str("p").as_deref().unwrap_or("4")).unwrap();
    let g_opt = matches.opt_str("g");
    let strategy_str: &str = g_opt.as_deref().unwrap_or("cheat");
    let json_output_pattern = matches.opt_str("j");
    let json_losses_only = matches.opt_present("losses-only");

    sim_games(
        n_players,
        strategy_str,
        seed,
        n_trials,
        n_threads,
        progress_info,
        json_output_pattern,
        json_losses_only,
    )
    .info();
}

fn sim_games(
    n_players: u32,
    strategy_str: &str,
    seed: Option<u64>,
    n_trials: u32,
    n_threads: u32,
    progress_info: Option<u32>,
    json_output_pattern: Option<String>,
    json_losses_only: bool,
) -> simulator::SimResult {
    let hand_size = match n_players {
        2 => 5,
        3 => 5,
        4 => 4,
        5 => 4,
        _ => {
            panic!("There should be 2 to 5 players, not {n_players}");
        }
    };

    let game_opts = game::GameOptions {
        num_players: n_players,
        hand_size,
        num_hints: 8,
        num_lives: 3,
        // hanabi rules are a bit ambiguous about whether you can give hints that match 0 cards
        allow_empty_hints: false,
    };

    let strategy_config: Box<dyn strategy::GameStrategyConfig + Sync> = match strategy_str {
        "random" => Box::new(strategies::examples::RandomStrategyConfig {
            hint_probability: 0.4,
            play_probability: 0.2,
        }) as Box<dyn strategy::GameStrategyConfig + Sync>,
        "cheat" => Box::new(strategies::cheating::CheatingStrategyConfig::new())
            as Box<dyn strategy::GameStrategyConfig + Sync>,
        "info" => Box::new(strategies::information::InformationStrategyConfig::new())
            as Box<dyn strategy::GameStrategyConfig + Sync>,
        _ => {
            panic!("Unexpected strategy argument {strategy_str}");
        }
    };
    simulator::simulate(
        &game_opts,
        strategy_config,
        seed,
        n_trials,
        n_threads,
        progress_info,
        json_output_pattern,
        json_losses_only,
    )
}

fn get_results_table() -> String {
    let strategies = ["cheat", "info"];
    let player_nums = (2..=5).collect::<Vec<_>>();
    let seed = 0;
    let n_trials = 20000;
    let n_threads = 8;

    let intro = format!(
        "On the first {n_trials} seeds, we have these scores and win rates (average ± standard error):\n\n"
    );
    let format_name = |x| format!(" {x:7} ");
    let format_players = |x| format!("   {x}p    ");
    let format_percent = |x, stderr| format!(" {x:05.2} ± {stderr:.2} % ");
    let format_score = |x, stderr| format!(" {x:07.4} ± {stderr:.4} ");
    let space = String::from("         ");
    let dashes = String::from("---------");
    let dashes_long = String::from("------------------");
    type TwoLines = (String, String);
    fn make_twolines(
        player_nums: &[u32],
        head: TwoLines,
        make_block: &dyn Fn(u32) -> TwoLines,
    ) -> TwoLines {
        let mut blocks = player_nums
            .iter()
            .cloned()
            .map(make_block)
            .collect::<Vec<_>>();
        blocks.insert(0, head);
        fn combine(items: Vec<String>) -> String {
            items
                .iter()
                .fold(String::from("|"), |init, next| init + next + "|")
        }
        let (a, b): (Vec<_>, Vec<_>) = blocks.into_iter().unzip();
        (combine(a), combine(b))
    }
    fn concat_twolines(body: Vec<TwoLines>) -> String {
        body.into_iter().fold(String::default(), |output, (a, b)| {
            output + &a + "\n" + &b + "\n"
        })
    }
    let header = make_twolines(&player_nums, (space.clone(), dashes), &|n_players| {
        (format_players(n_players), dashes_long.clone())
    });
    let mut body = strategies
        .iter()
        .map(|strategy| {
            make_twolines(
                &player_nums,
                (format_name(strategy), space.clone()),
                &|n_players| {
                    let simresult = sim_games(
                        n_players,
                        strategy,
                        Some(seed),
                        n_trials,
                        n_threads,
                        None,
                        None,
                        false,
                    );
                    (
                        format_score(simresult.average_score(), simresult.score_stderr()),
                        format_percent(
                            simresult.percent_perfect(),
                            simresult.percent_perfect_stderr(),
                        ),
                    )
                },
            )
        })
        .collect::<Vec<_>>();
    body.insert(0, header);
    intro + &concat_twolines(body)
}

fn write_results_table() {
    let separator = r#"
## Results (auto-generated)

To reproduce:
```
time cargo run --release -- --results-table
```

To update this file:
```
time cargo run --release -- --write-results-table
```

"#;
    let readme = "README.md";
    let readme_contents = std::fs::read_to_string(readme).unwrap();
    let readme_init = {
        let parts = readme_contents.splitn(2, separator).collect::<Vec<_>>();
        if parts.len() != 2 {
            panic!("{readme} has been modified in the Results section!");
        }
        parts[0]
    };
    let table = get_results_table();
    let new_readme_contents = String::from(readme_init) + separator + &table;
    std::fs::write(readme, new_readme_contents).unwrap();
}
