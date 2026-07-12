use std::path::PathBuf;
use std::process::Command;

/// Wrapper around a built lx-binary for system-level tests.
pub struct BinaryUnderTest {
    path: PathBuf,
}

impl BinaryUnderTest {
    /// Locates the binary in the cargo build-output directory.
    /// Looks for `target/debug/<name>` (Linux) or `target/debug/<name>.exe` (Windows).
    pub fn for_tool(name: &str) -> Self {
        Self::locate(name, "debug")
    }

    /// Locates the binary in `target/release` instead of `target/debug`.
    /// Used by the extended acceptance harness, which exercises the optimized
    /// binaries users actually run (matching `acceptance/run.sh`). Building all
    /// 72 tools in release once is far cheaper than maintaining a debug build of
    /// the whole suite just for the harness.
    pub fn for_tool_release(name: &str) -> Self {
        Self::locate(name, "release")
    }

    fn locate(name: &str, profile: &str) -> Self {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // Navigate out of <category>/<crate>/ (e.g. tools/<tool>/ or
        // crates/<crate>/) to the workspace root.
        p.pop();
        p.pop();
        p.push("target");
        p.push(profile);
        p.push(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_string()
        });
        let build_hint = if profile == "release" {
            format!("cargo build --release -p {name}")
        } else {
            format!("cargo build -p {name}")
        };
        assert!(
            p.exists(),
            "Binary not found: {p:?}\nRun `{build_hint}` first."
        );
        Self { path: p }
    }

    /// Runs the binary with the given args and returns stdout/stderr/exit_code.
    pub fn run(&self, args: &[&str]) -> BinaryOutput {
        let out = Command::new(&self.path)
            .args(args)
            .output()
            .expect("failed to run binary");
        BinaryOutput {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            exit_code: out.status.code().unwrap_or(-1),
        }
    }

    /// Runs the binary with stdin input.
    pub fn run_with_stdin(&self, args: &[&str], input: &str) -> BinaryOutput {
        use std::io::Write;
        let mut child = Command::new(&self.path)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn binary");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        BinaryOutput {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            exit_code: out.status.code().unwrap_or(-1),
        }
    }
}

pub struct BinaryOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl BinaryOutput {
    pub fn assert_success(&self) {
        assert_eq!(
            self.exit_code, 0,
            "expected exit 0, got {}\nstderr: {}",
            self.exit_code, self.stderr
        );
    }

    pub fn assert_exit(&self, code: i32) {
        assert_eq!(
            self.exit_code, code,
            "expected exit {code}, got {}\nstderr: {}",
            self.exit_code, self.stderr
        );
    }

    /// Checks pipe-safety: stdout must not contain comment lines or explanations.
    pub fn assert_stdout_pipe_safe(&self) {
        let stdout = self.stdout.trim();
        assert!(
            !stdout.is_empty(),
            "stdout must not be empty for a successful call"
        );
        for line in stdout.lines() {
            assert!(
                !line.starts_with('#'),
                "comment line on stdout (pipe unsafe): {line:?}"
            );
            assert!(
                !line.starts_with("//"),
                "comment line on stdout (pipe unsafe): {line:?}"
            );
        }
    }

    /// Checks that stdout is valid JSON.
    pub fn assert_stdout_valid_json(&self) {
        serde_json::from_str::<serde_json::Value>(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}\n{}", self.stdout));
    }

    /// Checks that stdout JSON contains the expected field.
    pub fn assert_json_field(&self, field: &str) {
        let v: serde_json::Value = serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not valid JSON: {e}"));
        assert!(
            v.get(field).is_some(),
            "JSON field {field:?} missing in: {}",
            self.stdout
        );
    }
}
