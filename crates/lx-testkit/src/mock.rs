#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};

use lx_llm::{ImageData, LlmClient, LlmError, Request, Response};

/// A single captured LLM request for inspection in tests.
#[derive(Debug, Clone)]
pub struct CapturedRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub image: Option<ImageData>,
}

/// A mock LLM client for use in integration tests.
///
/// Returns a fixed response for every call without any network access.
/// Captures every request so tests can assert on what was sent.
pub struct MockLlmClient {
    response: String,
    /// Return `LlmError::Network` when the call count reaches this value (1-based).
    error_on_call: Option<usize>,
    pub calls: Arc<Mutex<Vec<CapturedRequest>>>,
}

impl MockLlmClient {
    /// Always return `response` for every call.
    pub fn returning(response: &str) -> Self {
        MockLlmClient {
            response: response.to_string(),
            error_on_call: None,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Return a transient `LlmError::Network` on the Nth call (1-based).
    ///
    /// Useful for testing retry logic in tools.
    pub fn with_transient_error_on(call_n: usize) -> Self {
        MockLlmClient {
            response: String::new(),
            error_on_call: Some(call_n),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Return the most recent captured request.
    ///
    /// # Panics
    /// Panics if no calls have been made yet.
    pub fn last_request(&self) -> CapturedRequest {
        self.calls
            .lock()
            .unwrap()
            .last()
            .expect("no LLM calls were made")
            .clone()
    }

    /// Number of calls made so far.
    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

impl LlmClient for MockLlmClient {
    fn complete(&self, req: &Request<'_>) -> Result<Response, LlmError> {
        let mut calls = self.calls.lock().unwrap();
        calls.push(CapturedRequest {
            system: req.system.to_string(),
            user: req.user.to_string(),
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            image: req.image.clone(),
        });
        let call_number = calls.len(); // 1-based after push
        drop(calls);

        if self.error_on_call == Some(call_number) {
            return Err(LlmError::Network(
                "injected transient error for test".to_string(),
            ));
        }

        Ok(Response {
            content: self.response.clone(),
            prompt_tokens: Some(10),
            completion_tokens: Some(20),
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_req() -> MockLlmClient {
        MockLlmClient::returning(r#"{"ok":true}"#)
    }

    fn call(client: &MockLlmClient) -> Result<Response, LlmError> {
        client.complete(&Request {
            system: "sys",
            user: "user",
            max_tokens: 256,
            temperature: 0.0,
            image: None,
        })
    }

    #[test]
    fn returning_gives_fixed_response() {
        let c = make_req();
        let r = call(&c).unwrap();
        assert_eq!(r.content, r#"{"ok":true}"#);
        assert_eq!(r.prompt_tokens, Some(10));
        assert_eq!(r.completion_tokens, Some(20));
    }

    #[test]
    fn call_count_increments() {
        let c = make_req();
        call(&c).unwrap();
        call(&c).unwrap();
        assert_eq!(c.call_count(), 2);
    }

    #[test]
    fn last_request_captures_fields() {
        let c = make_req();
        c.complete(&Request {
            system: "s",
            user: "u",
            max_tokens: 128,
            temperature: 0.0,
            image: None,
        })
        .unwrap();
        let r = c.last_request();
        assert_eq!(r.system, "s");
        assert_eq!(r.user, "u");
        assert_eq!(r.max_tokens, 128);
        assert_eq!(r.temperature, 0.0);
    }

    #[test]
    #[should_panic(expected = "no LLM calls were made")]
    fn last_request_panics_when_empty() {
        let c = make_req();
        c.last_request();
    }

    #[test]
    fn transient_error_on_nth_call() {
        let c = MockLlmClient::with_transient_error_on(2);
        assert!(call(&c).is_ok(), "call 1 should succeed — got error");
        let result = call(&c);
        assert!(
            matches!(result, Err(LlmError::Network(_))),
            "call 2 should error"
        );
        // call 3 would return empty string (no response set), but no error
        let r = call(&c);
        assert!(r.is_ok(), "call 3 should succeed again");
    }
}
