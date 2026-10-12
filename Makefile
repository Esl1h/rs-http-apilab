# rs-http-apilab: deterministic synthetic HTTP API simulator.

.PHONY: build run test unit-test test-smoke test-errors test-limits docker-build docker-build-runtime docker-up docker-down fmt fmt-check lint clean

build:
	scripts/build.sh

run:
	scripts/run.sh

test: test-smoke test-errors test-limits

unit-test:
	cargo test --locked

test-smoke:
	scripts/test-smoke.sh

test-errors:
	scripts/test-errors.sh

test-limits:
	scripts/test-limits.sh

docker-build:
	docker build -t rs-http-apilab:latest .

docker-build-runtime:
	docker build -f Dockerfile.runtime -t rs-http-apilab:latest .

docker-up:
	docker compose up -d --build

docker-down:
	docker compose down

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

lint:
	cargo clippy --locked -- -D warnings

clean:
	rm -rf bin target
