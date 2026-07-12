#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_select_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxsql::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _warning) = run(
        "get the names and email addresses of all active users",
        None,
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.sql.is_empty(), "sql must not be empty");
    assert!(
        out.sql.to_lowercase().contains("select"),
        "expected a SELECT statement, got: {}",
        out.sql
    );
    assert!(!out.mutating, "SELECT must not be mutating");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_mutating_statement_flagged() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxsql::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _warning) = run(
        "delete all rows from the sessions table that are older than 30 days",
        None,
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.sql.is_empty(), "sql must not be empty");
    assert!(
        out.mutating,
        "DELETE statement must be flagged as mutating; sql={}",
        out.sql
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_schema_hint_influences_query() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxsql::run::run;

    let schema = r#"
CREATE TABLE products (
    id INT PRIMARY KEY,
    sku VARCHAR(64) NOT NULL,
    price DECIMAL(10,2),
    category_id INT
);
CREATE TABLE categories (id INT PRIMARY KEY, name VARCHAR(128));
"#;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _warning) = run(
        "list all products with their category names sorted by price descending",
        Some(schema),
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.sql.is_empty());
    let lower = out.sql.to_lowercase();
    assert!(
        lower.contains("products") && lower.contains("categories"),
        "SQL should reference both tables from the schema; got: {}",
        out.sql
    );
    assert!(!out.mutating, "a join-select must not be mutating");
}
