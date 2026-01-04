UV ?= uv
.PHONY: help venv develop release docs test coverage clean cleancargo cleanvenv

help:
	@echo "Targets:"
	@echo "  venv     - create a local .venv using uv"
	@echo "  install  - install build dependencies into .venv (via uv)"
	@echo "  develop  - install pygitx in editable mode (pip install -e .)"
	@echo "  release  - build wheel (pip wheel . -w dist)"
	@echo "  docs     - install docs/requirements.txt and build Sphinx docs to docs/_build/html (via uv)"
	@echo "  test     - run cargo tests (including python-tests feature)"
	@echo "  coverage - run pytest with coverage for python package"
	@echo "  clean    - remove build artifacts and .venv"
	@echo "  cleancargo - cargo clean only"
	@echo "  cleanvenv  - remove local .venv"

venv:
	@if [ -d .venv ]; then echo ".venv already exists; skipping creation"; else $(UV) venv; fi
	
install:
	$(UV) pip install --upgrade pip setuptools wheel maturin pytest pytest-cov

develop:
	pip install -e .

release:
	pip wheel . -w dist

docs:
	$(UV) pip install -r docs/requirements.txt
	$(UV) run --with sphinx --with sphinx_rtd_theme -- sphinx-build -b html docs docs/_build/html

test:
	cargo test --features python-tests

coverage:
	pytest --cov=pygitx --cov-report=term-missing

clean: cleancargo cleanvenv
	rm -rf target/wheels
	rm -rf docs/_build

cleancargo:
	cargo clean

cleanvenv:
	rm -rf .venv
