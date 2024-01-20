CC=mpicc
LD=mpicc

MPIDIR=${HOME}/opt/mpi
MPIINC=-I$(MPIDIR)/include
MPILIB=-lpthread -L$(MPIDIR)/lib -lmpi

# The user must set SCR_ROOT to point to the SCR installation

SCRLIBDIR=-L$(SCR_ROOT)/lib64 -Wl,-rpath,$(SCR_ROOT)/lib64 -lscr
SCRINCLUDES=-I$(SCR_ROOT)/include

CFLAGS=-g -Wall
LDFLAGS= $(MPILIB) -g

LINK=$(LD)

APPS=jacobi_noft jacobi_ulfm jacobi_scr

all: $(APPS)

jacobi_noft: jacobi_noft.o main.o
	$(LINK) -o $@ $^ -lm

jacobi_ulfm: jacobi_ulfm.o main.o
	$(LINK) -o $@ $^ -lm

jacobi_scr: main_scr.o
	$(LINK) $(SCRINCLUDES) -o $@ $@.c $^ -lm $(SCRLIBDIR)

%.o: %.c jacobi.h
	$(CC) -c $(CFLAGS) -o $@ $<

clean:
	rm -f *.o $(APPS) *~
