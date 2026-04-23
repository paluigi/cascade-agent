.PHONY: release check test clippy clean-release sweep size-report

release:
	cargo build --release

check:
	cargo check

test:
	cargo test

clippy:
	cargo clippy

size-report:
	@echo "=== Binary Sizes ==="
	@if [ -f target/release/cascade-agent ]; then \
		echo "Release: $$(du -sh target/release/cascade-agent | cut -f1)"; \
	else \
		echo "Release binary not found. Run 'make release' first."; \
	fi
	@echo ""
	@echo "=== Target Directory ==="
	@du -sh target/ 2>/dev/null || echo "No target directory"
	@du -sh target/debug/ 2>/dev/null || echo "No debug directory"
	@du -sh target/release/ 2>/dev/null || echo "No release directory"

clean-release:
	rm -rf target/release/

sweep:
	cargo sweep --time 30
