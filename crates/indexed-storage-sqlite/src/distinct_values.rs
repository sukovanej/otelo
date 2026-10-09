pub const MAX_EXACTLY_COUNTED_VALUES: usize = 64;

const SKETCH_REGISTER_BITS: u32 = 6;
const SKETCH_REGISTER_COUNT: usize = 1 << SKETCH_REGISTER_BITS;
// From the HyperLogLog paper of Flajolet and others, for 64 registers.
const SKETCH_BIAS_CORRECTION: f64 = 0.709;

// A HyperLogLog of 64 registers is about 13% off, so a few values are counted exactly.
#[derive(Clone)]
pub struct DistinctValueCounter {
    exact_hashes: Option<Vec<u64>>,
    registers: [u8; SKETCH_REGISTER_COUNT],
}

impl Default for DistinctValueCounter {
    fn default() -> Self {
        Self {
            exact_hashes: Some(Vec::new()),
            registers: [0; SKETCH_REGISTER_COUNT],
        }
    }
}

impl DistinctValueCounter {
    pub fn add_value_hash(&mut self, value_hash: u64) {
        if let Some(exact_hashes) = &mut self.exact_hashes
            && !exact_hashes.contains(&value_hash)
        {
            if exact_hashes.len() == MAX_EXACTLY_COUNTED_VALUES {
                self.exact_hashes = None;
            } else {
                exact_hashes.push(value_hash);
            }
        }
        let register_index = usize::try_from(value_hash >> (64 - SKETCH_REGISTER_BITS))
            .expect("six bits fit a usize");
        let remaining_bits = value_hash << SKETCH_REGISTER_BITS;
        let leading_zero_run = u8::try_from(
            remaining_bits
                .leading_zeros()
                .min(64 - SKETCH_REGISTER_BITS)
                + 1,
        )
        .expect("a run of at most 59 fits a u8");
        let register = &mut self.registers[register_index];
        *register = (*register).max(leading_zero_run);
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an estimate"
    )]
    pub fn count_distinct_values(&self) -> u64 {
        if let Some(exact_hashes) = &self.exact_hashes {
            return exact_hashes.len() as u64;
        }
        let register_count = SKETCH_REGISTER_COUNT as f64;
        let inverse_sum: f64 = self
            .registers
            .iter()
            .map(|&register| (-f64::from(register)).exp2())
            .sum();
        let raw_estimate = SKETCH_BIAS_CORRECTION * register_count * register_count / inverse_sum;
        let empty_registers = self.registers.iter().fold(0_usize, |count, &register| {
            count + usize::from(register == 0)
        });
        let estimate = if raw_estimate <= 2.5 * register_count && empty_registers > 0 {
            register_count * (register_count / empty_registers as f64).ln()
        } else {
            raw_estimate
        };
        // The exact count passed MAX_EXACTLY_COUNTED_VALUES, so the estimate does too.
        (estimate.round() as u64).max(MAX_EXACTLY_COUNTED_VALUES as u64 + 1)
    }
}
