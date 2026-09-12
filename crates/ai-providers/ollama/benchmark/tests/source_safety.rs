#[test]
fn benchmark_rust_sources_do_not_bypass_the_llm_abstraction() {
    let rust_sources = [
        include_str!("../src/main.rs"),
        include_str!("../src/lib.rs"),
        include_str!("../src/artifact.rs"),
        include_str!("../src/history.rs"),
        include_str!("../src/logging.rs"),
        include_str!("../src/stages.rs"),
        include_str!("../src/summary.rs"),
        include_str!("../src/verifier.rs"),
    ];
    let forbidden_native_access = [
        "reqwest",
        "api/chat",
        "api/show",
        "api/tags",
        "ollama list",
        "ollama pull",
        "ollama serve",
    ];

    for rust_source in rust_sources {
        for forbidden_text in forbidden_native_access {
            assert!(!rust_source.contains(forbidden_text));
        }
    }
}
