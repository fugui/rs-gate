.PHONY: all test test-metrics test-retention test-e2e fmt fmt-check clippy check build-web release clean

all: check release

test:
	cargo test --all

test-metrics:
	cargo test --test metrics_scrape_test -- --nocapture

test-retention:
	cargo test --test retention_cleanup_test -- --nocapture

test-e2e:
	cargo test --test e2e_full_flow_test -- --nocapture

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

clippy:
	cargo clippy --all-targets -- -D warnings

check: fmt-check clippy test

build-web:
	cd web && npm run build

release:
	bash scripts/build.sh

clean:
	cargo clean
	rm -rf web/dist
