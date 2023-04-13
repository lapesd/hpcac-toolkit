#ifndef JACOBI_HEADER_H
#define JACOBI_HEADER_H

#include <mpi.h>

#define TYPE       double
#define MPI_TYPE   MPI_DOUBLE

#define MAX_ITER 67

/**
 * Helper macros to compute the displacement of the
 * buffers for the north and south neighbors.
 */
#define RECV_NORTH(p) (((TYPE*)(p)) + (NB+2) * 0 + 1)
#define SEND_NORTH(p) (((TYPE*)(p)) + (NB+2) * 1 + 1)
#define RECV_SOUTH(p) (((TYPE*)(p)) + (NB+2) * (MB+1) + 1)
#define SEND_SOUTH(p) (((TYPE*)(p)) + (NB+2) * (MB) + 1)

int jacobi_cpu(TYPE* om, int NB, int MB, int P, int Q, MPI_Comm comm, TYPE epsilon);
int preinit_jacobi_cpu(void);

#endif  /* JACOBI_HEADER_H */
