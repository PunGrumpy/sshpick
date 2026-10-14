.PHONY: quality fmt toml lint deny test fix setup install

quality: fmt toml lint deny test

fmt:
	cargo fmt --check
toml:
	taplo format --check
lint:
	cargo lint
deny:
	cargo deny --all-features check licenses advisories bans sources
test:
	cargo test --workspace --locked

fix:
	cargo fmt
	taplo format

setup:
	git config core.hooksPath .githooks

install:
	if [ -L "$(HOME)/.local/bin/sshpick" ]; then rm "$(HOME)/.local/bin/sshpick"; fi
	cargo install --locked --path crates/sshpick --root $(HOME)/.local
