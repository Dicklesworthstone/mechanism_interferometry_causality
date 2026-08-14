# `python` is deliberately not used directly: modern macOS ships no such binary, and
# where one exists it is usually a system interpreter lacking jsonschema and lxml. The
# project is uv-managed, so resolve uv when present and fall back to python3.
PYTHON ?= $(shell command -v uv >/dev/null 2>&1 && echo "uv run python" || echo python3)

.PHONY: all paper simulations examples check rust site clean
all: examples simulations paper check

examples:
	$(PYTHON) scripts/generate_example_data.py

simulations:
	$(PYTHON) scripts/generate_simulations.py

paper:
	cd paper && latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
	cp paper/main.pdf site/mechanism_interferometry.pdf

check:
	$(PYTHON) scripts/check_repo.py

rust:
	cargo test --workspace --no-default-features

site:
	./scripts/serve_site.sh

clean:
	cd paper && latexmk -C
	rm -rf target __pycache__ scripts/__pycache__ _renders
