#include <math.h>
#include <mpi.h>
#include <mpi-ext.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <string.h>
#include <signal.h>
#include <setjmp.h>
#include "jacobi.h"


static int MPIX_Comm_replace(MPI_Comm comm, MPI_Comm *newcomm);

static int rank = MPI_PROC_NULL, verbose = 0;
static char estr[MPI_MAX_ERROR_STRING]=""; 
static int strl;

extern char** gargv;

static int iteration = 0, ckpt_iteration = 0, last_dead = MPI_PROC_NULL;
static MPI_Comm ew, ns;

static TYPE *bckpt = NULL;
static jmp_buf stack_jmp_buf;

#define CKPT_STEP 10


/**
 * Reloads checkpoint data for the application, resetting the iteration
 * and preventing further error injection.
 *
 * @param comm The MPI communicator.
 * @return 0 on success.
 */
static int app_reload_ckpt(MPI_Comm comm) {
    /* Fall back to the last checkpoint */
    MPI_Allreduce(&ckpt_iteration, &iteration, 1, MPI_INT, MPI_MIN, comm);
    iteration++;
    return 0;
}


// `world` will swap between `worldc[0]` and `worldc[1]` after each respawn
static MPI_Comm worldc[2] = { MPI_COMM_NULL, MPI_COMM_NULL };
static int worldi = 0;
#define world (worldc[worldi])


/**
 * Attempts to repair the MPI communicator after a failure, reload checkpoints,
 * and determine if any work needs to be redone.
 *
 * @param comm The MPI communicator associated with the process.
 * @return true if the application needs to redo some iterations due to failure,
 *         false if no failure was fixed and no work needs to be redone.
 */
static int app_needs_repair(MPI_Comm comm)
{
    // If this is the first error on this communicator, perform the swap of worlds
    if (comm == world) {
        // Swap the worlds
        worldi = (worldi + 1) % 2;

        // Keep 'comm' around until the user completes all pending operations
        // to ensure the error handler remains attached
        if (MPI_COMM_NULL != world) {
            MPI_Comm_free(&world);
        }

        // Replace the communicator and reload the checkpoint
        MPIX_Comm_replace(comm, &world);
        app_reload_ckpt(world);

        // If the communicator is still MPI_COMM_NULL, no repair was done
        // and no work needs to be redone
        if (MPI_COMM_NULL == comm) {
            return false;
        }

        // If a repair was done, jump to the appropriate location in the code
        _longjmp(stack_jmp_buf, 1);
    }

    // If a repair was done, return true to indicate work needs to be redone
    return true;
}


/**
 * Error handler function for MPI processes.
 * Handles process failures and revocation errors, invoking the repair process.
 * For other errors, it aborts the MPI execution.
 *
 * @param pcomm   Pointer to the MPI communicator associated with the error.
 * @param errcode Pointer to the error code returned by the MPI operation that raised the error.
 * @param ...     Additional arguments depending on the error (not used in this implementation).
 */
static void errhandler_respawn(MPI_Comm* pcomm, int* errcode, ...)
{
    int eclass;
    MPI_Error_class(*errcode, &eclass);

    if (verbose) {
        MPI_Error_string(*errcode, estr, &strl);
        fprintf(stderr, "%04d: errhandler invoked with error %s\n", rank, estr);
    }

    // Check if the error is related to process failure or revocation
    if (MPIX_ERR_PROC_FAILED != eclass && MPIX_ERR_REVOKED != eclass) {
        // For other errors, abort the MPI execution
        MPI_Abort(MPI_COMM_WORLD, *errcode);
    }

    // Revoke the MPI communicator for the failed process
    MPIX_Comm_revoke(world);

    // Initiate the repair process for the application
    app_needs_repair(world);
}


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

