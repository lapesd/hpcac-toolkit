SHELL = /bin/bash

.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

cluster-create: ## create a EC2 cluster using terraform
	terraform init
	terraform plan -out plan
	time terraform apply plan

cluster-destroy: ## destroy all cluster resources
	time terraform destroy -auto-approve


build-singularity-hello-world: ## build the hello_world singularity container
	scp -r ./../sample_apps/hello_world ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) sudo /usr/local/bin/singularity build /var/nfs_dir/hello_world/mvapich_hybrid.sif /var/nfs_dir/hello_world/mvapich_hybrid.def
	scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

run-singularity-hello-world: ## run the hello_world singularity container
	ssh ec2-user@$(ip) mpirun -n $(n) --hostfile /var/nfs_dir/hostfile singularity run /var/nfs_dir/hello_world/mvapich_hybrid.sif /opt/mpitest


build-singularity-NPB-IS: ## build the NPB IS singularity container
	scp -r ../../sample_apps/NPB ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) sudo /usr/local/bin/singularity build /var/nfs_dir/NPB/npb_image.sif /var/nfs_dir/NPB/npb_image.def
	scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

run-singularity-NPB-IS: ## run the NPB IS singularity container
	ssh ec2-user@$(ip) mpirun -n $(n) --hostfile /var/nfs_dir/hostfile singularity run /var/nfs_dir/NPB/npb_image.sif /opt/is.A.2


build-heat-exchange-ulfm: ## compile the heat_exchange ULFM sample application
	scp -r ../../sample_apps/heat_exchange_ulfm ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) make -C /var/nfs_dir/heat_exchange
	scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

run-heat-exchange-ulfm: ## execute the heat exchange ULFM sample application
	ssh ec2-user@$(ip) mpirun -np $(n) --hostfile /var/nfs_dir/hostfile /var/nfs_dir/heat_exchange_ulfm/parallelHeatEqULFM


build-jacobi-ulfm: ## compile a fault-tolerant (ULFM) Jacobi solver
	scp -r ../../sample_apps/jacobi ec2-user@$(ip):/var/nfs_dir
	ssh ec2-user@$(ip) make all -C /var/nfs_dir/jacobi
	scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

run-jacobi-ulfm: build-jacobi-ulfm ## execute a fault-tolerant (ULFM) Jacobi solver
	time ssh ec2-user@$(ip) mpirun --oversubscribe --with-ft ulfm -np $(n) --hostfile /var/nfs_dir/hostfile /var/nfs_dir/jacobi/jacobi_ulfm -p 2 -q 2 -NB 128
