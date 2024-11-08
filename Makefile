SHELL = /bin/bash

.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

init:  ## start hpcac containers
	docker compose up -d

install:  ## install hpcac using pip
	pip install . && \
	aerich init -t hpcac_cli.db.TORTOISE_ORM && \
	aerich init-db && \
	aerich migrate --name migration && \
	aerich upgrade

stop:  ## stops the hpcac containers
	docker compose stop
