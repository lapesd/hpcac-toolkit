SHELL = /bin/bash

.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

init:  ## start the HPC@Cloud infrastructure containers only
	docker compose up -d postgres minio
	pip install .
	aerich init -t hpcac_cli.db.TORTOISE_ORM

stop:  ## stops the HPC@Cloud Toolkit containers
	docker compose stop

init-db:  ## initialize HPC@Cloud database
	aerich init-db

migrate:  ## create and apply migrations
	pip install .
	aerich migrate --name migration
	aerich upgrade
