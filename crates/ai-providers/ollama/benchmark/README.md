# Ollama real benchmark

The benchmark is opt-in because it downloads a model and creates a project next to the release executable. Run it with:

```sh
./run-real.sh
```

Set `OLLAMA_BENCHMARK_MODEL` to use a different installed model. The default is `gemma4:e2b`.

The launcher builds the release executable, starts a native Ollama daemon only when one is not already available, pulls the selected model, runs all five stages, and stops only the daemon it started. Model data is kept in Ollama's normal local model store.

The Rust benchmark accesses the model only through `llm::LlmRuntime`. It creates `ollama-react-test`, writes `stage-1.log` through `stage-5.log`, and leaves the generated project, ownership marker, build output, and logs beside the executable. Existing projects are replaced only when the ownership marker belongs to this benchmark.

Agent runs use the generic file and command tools registered by `llm::register_basic_tools`, with full create, read, modify, rename, and delete access within `ollama-react-test`, plus permission to run npm commands with that project as the working directory. The benchmark contains no provider-specific tool implementations. Direct file tools reject absolute paths and parent traversal, and recursive listings omit `node_modules` and `dist`. System and user prompts are compiled into the executable from the `prompts` directory.
