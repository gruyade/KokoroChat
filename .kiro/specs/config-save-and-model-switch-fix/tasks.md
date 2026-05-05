# Implementation Plan

- [x] 1. Write bug condition exploration test
  - **Property 1: Bug Condition** - Config Non-Empty Values Overwritten by Env Vars
  - **CRITICAL**: This test MUST FAIL on unfixed code - failure confirms the bug exists
  - **DO NOT attempt to fix the test or the code when it fails**
  - **NOTE**: This test encodes the expected behavior - it will validate the fix when it passes after implementation
  - **GOAL**: Surface counterexamples that demonstrate the bug exists
  - **Scoped PBT Approach**: Generate random non-empty ModelSettings (base_url, model, api_key) and corresponding env vars with different non-empty values. Assert that after `apply_env_fallback`, the config values are preserved (not overwritten by env vars)
  - Test file: `src-tauri/src/config/property_tests.rs` (add new test module or extend existing)
  - Use `proptest` crate to generate:
    - Random non-empty strings for `settings.base_url`, `settings.model`
    - Random `Some(non_empty_string)` for `settings.api_key`
    - Random non-empty env var values that differ from config values
  - Set env vars using `std::env::set_var` before calling `apply_env_for_purpose`
  - Assert: `settings.base_url == original_base_url` (not env value)
  - Assert: `settings.model == original_model` (not env value)
  - Assert: `settings.api_key == original_api_key` (not env value)
  - Bug Condition from design: `configValue(field) != "" AND envVarSet(envVarName) AND envVarValue != configValue(field)`
  - Expected Behavior from design: config.json non-empty values SHALL NOT be overwritten
  - Run test on UNFIXED code
  - **EXPECTED OUTCOME**: Test FAILS (env vars overwrite non-empty config values, confirming the bug)
  - Document counterexamples found (e.g., "base_url='http://localhost:1234' overwritten by env 'http://other:5678'")
  - Mark task complete when test is written, run, and failure is documented
  - _Requirements: 1.1, 2.1_

- [x] 2. Write preservation property tests (BEFORE implementing fix)
  - **Property 2: Preservation** - Empty Config Values Receive Env Fallback
  - **IMPORTANT**: Follow observation-first methodology
  - Observe on UNFIXED code: `apply_env_for_purpose` with empty `base_url=""` and env `PREFIX_BASE_URL=X` → result is `X`
  - Observe on UNFIXED code: `apply_env_for_purpose` with empty `model=""` and env `PREFIX_MODEL=Y` → result is `Y`
  - Observe on UNFIXED code: `apply_env_for_purpose` with `api_key=None` and env `PREFIX_API_KEY=Z` → result is `Some(Z)`
  - Write property-based test using `proptest`:
    - Generate random non-empty env var values
    - Set config fields to empty string / None
    - Assert: after `apply_env_fallback`, empty fields receive env var values
    - Assert: `settings.base_url == env_base_url` when original was empty
    - Assert: `settings.model == env_model` when original was empty
    - Assert: `settings.api_key == Some(env_api_key)` when original was None
  - Preservation Requirements from design: "config.jsonのフィールドが空文字列の場合、環境変数の値がフォールバックとして適用される"
  - Run tests on UNFIXED code
  - **EXPECTED OUTCOME**: Tests PASS (empty values already receive env fallback in unfixed code)
  - Mark task complete when tests are written, run, and passing on unfixed code
  - _Requirements: 3.1, 3.2_

