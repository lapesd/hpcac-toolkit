use rand::distr::Alphanumeric;
use rand::{Rng, rng};

pub fn generate_id() -> String {
    rng()
        .sample_iter(Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}
