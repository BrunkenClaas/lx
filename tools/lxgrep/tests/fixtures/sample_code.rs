use std::io;

/// Connects to the database at the given host/port.
pub fn connect(host: &str, port: u16) -> Result<Connection, io::Error> {
    Connection::new(host, port)
}

/// Handles incoming errors and logs them.
pub fn handle_error(err: &dyn std::error::Error) {
    eprintln!("error: {err}");
}

/// Adds two integers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Multiplies two integers.
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

fn main() {
    match connect("localhost", 5432) {
        Ok(conn) => {
            println!("connected: {conn:?}");
        }
        Err(e) => {
            handle_error(&e);
            std::process::exit(1);
        }
    }
}
