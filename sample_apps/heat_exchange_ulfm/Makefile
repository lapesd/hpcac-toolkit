MPI_CC = mpicc -Wall
LD = -lm

DIR = objectsDir
SRC = parallelHeatEqULFM.c parallelHeatEqULFMUtils.c
OBJ = $(addprefix $(DIR)/,$(SRC:.c=.o))

.PHONY: all
all: parallelHeatEqULFM

parallelHeatEqULFM: $(OBJ)
	$(MPI_CC) $(LD) -o $@ $^

$(DIR)/%.o: %.c
	$(MPI_CC) -c $< -o $@

$(OBJ): | $(DIR)

$(DIR):
	mkdir $(DIR)

clean:
	rm -f parallelHeatEqULFM
	rm -f output.dat
	rm -rf $(DIR)
