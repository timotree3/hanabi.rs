# Simulations of Hanabi strategies

Hanabi is a cooperative card game of incomplete information.
Despite relatively [simple rules](https://boardgamegeek.com/article/10670613#10670613),
the space of Hanabi strategies is quite interesting.

This repository provides a framework for implementing Hanabi strategies.
It also explores some implementations, based on ideas from
[this paper](https://d0474d97-a-62cb3a1a-s-sites.googlegroups.com/site/rmgpgrwc/research-papers/Hanabi_final.pdf).

In particular, it contains a variant of their "information strategy", with some improvements.
This strategy achieves the best results I am aware of (see below), for n > 3.

Please contact me if:
- You know of other interesting/good strategy ideas!
- Have questions about the framework

Some similar projects I am aware of:
- https://github.com/rjtobin/HanSim (written for the paper mentioned above)
- https://github.com/Quuxplusone/Hanabi

## Setup

Install rust/rustc and cargo, and change the options in main.rs appropriately.

`cargo run -- -h`

```
Usage: target/debug/rust_hanabi [options]

Options:
    -l, --loglevel LOGLEVEL
                        Log level, one of 'trace', 'debug', 'info', 'warn', and 'error'
    -n, --ntrials NTRIALS
                        Number of games to simulate
    -t, --nthreads NTHREADS
                        Number of threads to use for simulation
    -s, --seed SEED     Seed for PRNG
    -p, --nplayers NPLAYERS
                        Number of players
    -h, --help          Print this help menu
```

For example,

`cargo run -- -n 10000 -s 0 -t 2 -p 3`

## Results (sparsely updated)

Currently, on seeds 0-9999, we have:
(info strategy is only ran on 1000 seeds)

          |   2p    |   3p    |   4p    |   5p    |
----------|---------|---------|---------|---------|
cheating  | 24.8600 | 24.9781 | 24.9715 | 24.9583 |
info      | 14.981  | 22.526  | 24.516  | 24.742  |

