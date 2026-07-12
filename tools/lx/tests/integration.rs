use lx::catalog::{Category, TOOLS};
use lx::render;
use std::collections::HashSet;

#[test]
fn catalog_has_72_tools() {
    assert_eq!(TOOLS.len(), 72, "catalog must have exactly 72 tools");
}

#[test]
fn every_name_starts_with_lx_and_is_nonempty() {
    for e in TOOLS {
        assert!(!e.name.is_empty(), "empty name in catalog");
        assert!(
            e.name.starts_with("lx"),
            "name does not start with 'lx': {}",
            e.name
        );
        assert!(!e.short.is_empty(), "empty short for {}", e.name);
        assert!(!e.purpose.is_empty(), "empty purpose for {}", e.name);
    }
}

#[test]
fn no_duplicate_names() {
    let names: HashSet<&str> = TOOLS.iter().map(|e| e.name).collect();
    assert_eq!(
        names.len(),
        TOOLS.len(),
        "duplicate tool names found in catalog"
    );
}

#[test]
fn catalog_matches_workspace_members() {
    // Workspace members are the ground truth. This embedded list is generated
    // from the Cargo.toml at tools/lx/../../Cargo.toml.
    // If this test fails, update catalog.rs to match the workspace.
    let workspace_toml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml"),
    )
    .expect("could not read workspace Cargo.toml");

    let workspace_tools: HashSet<String> = workspace_toml
        .lines()
        .filter(|l| l.trim().starts_with("\"tools/"))
        .map(|l| {
            l.trim()
                .trim_start_matches('"')
                .trim_start_matches("tools/")
                .trim_end_matches(',')
                .trim_end_matches('"')
                .to_string()
        })
        .filter(|s| s != "lx") // lx itself is the umbrella, not a cataloged tool
        .collect();

    let catalog_names: HashSet<String> = TOOLS.iter().map(|e| e.name.to_string()).collect();

    let missing: Vec<&str> = workspace_tools
        .iter()
        .filter(|n| !catalog_names.contains(n.as_str()))
        .map(String::as_str)
        .collect();
    let extra: Vec<&str> = catalog_names
        .iter()
        .filter(|n| !workspace_tools.contains(n.as_str()))
        .map(String::as_str)
        .collect();

    assert!(
        missing.is_empty(),
        "workspace tools missing from catalog: {:?}",
        missing
    );
    assert!(
        extra.is_empty(),
        "catalog tools not in workspace: {:?}",
        extra
    );
}

#[test]
fn render_grouped_contains_all_category_headings() {
    let all: Vec<&lx::catalog::ToolEntry> = TOOLS.iter().collect();
    let out = render::render_grouped(&all, false, 80);
    for cat in Category::all() {
        assert!(
            out.contains(cat.display_name()),
            "grouped render missing category: {}",
            cat.display_name()
        );
    }
}

#[test]
fn keyword_commit_finds_lxcommit() {
    let hits = render::filter_by_keyword("commit");
    assert!(
        hits.iter().any(|e| e.name == "lxcommit"),
        "lxcommit not found when searching 'commit'"
    );
}

#[test]
fn keyword_no_match_returns_empty() {
    let hits = render::filter_by_keyword("zzz_no_such_tool_99999");
    assert!(hits.is_empty());
}

#[test]
fn cat_filter_code_returns_code_tools_only() {
    let hits = render::filter_by_cat(Some("code"));
    assert!(!hits.is_empty());
    assert!(
        hits.iter().all(|e| e.category == Category::CodeDevelopment),
        "non-code tool in code category results"
    );
}

#[test]
fn cat_filter_none_returns_all() {
    let hits = render::filter_by_cat(None);
    assert_eq!(hits.len(), TOOLS.len());
}

#[test]
fn json_render_is_valid_array_of_72() {
    let all: Vec<&lx::catalog::ToolEntry> = TOOLS.iter().collect();
    let json = render::render_json(&all);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = parsed.as_array().expect("JSON must be array");
    assert_eq!(arr.len(), 72);
    // Every entry must have the expected fields.
    for item in arr {
        assert!(item.get("name").is_some());
        assert!(item.get("category").is_some());
        assert!(item.get("short").is_some());
        assert!(item.get("purpose").is_some());
    }
}

#[test]
fn snapshot_grouped_plain() {
    let all: Vec<&lx::catalog::ToolEntry> = TOOLS.iter().collect();
    let out = render::render_grouped(&all, false, 100);
    insta::assert_snapshot!(out);
}

#[test]
fn snapshot_json() {
    let all: Vec<&lx::catalog::ToolEntry> = TOOLS.iter().collect();
    let json = render::render_json(&all);
    // Only snapshot the first entry to keep the snapshot manageable.
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
    let first = serde_json::to_string_pretty(&parsed[0]).unwrap();
    insta::assert_snapshot!(first);
}
