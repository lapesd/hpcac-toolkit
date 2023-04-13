CC=mpicc
LD=mpicc

MPIDIR=${HOME}/opt/mpi
MPIINC=-I$(MPIDIR)/include
MPILIB=-lpthread -L$(MPIDIR)/lib -lmpi

CFLAGS=-g -Wall
LDFLAGS= $(MPILIB) -g

LINK=$(LD)

APPS=jacobi_noft jacobi_ulfm

all: $(APPS)

jacobi_noft: jacobi_noft.o main.o
	$(LINK) -o $@ $^ -lm

jacobi_ulfm: jacobi_ulfm.o main.o
	$(LINK) -o $@ $^ -lm

%.o: %.c jacobi.h
	$(CC) -c $(CFLAGS) -o $@ $<

clean:
	rm -f *.o $(APPS) *~
