.PHONY: test example
test:
	cargo test --workspace --all-targets

example:
	cargo run --example basic
