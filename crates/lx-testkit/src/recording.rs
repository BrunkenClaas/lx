#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};

use lx_llm::{LlmClient, LlmError, Request, Response};

use crate::mock::CapturedRequest;

/// A recording LLM client for eval tests.
///
/// Wraps a real `LlmClient`, forwards every call to it, and records both the
/// request and the response. Use this in `#[ignore = "eval: ..."]` tests that
/// hit the actual provider.
pub struct RecordingLlmClient {
    inner: Box<dyn LlmClient>,
    pub calls: Arc<Mutex<Vec<(CapturedRequest, String)>>>,
}

impl RecordingLlmClient {
    pub fn new(inner: Box<dyn LlmClient>) -> Self {
        RecordingLlmClient {
            inner,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Return the user message from the most recent call.
    ///
    /// # Panics
    /// Panics if no calls have been made yet.
    pub fn last_user_message(&self) -> String {
        self.calls
            .lock()
            .unwrap()
            .last()
            .expect("no LLM calls were made")
            .0
            .user
            .clone()
    }

    /// Return the response text from the most recent call.
    ///
    /// # Panics
    /// Panics if no calls have been made yet.
    pub fn last_response(&self) -> String {
        self.calls
            .lock()
            .unwrap()
            .last()
            .expect("no LLM calls were made")
            .1
            .clone()
    }
}

impl LlmClient for RecordingLlmClient {
    fn complete(&self, req: &Request<'_>) -> Result<Response, LlmError> {
        let resp = self.inner.complete(req)?;
        self.calls.lock().unwrap().push((
            CapturedRequest {
                system: req.system.to_string(),
                user: req.user.to_string(),
                max_tokens: req.max_tokens,
                temperature: req.temperature,
                image: req.image.clone(),
            },
            resp.content.clone(),
        ));
        Ok(resp)
    }
}
