NUM_ITERACOES ?= 8

run:
	@echo "Limpando arquivos antigos..."
	@python setup-configs.py clean
	@echo "Iniciando..."
	@for i in `seq 1 $(NUM_ITERACOES)`; do \
		echo "Iteration $$i"; \
		python setup-configs.py switch; \
		hpcac create-cluster; \
		hpcac run-tasks; \
		hpcac destroy-cluster; \
	done
	$(MAKE) export
