MPI_CC = mpicc -Wall
LD = -lm

DIR = objectsDir
SRC = parallelHeatEq.c parallelHeatEqUtils.c
OBJ = $(addprefix $(DIR)/,$(SRC:.c=.o))

.PHONY: all
all: parallelHeatEq

parallelHeatEq: $(OBJ)
	$(MPI_CC) $(LD) -o $@ $^

$(DIR)/%.o: %.c
	$(MPI_CC) -c $< -o $@

$(OBJ): | $(DIR)

$(DIR):
	mkdir $(DIR)

clean:
	rm -f parallelHeatEq
	rm -f output.dat
	rm -rf $(DIR)