- [x] 3. Fix for config persistence and LLM request serialization

  - [x] 3.1 Fix `apply_env_for_purpose` to only apply env vars as fallback
    - Change condition: only apply `{PREFIX}_BASE_URL` when `settings.base_url` is empty
    - Change condition: only apply `{PREFIX}_MODEL` when `settings.model` is empty
    - Change condition: only apply `{PREFIX}_API_KEY` when `settings.api_key` is None
    - Change log messages from "env override" to "env fallback"
    - File: `src-tauri/src/config/model_config.rs`, function `apply_env_for_purpose`
    - _Bug_Condition: isBugCondition(input) where configValue(field) != "" AND envVarSet(envVarName)_
    - _Expected_Behavior: non-empty config values preserved, env vars only applied to empty fields_
    - _Preservation: empty fields still receive env var fallback_
    - _Requirements: 1.1, 2.1, 3.1, 3.2_

  - [x] 3.2 Add `llm_lock` to `AppState` and initialize
    - Add `llm_lock: Arc<tokio::sync::Mutex<()>>` field to `AppState` in `src-tauri/src/state.rs`
    - Initialize `llm_lock: Arc::new(tokio::sync::Mutex::new(()))` in `src-tauri/src/lib.rs` during `AppState` construction
    - _Bug_Condition: anotherLlmRequestInProgress() == true_
    - _Expected_Behavior: at most one LLM request in-flight at any time_
    - _Requirements: 1.2, 1.3, 2.2, 2.3_

  - [x] 3.3 Acquire `llm_lock` in chat engine before LLM call
    - Pass `llm_lock` reference to `DefaultChatEngine` (add field or pass via method)
    - Acquire lock before `chat_stream` / LLM call in `src-tauri/src/chat/engine.rs`
    - Hold lock until streaming completes
    - _Requirements: 2.2, 2.3_

  - [x] 3.4 Acquire `llm_lock` in memory manager before LLM call
    - Pass `llm_lock` reference to `DefaultMemoryManager` (add field)
    - Acquire lock before `self.llm_client.chat()` in `check_and_compress`
    - File: `src-tauri/src/memory/manager.rs`
    - _Requirements: 2.2, 3.3_

  - [x] 3.5 Acquire `llm_lock` in thought engine before LLM call
    - Pass `llm_lock` reference to `DefaultThoughtEngine` (add field)
    - Acquire lock before `llm_client.chat()` in the background loop
    - File: `src-tauri/src/thought/engine.rs`
    - _Requirements: 2.3, 3.4_

  - [x] 3.6 Serialize memory compression with chat in `send_message`
    - Remove `tokio::spawn` for memory compression in `src-tauri/src/commands/chat.rs`
    - Instead, spawn with `llm_lock` passed so compression waits for lock release
    - Or: keep spawn but ensure `check_and_compress` acquires `llm_lock` internally (already done in 3.4)
    - Verify chat streaming releases lock before memory compression acquires it
    - _Requirements: 2.2, 3.3_

  - [x] 3.7 Verify bug condition exploration test now passes
    - **Property 1: Expected Behavior** - Config Non-Empty Values Preserved on Load
    - **IMPORTANT**: Re-run the SAME test from task 1 - do NOT write a new test
    - The test from task 1 encodes the expected behavior
    - When this test passes, it confirms the expected behavior is satisfied
    - Run bug condition exploration test from step 1
    - **EXPECTED OUTCOME**: Test PASSES (confirms config values are no longer overwritten)
    - _Requirements: 2.1_

  - [x] 3.8 Verify preservation tests still pass
    - **Property 2: Preservation** - Empty Config Values Receive Env Fallback
    - **IMPORTANT**: Re-run the SAME tests from task 2 - do NOT write new tests
    - Run preservation property tests from step 2
    - **EXPECTED OUTCOME**: Tests PASS (confirms empty value fallback still works)
    - Confirm all tests still pass after fix (no regressions)
    - _Requirements: 3.1, 3.2_

- [x] 4. Checkpoint - Ensure all tests pass
  - Run full test suite: `cargo test` in `src-tauri/`
  - Ensure all property-based tests pass (bug condition + preservation)
  - Ensure existing tests in `config/tests.rs`, `chat/tests.rs`, `memory/tests.rs`, `thought/tests.rs` still pass
  - Ask the user if questions arise
