// Example Rust file with various TODO-style comments.

/// A simple calculator.
pub struct Calculator {
    // TODO: add support for floating-point arithmetic
    value: i64,
}

impl Calculator {
    pub fn new() -> Self {
        // FIXME: should return Result in case of overflow
        Self { value: 0 }
    }

    pub fn add(&mut self, n: i64) -> i64 {
        // HACK: using wrapping_add to avoid panic; should return Err
        self.value = self.value.wrapping_add(n);
        self.value
    }

    pub fn divide(&mut self, n: i64) -> Option<i64> {
        if n == 0 {
            // XXX: log the error somewhere
            return None;
        }
        self.value /= n;
        Some(self.value)
    }

    pub fn reset(&mut self) {
        self.value = 0;
        // NOTE: this does not persist the reset to disk
    }
}

// TODO(alice): write unit tests for Calculator
// OPTIMIZE: consider using u128 for large numbers
