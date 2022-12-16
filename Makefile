SHELL = /bin/bash

.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

docker-run:  ## Starts the HPC@Cloud Toolkit containers using docker-compose
	docker-compose up -d

migrate:  ## Execute database migrations using refinery-cli
	cargo install refinery_cli
	refinery migrate -c psql_refinery.toml -p ./migrations/
