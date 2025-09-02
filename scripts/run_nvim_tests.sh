#!/usr/bin/env bash

printf "\033[1;36m\n================= Rustowl Test Suite =================\n\033[0m\n\n"

# Capture the output of the test run
output=$(nvim --headless --noplugin -u ./nvim-tests/minimal_init.lua \
  -c "lua MiniTest.run()" \
  -c "qa" 2>&1)

nvim_exit_code=$?

# Print the output
echo "$output"

echo ""
printf "\033[1;36m\n================= Rustowl Test Summary =================\n\033[0m\n"

# Check for failures in the output
if echo "$output" | grep -q "Fails (0) and Notes (0)" && [ $nvim_exit_code -eq 0 ]; then
  printf "\n\033[1;32m✅ ALL TESTS PASSED\033[0m\n\n"
  exit 0
else
  printf "\n\033[1;31m❌ SOME TESTS FAILED\033[0m\n\n"
  exit 1
fi
#!/bin/sh

printf "\033[1;36m\n================= Rustowl Test Suite =================\n\033[0m\n\n"

# Capture the output of the test run
output=$(nvim --headless --noplugin -u ./nvim-tests/minimal_init.lua \
	-c "lua MiniTest.run()" \
	-c "qa" 2>&1)

nvim_exit_code=$?

# Print the output
echo "$output"

echo ""
printf "\033[1;36m\n================= Rustowl Test Summary =================\n\033[0m\n"

# Check for failures in the output
if echo "$output" | grep -q "Fails (0) and Notes (0)" && [ "$nvim_exit_code" -eq 0 ]; then
	printf "\n\033[1;32m✅ ALL TESTS PASSED\033[0m\n\n"
	exit 0
else
	printf "\n\033[1;31m❌ SOME TESTS FAILED\033[0m\n\n"
	exit 1
fi
