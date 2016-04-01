# Simulations of Hanabi strategies

Hanabi is a cooperative card game of incomplete information.
Despite relatively [simple rules](https://boardgamegeek.com/article/10670613#10670613),
the space of Hanabi strategies is quite interesting.

This repository provides a framework for implementing Hanabi strategies.
It also explores some implementations, based on ideas from
[this paper](https://d0474d97-a-62cb3a1a-s-sites.googlegroups.com/site/rmgpgrwc/research-papers/Hanabi_final.pdf).

In particular, it contains a variant of their "information strategy", with some improvements.
This strategy achieves the best results I am aware of for n > 2 (see below).

Please contact me if:
- You know of other interesting/good strategy ideas!
- Have questions about the framework or existing strategies

Some similar projects I am aware of:
- https://github.com/rjtobin/HanSim (written for the paper mentioned above)
- https://github.com/Quuxplusone/Hanabi

## Setup

Install rust/rustc and cargo. Then,

`cargo run -- -h`

```
Usage: target/debug/rust_hanabi [options]

Options:
    -l, --loglevel LOGLEVEL
                        Log level, one of 'trace', 'debug', 'info', 'warn',
                        and 'error'
    -n, --ntrials NTRIALS
                        Number of games to simulate (default 1)
    -t, --nthreads NTHREADS
                        Number of threads to use for simulation (default 1)
    -s, --seed SEED     Seed for PRNG (default random)
    -p, --nplayers NPLAYERS
                        Number of players
    -g, --strategy STRATEGY
                        Which strategy to use. One of 'random', 'cheat', and
                        'info'
    -h, --help          Print this help menu
```

For example,

```
cargo run -- -n 10000 -s 0 -p 5 -g cheat
```

Or, if the simulation is slow (as the info strategy is),

```
time cargo run --release -- -n 10000 -o 1000 -s 0 -t 4 -p 5 -g info
```

Or, to see a transcript of a single game:
```
cargo run -- -s 2222 -p 5 -g info -l debug | less
```

## Results

On seeds 0-9999, we have:

          |   2p    |   3p    |   4p    |   5p    |
----------|---------|---------|---------|---------|
cheating  | 24.8600 | 24.9781 | 24.9715 | 24.9583 |
info      | 18.5909 | 24.1655 | 24.7922 | 24.8784 |


To reproduce:
```
n=10000   # number of rounds to simulate
t=4       # number of threads
for strategy in info cheat; do
  for p in $(seq 2 5); do
    time cargo run --release -- -n $n -s 0 -t $t -p $p -g $strategy;
  done
done
```
