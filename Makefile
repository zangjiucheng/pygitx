PYTHON ?= python
.PHONY: help develop release docs test clean

help:
	@echo "Targets:"
	@echo "  develop  - install histgit in editable mode (expects maturin available)"
	@echo "  release  - build wheel (expects maturin available)"
	@echo "  docs     - build Sphinx docs to docs/_build/html (expects sphinx available)"
	@echo "  test     - run cargo tests (including python-tests feature)"
	@echo "  clean    - remove build artifacts"

develop:
	maturin develop --features python-extension

release:
	maturin build --features python-extension --release

docs:
	sphinx-build -b html docs docs/_build/html

test:
	cargo test --features python-tests

clean:
	cargo clean
	rm -rf target/wheels
	rm -rf docs/_build
