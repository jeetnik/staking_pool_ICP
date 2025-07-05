# `staking_pool`

Welcome to your new `staking_pool` project and to the Internet Computer development community. By default, creating a new project adds this README and some template files to your project directory. You can edit these template files to customize your project and to include your own code to speed up the development cycle.

To get started, you might want to explore the project directory structure and the default configuration file. Working with this project in your development environment will not affect any production deployment or identity tokens.

To learn more before you start working with `staking_pool`, see the following documentation available online:

- [Quick Start](https://internetcomputer.org/docs/current/developer-docs/setup/deploy-locally)
- [SDK Developer Tools](https://internetcomputer.org/docs/current/developer-docs/setup/install)
- [Rust Canister Development Guide](https://internetcomputer.org/docs/current/developer-docs/backend/rust/)
- [ic-cdk](https://docs.rs/ic-cdk)
- [ic-cdk-macros](https://docs.rs/ic-cdk-macros)
- [Candid Introduction](https://internetcomputer.org/docs/current/developer-docs/backend/candid/)

If you want to start working on your project right away, you might want to try the following commands:

```bash
cd staking_pool/
dfx help
dfx canister --help
```


## Running the project locally

If you want to test your project locally, you can use the following commands:

```bash
# Build the canister
cargo build --target wasm32-unknown-unknown --release  

 # Run tests
cargo test -p staking_pool_backend --test integration_tests
```

The output will be shown like this
```bash

saijeetnikam@qwerty-2 staking_pool % cargo test -p staking_pool_backend --test integration_tests      

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.54s
     Running tests/integration_tests.rs (target/debug/deps/integration_tests-3a591f9b32566af9)

running 25 tests
2025-07-05T13:52:53.424617Z  INFO pocket_ic_server: The PocketIC server is listening on port 57050
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_edge_case_empty_operations ... ok
test test_cross_user_interference ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_complete_user_journey ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_cleanup_expired_deposits ... ok
test test_create_deposit_intention ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_confirm_deposit_without_funds ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_different_lock_periods ... ok
test test_cleanup_performance ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_expired_deposit_intention ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_get_reward_address ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_input_boundary_conditions ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_invalid_operations ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_multiple_deposits_same_user ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_query_functions_edge_cases ... ok
test test_multiple_users_operations ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_repeated_cleanup_calls ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_reward_address_consistency ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_reward_distribution_edge_cases ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_reward_pool_empty ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_slash_pool_comprehensive ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_slash_pool_empty ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_slash_pool_receiver_scenarios ... ok
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
2021-05-06 19:17:10.000000003 UTC: [Canister lxzze-o7777-77777-aaaaa-cai] Staking pool canister initialized
test test_time_manipulation_scenarios ... ok
test test_unauthorized_confirm_deposit ... ok
test test_stress_subaccount_generation ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.57s






```


Snapshot of the the test


<img width="1000" alt="Screenshot 2025-07-05 at 19 42 18" src="https://github.com/user-attachments/assets/71556b85-55fb-41db-978c-c243b48b7240" />





