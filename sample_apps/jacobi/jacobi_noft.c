#include <math.h>
#include <mpi.h>
#include <stdio.h>
#include <stdlib.h>
#include "jacobi.h"


/**
 * Prints the minimum and maximum timings of a specific loop in the program.
 *
 * @param scomm  MPI communicator for the processes involved in the timings.
 * @param rank   Rank of the current MPI process.
 * @param twf    Time (in seconds) taken for the specific loop in the current MPI process.
 */
void print_timings(MPI_Comm scomm, int rank, double twf)
{
    // Storage for min and max times
    double mtwf, Mtwf;

    // Perform reduction to find the minimum time across all MPI processes
    MPI_Reduce(&twf, &mtwf, 1, MPI_DOUBLE, MPI_MIN, 0, scomm);

    // Perform reduction to find the maximum time across all MPI processes
    MPI_Reduce(&twf, &Mtwf, 1, MPI_DOUBLE, MPI_MAX, 0, scomm);

    // If the current process is rank 0, print the min and max timings
    if (0 == rank) {
        printf(
            "##### Timings #####\n"
            "# MIN: %13.5e \t MAX: %13.5e\n",
            mtwf, Mtwf
        );
    }
}


/**
 * Performs one iteration of the Successive Over-Relaxation (SOR) method
 * on the input matrix and computes the squared L2-norm of the difference
 * between the input and output matrices.
 *
 * @param nm   Pointer to the output matrix after one iteration of the SOR method.
 * @param om   Pointer to the input matrix.
 * @param nb   Number of columns in the input matrix.
 * @param mb   Number of rows in the input matrix.
 * @return     The squared L2-norm of the difference between the input and output matrices.
 */
TYPE SOR1(TYPE* nm, TYPE* om, int nb, int mb)
{
    TYPE norm = 0.0;
    TYPE _W = 2.0 / (1.0 + M_PI / (TYPE)nb);
    int i, j, pos;

    // Iterate through each element of the matrix
    for (j = 0; j < mb; j++) {
        for (i = 0; i < nb; i++) {
            // Compute the position of the current element
            pos = 1 + i + (j + 1) * (nb + 2);

            // Update the current element using the SOR method
            nm[pos] = (1 - _W) * om[pos] +
                      _W / 4.0 * (nm[pos - 1] +
                                  om[pos + 1] +
                                  nm[pos - (nb + 2)] +
                                  om[pos + (nb + 2)]);

            // Accumulate the squared L2-norm of the difference
            norm += (nm[pos] - om[pos]) * (nm[pos] - om[pos]);
        }
    }

    return norm;
}


/**
 * Performs any required pre-initialization steps for the Jacobi method.
 * This function is a placeholder that can be extended if needed.
 *
 * @return     0 on successful completion.
 */
int preinit_jacobi_cpu(void)
{
    // Currently, there are no pre-initialization steps required for the
    // Jacobi method on the CPU. This function serves as a placeholder and
    // can be extended if necessary.

    return 0;
}

int jacobi_cpu(TYPE* matrix, int NB, int MB, int P, int Q, MPI_Comm comm, TYPE epsilon)
{
    int i, iter = 0;
    int rank, size, ew_rank, ew_size, ns_rank, ns_size;
    TYPE *om, *nm, *tmpm, *send_east, *send_west, *recv_east, *recv_west, diff_norm;
    double start, twf=0; /* timings */
    MPI_Comm ns, ew;
    MPI_Request req[8] = {MPI_REQUEST_NULL, MPI_REQUEST_NULL, MPI_REQUEST_NULL, MPI_REQUEST_NULL,
                          MPI_REQUEST_NULL, MPI_REQUEST_NULL, MPI_REQUEST_NULL, MPI_REQUEST_NULL};

    MPI_Comm_rank(comm, &rank);
    MPI_Comm_size(comm, &size);

    om = matrix;
    nm = (TYPE*)calloc(sizeof(TYPE), (NB+2) * (MB+2));
    send_east = (TYPE*)malloc(sizeof(TYPE) * MB);
    send_west = (TYPE*)malloc(sizeof(TYPE) * MB);
    recv_east = (TYPE*)malloc(sizeof(TYPE) * MB);
    recv_west = (TYPE*)malloc(sizeof(TYPE) * MB);

    /* create the north-south and east-west communicator */
    MPI_Comm_split(comm, rank % P, rank, &ns);
    MPI_Comm_size(ns, &ns_size);
    MPI_Comm_rank(ns, &ns_rank);
    MPI_Comm_split(comm, rank / P, rank, &ew);
    MPI_Comm_size(ew, &ew_size);
    MPI_Comm_rank(ew, &ew_rank);

    start = MPI_Wtime();
    do {
        /* post receives from the neighbors */
        if( 0 != ns_rank )
            MPI_Irecv( RECV_NORTH(om), NB, MPI_TYPE, ns_rank - 1, 0, ns, &req[0]);
        if( (ns_size-1) != ns_rank )
            MPI_Irecv( RECV_SOUTH(om), NB, MPI_TYPE, ns_rank + 1, 0, ns, &req[1]);
        if( (ew_size-1) != ew_rank )
            MPI_Irecv( recv_east,      MB, MPI_TYPE, ew_rank + 1, 0, ew, &req[2]);
        if( 0 != ew_rank )
            MPI_Irecv( recv_west,      MB, MPI_TYPE, ew_rank - 1, 0, ew, &req[3]);

        /* post the sends */
        if( 0 != ns_rank )
            MPI_Isend( SEND_NORTH(om), NB, MPI_TYPE, ns_rank - 1, 0, ns, &req[4]);
        if( (ns_size-1) != ns_rank )
            MPI_Isend( SEND_SOUTH(om), NB, MPI_TYPE, ns_rank + 1, 0, ns, &req[5]);
        for(i = 0; i < MB; i++) {
            send_west[i] = om[(i+1)*(NB+2)      + 1];  /* the real local data */
            send_east[i] = om[(i+1)*(NB+2) + NB + 0];  /* not the ghost region */
        }
        if( (ew_size-1) != ew_rank)
            MPI_Isend( send_east,      MB, MPI_TYPE, ew_rank + 1, 0, ew, &req[6]);
        if( 0 != ew_rank )
            MPI_Isend( send_west,      MB, MPI_TYPE, ew_rank - 1, 0, ew, &req[7]);
        /* wait until they all complete */
        MPI_Waitall(8, req, MPI_STATUSES_IGNORE);

        /* unpack the east-west newly received data */
        for(i = 0; i < MB; i++) {
            om[(i+1)*(NB+2)         ] = recv_west[i];
            om[(i+1)*(NB+2) + NB + 1] = recv_east[i];
        }

        /**
         * Call the Successive Over Relaxation (SOR) method
         */
        diff_norm = SOR1(nm, om, NB, MB);

        MPI_Allreduce(MPI_IN_PLACE, &diff_norm, 1, MPI_TYPE, MPI_SUM,
                      comm);
        if(0 == rank) {
            printf("Iteration %4d norm %f\n", iter, sqrtf(diff_norm));
        }
        tmpm = om; om = nm; nm = tmpm;  /* swap the 2 matrices */
        iter++;
    } while((iter < MAX_ITER) && (sqrt(diff_norm) > epsilon));

    twf = MPI_Wtime() - start;
    print_timings( comm, rank, twf );

    if(matrix != om) free(om);
    else free(nm);
    free(send_west);
    free(send_east);
    free(recv_west);
    free(recv_east);

    MPI_Comm_free(&ns);
    MPI_Comm_free(&ew);

    return iter;
}