static int MPIX_Comm_replace(MPI_Comm comm, MPI_Comm *newcomm)
{
    MPI_Comm icomm, /* the intercomm between the spawnees and the old (shrinked) world */
        scomm, /* the local comm for each sides of icomm */
        mcomm; /* the intracomm, merged from icomm */
    MPI_Group cgrp, sgrp, dgrp;
    int rc, flag, rflag, i, nc, ns, nd, crank, srank, drank;

 redo:
    if( comm == MPI_COMM_NULL ) {
        // I'm a new process
        MPI_Comm_get_parent(&icomm);
        scomm = MPI_COMM_WORLD;
        MPI_Recv(&crank, 1, MPI_INT, 0, 1, icomm, MPI_STATUS_IGNORE);
        if( verbose ) {
            MPI_Comm_rank(scomm, &srank);
            printf("Spawnee %d: crank=%d\n", srank, crank);
        }
    } else {
        // I'm a survivor process
        MPIX_Comm_shrink(comm, &scomm);
        MPI_Comm_size(scomm, &ns);
        MPI_Comm_size(comm, &nc);
        nd = nc-ns;  // Number of dead processes
        if( 0 == nd ) {
            // Nobody was dead
            MPI_Comm_free(&scomm);
            *newcomm = comm;
            return MPI_SUCCESS;
        }
        // Set the error handler
        MPI_Comm_set_errhandler( scomm, MPI_ERRORS_RETURN );

        rc = MPI_Comm_spawn(gargv[0], &gargv[1], nd, MPI_INFO_NULL, 0, scomm, &icomm, MPI_ERRCODES_IGNORE);
        flag = (MPI_SUCCESS == rc);
        MPIX_Comm_agree(scomm, &flag);
        if( !flag ) {
            if( MPI_SUCCESS == rc ) {
                MPIX_Comm_revoke(icomm);
                MPI_Comm_free(&icomm);
            }
            MPI_Comm_free(&scomm);
            if( verbose ) fprintf(stderr, "%04d: comm_spawn failed, redo\n", rank);
            goto redo;
        }

        // remembering the former rank: we will reassign the same ranks in the new world.
        MPI_Comm_rank(comm, &crank);
        MPI_Comm_rank(scomm, &srank);
        // The rank 0 in the `scomm` communicator is going to determine the ranks at 
        // which the spares need to be inserted.
        if(0 == srank) {
            // Processes in `comm` but not in `scomm` are the dead ones.
            MPI_Comm_group(comm, &cgrp);
            MPI_Comm_group(scomm, &sgrp);
            MPI_Group_difference(cgrp, sgrp, &dgrp);
            // Computing the rank assignment for the newly inserted spares.
            for(i=0; i<nd; i++) {
                MPI_Group_translate_ranks(dgrp, 1, &i, cgrp, &drank);
                // sending their new assignment to all new procs.
                MPI_Send(&drank, 1, MPI_INT, i, 1, icomm);
                last_dead = drank;
            }
            MPI_Group_free(&cgrp); MPI_Group_free(&sgrp); MPI_Group_free(&dgrp);
        }
    }

    // Merge the intercomm, to reconstruct an intracomm
    rc = MPI_Intercomm_merge(icomm, 1, &mcomm);
    rflag = flag = (MPI_SUCCESS==rc);

    // Check that this operation worked before we proceed further
    MPIX_Comm_agree(scomm, &flag);
    if( MPI_COMM_WORLD != scomm ) MPI_Comm_free(&scomm);
    MPIX_Comm_agree(icomm, &rflag);
    MPI_Comm_free(&icomm);
    if( !(flag && rflag) ) {
        if( MPI_SUCCESS == rc ) {
            MPI_Comm_free(&mcomm);
        }
        if( verbose ) fprintf(stderr, "%04d: Intercomm_merge failed, redo\n", rank);
        goto redo;
    }

    // Now, let's reorder `mcomm` based on its original rank ordering in `comm`. 
    // The `MPI_Comm_split` function remove any spare processes and reorder the ranks, 
    // ensuring that all surviving processes retain their original positions.
    rc = MPI_Comm_split(mcomm, 1, crank, newcomm);

    // In the case that new failures have disrupted the process, it is possible 
    // that the `MPI_Comm_split` function or some of the communications above may have failed. 
    // To ensure success at all ranks, we need to verify our progress or retry until it works.
    flag = (MPI_SUCCESS==rc);
    MPIX_Comm_agree(mcomm, &flag);
    MPI_Comm_free(&mcomm);
    if( !flag ) {
        if( MPI_SUCCESS == rc ) {
            MPI_Comm_free( newcomm );
        }
        if( verbose ) fprintf(stderr, "%04d: comm_split failed, redo\n", rank);
        goto redo;
    }

    // Restore the error handler:
    if( MPI_COMM_NULL != comm ) {
        MPI_Errhandler errh;
        MPI_Comm_get_errhandler( comm, &errh );
        MPI_Comm_set_errhandler( *newcomm, errh );
    }
    printf("Done with the recovery (rank %d)\n", crank);

    return MPI_SUCCESS;
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

/**
 * Implements the Jacobi method for solving a system of linear equations using
 * MPI on a CPU. The convergence of the solution is controlled by the specified
 * epsilon value.
 *
 * @param matrix   Pointer to the input matrix of the linear system.
 * @param NB       Number of columns in the input matrix.
 * @param MB       Number of rows in the input matrix.
 * @param P        Number of partitions along the x-axis.
 * @param Q        Number of partitions along the y-axis.
 * @param comm     MPI communicator for the parallel computation.
 * @param epsilon  Convergence threshold for the Jacobi method.
 * @return         Number of iterations performed by the Jacobi method.
 */
int jacobi_cpu(TYPE* matrix, int NB, int MB, int P, int Q, MPI_Comm comm, TYPE epsilon)
{
    int i, is_allowed_to_kill = 1;
    int world_size, ew_rank, ew_size, ns_rank, ns_size;
    TYPE *old_matrix, *new_matrix, *temp_matrix, *send_east, *send_west, *recv_east, *recv_west, diff_norm;
    double start_time, total_wf_time = 0; // timings
    MPI_Errhandler errh;
    MPI_Comm parent;
    int do_recover = 0;
    MPI_Request req[8] = {MPI_REQUEST_NULL, MPI_REQUEST_NULL, MPI_REQUEST_NULL, MPI_REQUEST_NULL,
                          MPI_REQUEST_NULL, MPI_REQUEST_NULL, MPI_REQUEST_NULL, MPI_REQUEST_NULL};

    printf("Starting/resuming Jacobi method...\n");
    MPI_Comm_create_errhandler(&errhandler_respawn, &errh);
    // Check if it is a spare process
    MPI_Comm_get_parent(&parent);
    if (MPI_COMM_NULL == parent)
    {
        // First run: Create an initial world, a copy of MPI_COMM_WORLD
        MPI_Comm_dup(comm, &world);
    }
    else
    {
        is_allowed_to_kill = 0;
        ckpt_iteration = MAX_ITER;
        // It is a spare process, get the repaired world
        app_needs_repair(MPI_COMM_NULL);
    }

    MPI_Comm_rank(world, &rank);
    MPI_Comm_size(world, &world_size);
    printf("Rank %d is joining the execution at iteration %d\n", rank, iteration);

    old_matrix = matrix;
    new_matrix = (TYPE *)calloc(sizeof(TYPE), (NB + 2) * (MB + 2));
    send_east = (TYPE *)malloc(sizeof(TYPE) * MB);
    send_west = (TYPE *)malloc(sizeof(TYPE) * MB);
    recv_east = (TYPE *)malloc(sizeof(TYPE) * MB);
    recv_west = (TYPE *)malloc(sizeof(TYPE) * MB);

    // Prepare the space for the buddy checkpoint
    bckpt = (TYPE *)malloc(sizeof(TYPE) * (NB + 2) * (MB + 2));

restart: // This is the restart point
    do_recover = _setjmp(stack_jmp_buf);
    // Set an errhandler on world, so that a failure is not fatal anymore
    MPI_Comm_set_errhandler(world, errh);

    // Create the north-south and east-west communicators
    MPI_Comm_split(world, rank % P, rank, &ns);
    MPI_Comm_size(ns, &ns_size);
    MPI_Comm_rank(ns, &ns_rank);
    MPI_Comm_split(world, rank / P, rank, &ew);
    MPI_Comm_size(ew, &ew_size);
    MPI_Comm_rank(ew, &ew_rank);
    if (do_recover || (MPI_COMM_NULL != parent))
    {
        // Simple approach: everybody retrieves their data from the buddy rank
        MPI_Irecv(old_matrix, (NB + 2) * (MB + 2), MPI_TYPE, (rank + 1) % world_size, 111, world, &req[0]);

        if (rank == last_dead) // This process has nothing to send
            MPI_Send(bckpt, 0, MPI_TYPE, (rank - 1 + world_size) % world_size, 111, world);
        else
            MPI_Send(bckpt, (NB + 2) * (MB + 2), MPI_TYPE, (rank - 1 + world_size) % world_size, 111, world);
        MPI_Wait(&req[0], MPI_STATUS_IGNORE);
        goto do_sor;
    }

    start_time = MPI_Wtime();
    do
    {
        // Post receives from the neighbors
        if (0 != ns_rank)
            MPI_Irecv(RECV_NORTH(old_matrix), NB, MPI_TYPE, ns_rank - 1, 0, ns, &req[0]);
        if ((ns_size - 1) != ns_rank)
            MPI_Irecv(RECV_SOUTH(old_matrix), NB, MPI_TYPE, ns_rank + 1, 0, ns, &req[1]);
        if ((ew_size - 1) != ew_rank)
            MPI_Irecv(recv_east, MB, MPI_TYPE, ew_rank + 1, 0, ew, &req[2]);
        if (0 != ew_rank)
            MPI_Irecv(recv_west, MB, MPI_TYPE, ew_rank - 1, 0, ew, &req[3]);

        // Post the sends
        if (0 != ns_rank)
            MPI_Isend(SEND_NORTH(old_matrix), NB, MPI_TYPE, ns_rank - 1, 0, ns, &req[4]);
        if ((ns_size - 1) != ns_rank)
            MPI_Isend(SEND_SOUTH(old_matrix), NB, MPI_TYPE, ns_rank + 1, 0, ns, &req[5]);
        for (i = 0; i < MB; i++)
        {
            send_west[i] = old_matrix[(i + 1) * (NB + 2) + 1];      // The real local data
            send_east[i] = old_matrix[(i + 1) * (NB + 2) + NB + 0]; // Not the ghost region
        }
        if ((ew_size - 1) != ew_rank)
            MPI_Isend(send_east, MB, MPI_TYPE, ew_rank + 1, 0, ew, &req[6]);
        if (0 != ew_rank)
            MPI_Isend(send_west, MB, MPI_TYPE, ew_rank - 1, 0, ew, &req[7]);

        /*
        // Uncomment this code block to simulate a failure using a SIGKILL
        if (is_allowed_to_kill && (42 == iteration))
        {
            is_allowed_to_kill = 0;
            if (1 == rank)
            {
                printf("Raising SIGKILL....\n");
                raise(SIGKILL);
            }
        }
        */

        // Wait until they all complete
        MPI_Waitall(8, req, MPI_STATUSES_IGNORE);
        for (i = 0; i < MB; i++)
        {
            old_matrix[(i + 1) * (NB + 2)] = recv_west[i];
            old_matrix[(i + 1) * (NB + 2) + NB + 1] = recv_east[i];
        }

        // Perform a checkpoint every CKPT_STEP iterations
        if ((0 != iteration) && (0 == (iteration % CKPT_STEP)))
        {
            // Make sure the environment is safe before initiating circular buddy checkpointing
            if (0 == rank)
            {
                printf("Initiate circular buddy checkpointing\n");
            }

            // Receive the checkpoint data from the previous process
            MPI_Irecv(bckpt, (NB + 2) * (MB + 2), MPI_TYPE, (rank - 1 + world_size) % world_size, 111, world, &req[0]);

            // Send the checkpoint data to the next process
            MPI_Send(old_matrix, (NB + 2) * (MB + 2), MPI_TYPE, (rank + 1) % world_size, 111, world);

            // Wait for the receive operation to complete
            MPI_Wait(&req[0], MPI_STATUS_IGNORE);

            // Update the checkpoint iteration
            ckpt_iteration = iteration;
        }

        // Label for restarting the computation after recovery
        do_sor:
            // Replicate the east-west newly received data
            for (i = 0; i < MB; i++)
            {
                new_matrix[(i + 1) * (NB + 2)] = old_matrix[(i + 1) * (NB + 2)];
                new_matrix[(i + 1) * (NB + 2) + NB + 1] = old_matrix[(i + 1) * (NB + 2) + NB + 1];
            }

            // Replicate the north-south neighbors
            for (i = 0; i < NB; i++)
            {
                new_matrix[i + 1] = old_matrix[i + 1];
                new_matrix[(NB + 2) * (MB + 1) + i + 1] = old_matrix[(NB + 2) * (MB + 1) + i + 1];
            }

            // Call the Successive Over Relaxation (SOR) method
            diff_norm = SOR1(new_matrix, old_matrix, NB, MB);

            if (verbose)
                printf("Rank %d norm %f at iteration %d\n", rank, diff_norm, iteration);

            // Allreduce to compute the global norm
            MPI_Allreduce(MPI_IN_PLACE, &diff_norm, 1, MPI_TYPE, MPI_SUM, world);

            if (0 == rank)
            {
                printf("Iteration %4d norm %f\n", iteration, sqrtf(diff_norm));
            }

            // Swap the old and new matrices
            temp_matrix = old_matrix;
            old_matrix = new_matrix;
            new_matrix = temp_matrix;

            // Increment the iteration
            iteration++;

    } while((iteration < MAX_ITER) && (sqrt(diff_norm) > epsilon));

    total_wf_time = MPI_Wtime() - start_time;
    print_timings(world, rank, total_wf_time);

    // Free the memory allocated for matrices and buffers
    // If the 'matrix' variable is different from 'old_matrix', free 'old_matrix'; otherwise, free 'new_matrix'
    free(matrix != old_matrix ? old_matrix : new_matrix);

    // Free the memory allocated for send and receive buffers
    free(send_west);
    free(send_east);
    free(recv_west);
    free(recv_east);


    MPI_Comm_free(&ns);
    MPI_Comm_free(&ew);

    return iteration;
}
