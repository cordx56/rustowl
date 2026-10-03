#!/bin/sh

# Run the Neovim test suite defined in nvim-tests/, using mini.test.
# Works from any directory: the suite path is resolved relative to the repo.

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname "$0")" && pwd)"

# shellcheck source=scripts/lib/common.sh
. "$SCRIPT_DIR/lib/common.sh"

cd "$SCRIPT_DIR/.." || exit 1

require_cmd nvim "Install Neovim to run these tests (https://neovim.io)."

printf '%b\n' "${BLUE}${BOLD}================= Rustowl Test Suite =================${NC}"
echo ""

# Capture the output of the test run
output=$(nvim --headless --noplugin -u ./nvim-tests/minimal_init.lua \
	-c "lua MiniTest.run()" \
	-c "qa" 2>&1)
nvim_exit_code=$?

# Print the output
printf '%s\n' "$output"

echo ""
printf '%b\n' "${BLUE}${BOLD}================= Rustowl Test Summary =================${NC}"
echo ""

if printf '%s\n' "$output" | grep -q "Fails (0) and Notes (0)" && [ "$nvim_exit_code" -eq 0 ]; then
	printf '%b\n\n' "${GREEN}✅ ALL TESTS PASSED${NC}"
	exit 0
fi

printf '%b\n\n' "${RED}❌ SOME TESTS FAILED${NC}"
exit 1
