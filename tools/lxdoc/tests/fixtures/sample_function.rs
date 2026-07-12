pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn multiply(x: i64, y: i64) -> i64 {
    x * y
}

pub struct Calculator {
    history: Vec<(String, i64, i64, i64)>,
}

impl Calculator {
    pub fn new() -> Self {
        Calculator { history: Vec::new() }
    }

    pub fn compute(&mut self, op: &str, a: i64, b: i64) -> Result<i64, String> {
        let result = match op {
            "add" => add(a, b),
            "mul" => multiply(a, b),
            other => return Err(format!("Unknown operation: {other}")),
        };
        self.history.push((op.to_string(), a, b, result));
        Ok(result)
    }
}
