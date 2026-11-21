//! Which step a virtual user sends next and how long it thinks after it (RB-096-07): in order,
//! or at random by weight; a think time drawn from its range. Each user draws from its own
//! small generator (xorshift, seeded by its number), so a run needs no lock to choose and a
//! test can repeat a draw.

use std::time::Duration;

use super::scenario::{Order, Think};

/// A user's choices.
#[derive(Clone, Debug)]
pub struct Picker {
    order: Order,
    weights: Vec<u64>,
    total: u64,
    at: usize,
    state: u64,
}

impl Picker {
    /// The choices over steps of `weights` (each at least one), drawn from `seed`.
    #[must_use]
    pub fn new(order: Order, weights: &[u32], seed: u64) -> Self {
        let weights: Vec<u64> = weights.iter().map(|w| u64::from((*w).max(1))).collect();
        Self {
            order,
            total: weights.iter().sum::<u64>().max(1),
            weights,
            at: 0,
            // Never zero: xorshift stays at zero. The odd constant spreads near seeds apart.
            state: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
        }
    }

    fn draw(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// The next step's index.
    pub fn next_step(&mut self) -> usize {
        let count = self.weights.len().max(1);
        match self.order {
            Order::Sequence => {
                let step = self.at % count;
                self.at = (step + 1) % count;
                step
            }
            Order::Weighted => {
                let mut ticket = self.draw() % self.total;
                for (index, weight) in self.weights.iter().enumerate() {
                    if ticket < *weight {
                        return index;
                    }
                    ticket -= weight;
                }
                count - 1
            }
        }
    }

    /// A pause from `think`'s range.
    pub fn think(&mut self, think: Think) -> Duration {
        let span = think.max.saturating_sub(think.min);
        let extra = if span == 0 {
            0
        } else {
            self.draw() % (span + 1)
        };
        Duration::from_millis(think.min.saturating_add(extra))
    }
}

#[cfg(test)]
#[path = "pick_tests.rs"]
mod tests;
