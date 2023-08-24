SHELL = /bin/bash

.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

init:  ## start the HPC@Cloud infrastructure containers only
	docker-compose up -d postgres minio

stop:  ## stops the HPC@Cloud Toolkit containers
	docker-compose stop

create-cluster:  ## create a cluster using the cluster_config.yaml configutaion
	python manage.py create_cluster_config cluster_config.yaml
	python manage.py create_cluster test_cluster

destroy-cluster:  ## destroy the test_cluster
	python manage.py destroy_cluster
