.PHONY: all paper simulations examples check rust site clean
all: examples simulations paper check

examples:
	python scripts/generate_example_data.py

simulations:
	python scripts/generate_simulations.py

paper:
	cd paper && latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex
	cp paper/main.pdf site/mechanism_interferometry.pdf

check:
	python scripts/check_repo.py

rust:
	cargo test --workspace --no-default-features

site:
	python -m http.server 8765 --directory site

clean:
	cd paper && latexmk -C
	rm -rf target __pycache__ scripts/__pycache__ _renders
