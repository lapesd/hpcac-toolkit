SHELL = /bin/bash

.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

docker-run:  ## starts the HPC@Cloud Toolkit containers using docker-compose
	docker-compose up -d

migrate:  ## execute database migrations using refinery-cli
	cargo install refinery_cli
	refinery migrate -c psql_refinery.toml -p ./migrations/

cluster-create: ## create a EC2 cluster using terraform
	terraform init
	terraform plan -out plan
	terraform apply plan

cluster-destroy: ## destroy all cluster resources
	terraform destroy -auto-approve

compile-heat-exchange: ## Compile and run the heat_equations sample application
	scp -r ./apps/heat_exchange ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) make -C /var/nfs_dir/heat_exchange
	scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

run-heat-exchange:
	ssh ec2-user@$(ip) mpirun -np $(n) --hostfile /var/nfs_dir/hostfile /var/nfs_dir/heat_exchange/parallelHeatEq
