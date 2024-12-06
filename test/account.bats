load test_helper/bats-support/load.bash
load test_helper/bats-assert/load.bash

setup() {
  dfx stop
  dfx start --clean --background
}

# executed after each test
teardown() {
  dfx stop
}

@test "Can say hello" {
  dfx deploy
  run dfx canister call account hello
  assert_output '("hello")'
}

@test "Include signee: invalid principal" {
  dfx deploy
  run dfx canister call account include_signee invalid_principal
  assert_output --partial 'Is it a valid principal?'
}

@test "Include signee: valid principal" {
  dfx deploy
  run dfx canister call account include_signee un4fu-tqaaa-aaaab-qadjq-cai
  assert_success
}



