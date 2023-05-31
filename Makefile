SHELL = /bin/bash

.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

docker-run-dev:  ## start the HPC@Cloud infrastructure containers only
	docker-compose up -d postgres minio

docker-stop:  ## stops the HPC@Cloud Toolkit containers
	docker-compose stop
