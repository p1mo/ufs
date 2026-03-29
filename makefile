IGNORE = -i temp/ -i target/

.PHONY: dev
dev:
	cargo watch $(IGNORE) --clear -x 'run --example basic'