SHELL = /bin/bash

.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

cluster-create: ## create a EC2 cluster using terraform
	terraform init
	terraform plan -out plan
	terraform apply plan

cluster-destroy: ## destroy all cluster resources
	terraform destroy -auto-approve

build-heat-exchange: ## compile the heat_exchange sample application
	scp -r ../../sample_apps/heat_exchange ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) make -C /var/nfs_dir/heat_exchange
	scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

run-heat-exchange: ## execute the heat exchange sample application
	ssh ec2-user@$(ip) mpirun -np $(n) --hostfile /var/nfs_dir/hostfile /var/nfs_dir/heat_exchange/parallelHeatEq

build-singularity-hello-world: ## build the hello_world singularity container
	scp -r ./../sample_apps/hello_world ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) sudo /usr/local/bin/singularity build /var/nfs_dir/hello_world/mvapich_hybrid.sif /var/nfs_dir/hello_world/mvapich_hybrid.def
	scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

run-singularity-hello-world: ## run the hello_world singularity container
	ssh ec2-user@$(ip) mpirun -n $(n) --hostfile /var/nfs_dir/hostfile singularity run /var/nfs_dir/hello_world/mvapich_hybrid.sif /opt/mpitest

build-singularity-NPB: ## build the NPB IS singularity container
	scp -r ../../sample_apps/NPB ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) sudo /usr/local/bin/singularity build /var/nfs_dir/NPB/npb_image.sif /var/nfs_dir/NPB/npb_image.def
	scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

run-singularity-NPB-IS: ## run the NPB IS singularity container
	ssh ec2-user@$(ip) mpirun -n $(n) --hostfile /var/nfs_dir/hostfile singularity run /var/nfs_dir/NPB/npb_image.sif /opt/is.A.2

build-ulfm-is: ## build the ULFM version of the IS NBS benchmark
	scp -r ./../sample_apps/hello_world ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) sudo /usr/local/bin/singularity build /var/nfs_dir/hello_world/mvapich_hybrid.sif /var/nfs_dir/hello_world/mvapich_hybrid.def
	scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

run-ulfm-is: ## run the ULFM version of the IS NBS benchmark
	ssh ec2-user@$(ip) mpirun -n $(n) --hostfile /var/nfs_dir/hostfile singularity run /var/nfs_dir/hello_world/mvapich_hybrid.sif /opt/mpitest
