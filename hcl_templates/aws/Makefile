SHELL = /bin/bash

.DEFAULT_GOAL := help

.PHONY: help
help: ## show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

aws-configure: ## configure your aws credentials
	aws configure

cluster-create: ## create a EC2 cluster using terraform
	terraform -chdir="." init
	terraform -chdir="." plan -out aws-plan
	terraform -chdir="." apply aws-plan

cluster-destroy: ## destroy all cluster resources
	terraform -chdir="." destroy -auto-approve

compile-apps: ## compile applications
	scp -r ../../applications/heat-equations/no-ft ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) make -C /var/nfs_dir/no-ft

run: compile-apps ## execute La Place's heat diffusion equations solver
	scp -r ./hosts ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) mpirun -np $(n) --hostfile /var/nfs_dir/hosts /var/nfs_dir/no-ft/parallelHeatEq
