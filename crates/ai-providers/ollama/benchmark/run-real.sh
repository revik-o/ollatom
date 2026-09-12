#!/usr/bin/env bash
set -u

benchmark_model_identifier="${OLLAMA_BENCHMARK_MODEL:-gemma4:e2b}"
script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
release_executable="${script_directory}/../../../../target/release/ollama-benchmark"
release_executable_directory="$(dirname -- "${release_executable}")"
server_log_path="${release_executable_directory}/ollama-server.log"
server_process_identifier=""
launcher_started_server=0

terminate_started_server_and_exit() {
  benchmark_exit_status=$?
  if [[ "${launcher_started_server}" == "1" && -n "${server_process_identifier}" ]]; then
    kill "${server_process_identifier}" 2>/dev/null || true
    wait "${server_process_identifier}" 2>/dev/null || true
  fi
  exit "${benchmark_exit_status}"
}
trap terminate_started_server_and_exit EXIT

mise exec -- cargo build --release --manifest-path "${script_directory}/Cargo.toml" || exit $?

if ! command -v ollama >/dev/null 2>&1; then
  echo "ollama CLI is required for the real benchmark" >&2
  exit 1
fi

if ! ollama list >/dev/null 2>&1; then
  ollama serve >"${server_log_path}" 2>&1 &
  server_process_identifier="$!"
  launcher_started_server=1
  remaining_readiness_attempts=60
  while (( remaining_readiness_attempts > 0 )); do
    if ollama list >/dev/null 2>&1; then
      break
    fi
    if ! kill -0 "${server_process_identifier}" 2>/dev/null; then
      echo "ollama serve stopped before becoming ready" >&2
      exit 1
    fi
    remaining_readiness_attempts=$((remaining_readiness_attempts - 1))
    sleep 1
  done
fi

if ! ollama list >/dev/null 2>&1; then
  echo "ollama daemon did not become ready" >&2
  exit 1
fi

ollama pull "${benchmark_model_identifier}" || exit $?
mise exec -- "${release_executable}"
