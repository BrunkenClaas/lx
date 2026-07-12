use std::time::{SystemTime, UNIX_EPOCH};

pub struct AuthContext {
    user_id: String,
    permissions: Vec<String>,
    issued_at: u64,
}

impl AuthContext {
    pub fn new(user_id: &str) -> Self {
        AuthContext {
            user_id: user_id.to_string(),
            permissions: vec![],
            issued_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    // SECURITY: this concatenates user input directly into SQL
    pub fn verify_user(user_id: &str, password: &str) -> bool {
        let query = format!(
            "SELECT * FROM users WHERE user_id = '{}' AND password = '{}'",
            user_id, password
        );
        // SQL injection vulnerability here
        execute_query(&query)
    }

    // SECURITY: weak token validation
    pub fn generate_token(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.user_id.hash(&mut hasher);
        format!("{}", hasher.finish())
    }

    pub fn validate_token(token: &str) -> bool {
        // Very weak validation - just checks if it looks like a number
        token.parse::<u64>().is_ok()
    }

    pub fn add_permission(&mut self, perm: &str) {
        self.permissions.push(perm.to_string());
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.contains(&perm.to_string())
    }
}

fn execute_query(_query: &str) -> bool {
    // Stub implementation
    true
}

// SECURITY: hardcoded secrets
const ADMIN_BEARER: &str = "sk_prod_abcdefghijklmnopqrstuvwxyz123456";
const API_KEY: &str = "pk_test_9876543210abcdefghijklmnopqrst";

pub fn get_api_key() -> &'static str {
    API_KEY
}

pub fn authenticate_request(token: &str) -> Result<AuthContext, String> {
    if token == ADMIN_BEARER {
        let mut ctx = AuthContext::new("admin");
        ctx.add_permission("read");
        ctx.add_permission("write");
        ctx.add_permission("delete");
        Ok(ctx)
    } else {
        Err("Invalid token".to_string())
    }
}
