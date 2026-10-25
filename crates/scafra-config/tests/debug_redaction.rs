use scafra_config::{ConfigLoader, Properties};

const OVERRIDES: [(&str, &str); 3] = [
    ("security.bearer.token", "synthetic-bearer-secret-marker"),
    ("database.password", "synthetic-database-secret-marker"),
    (
        "domain.synthetic-sensitive-key-marker",
        "synthetic-arbitrary-value-marker",
    ),
];

fn loader_with_secrets() -> ConfigLoader {
    OVERRIDES
        .into_iter()
        .fold(ConfigLoader::new(), |loader, (key, value)| {
            loader.override_value(key, value)
        })
}

fn assert_redacted(output: &str) {
    assert!(output.contains("ConfigLoader"));
    assert!(output.contains("override_count: 3"));
    for (key, value) in OVERRIDES {
        assert!(!output.contains(key), "override key leaked in Debug");
        assert!(!output.contains(value), "override value leaked in Debug");
    }
}

#[test]
fn loader_debug_redacts_all_override_keys_and_values() {
    let loader = loader_with_secrets();
    for loader in [&loader, &loader.clone()] {
        assert_redacted(&format!("{loader:?}"));
        assert_redacted(&format!("{loader:#?}"));
    }
}

#[test]
fn properties_debug_preserves_loader_redaction() {
    let properties = Properties::<()>::with_loader(loader_with_secrets());
    assert_redacted(&format!("{properties:?}"));
    assert_redacted(&format!("{properties:#?}"));
}
