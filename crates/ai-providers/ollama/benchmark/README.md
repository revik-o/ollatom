# Ollama real benchmark

The benchmark is opt-in because it downloads a model and creates a project next to the release executable. Run it with:

```sh
./run-real.sh
```

Set `OLLAMA_BENCHMARK_MODEL` to use a different installed model. The default is `gemma4:e2b`.

The launcher builds the release executable, starts a native Ollama daemon only when one is not already available, pulls the selected model, runs all six stages, and stops only the daemon it started. Model data is kept in Ollama's normal local model store.

The Rust benchmark accesses the model only through `llm::LlmRuntime`. Stages 1–2 discover the model, Stage 3 requires the agent to print Rust and Cargo versions from sandboxed `mise exec` commands, and Stages 4–6 create and extend a React project. It creates `ollama-react-test`, writes `stage-1.log` through `stage-6.log`, and leaves the generated project, ownership marker, build output, and logs beside the executable. Existing projects are replaced only when the ownership marker belongs to this benchmark.

Agent runs use the generic file and command tools registered by `llm::register_basic_tools`, with full create, read, modify, rename, and delete access within `ollama-react-test`. The command policy permits `mise exec --` for Rust version checks and Node/npm/npx commands. The command sandbox exposes the active Node installation, Rustup toolchain, and Rust executable directory as read-only runtime paths. Agent commands may use network access for package installation; independent verification uses the same mise-managed Node toolchain with network access denied. After five command attempts fail specifically because the Node toolchain is inaccessible, that stage is logged as skipped and later stages continue. The benchmark contains no provider-specific tool implementations. Direct file tools reject absolute paths and parent traversal, and recursive listings omit `node_modules` and `dist`. System and user prompts are compiled into the executable from the `prompts` directory.
