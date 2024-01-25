#include <mpi.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include "jacobi.h"

int debug = 0;
int use_scr_need_checkpoint = 0;

/**
 * Generates a border array of random values in the range of -0.5 to 0.5.
 *
 * @param border    Pointer to the array where border values will be stored.
 * @param nb_elems  Number of elements in the border array.
 * @return          Returns 0 on successful generation, -1 if the input pointer is NULL.
 */
int generate_border(TYPE *border, size_t nb_elems)
{
    if (!border)
    {
        return -1; // Return error code if the input pointer is NULL
    }

    srand(time(NULL)); // Seed the random number generator (ideally only once)

    for (size_t i = 0; i < nb_elems; i++)
    {
        border[i] = (TYPE)((rand() / (double)RAND_MAX) - 0.5);
    }

    return 0;
}

/**
 * Initializes a matrix with a border and zeros inside.
 *
 * @param matrix  Pointer to the matrix to be initialized.
 * @param border  Pointer to the border array.
 * @param nb      Number of columns in the matrix (excluding border).
 * @param mb      Number of rows in the matrix (excluding border).
 * @return        Returns 0 on successful initialization.
 */
int init_matrix(TYPE *matrix, const TYPE *border, int nb, int mb)
{
    int i, j, idx = 0;

    // Initialize the top border
    for (idx = 0; idx < nb + 2; idx++)
    {
        matrix[idx] = border[idx];
    }
    matrix += idx;

    // Initialize the middle part of the matrix, including left and right borders
    for (j = 0; j < mb; j++)
    {
        // Set left border value
        matrix[0] = border[idx];
        idx++;

        // Set inner values to zero
        for (i = 0; i < nb; i++)
        {
            matrix[1 + i] = 0.0;
        }

        // Set right border value
        matrix[nb + 1] = border[idx];
        idx++;

        // Move to the next row
        matrix += (nb + 2);
    }

    // Initialize the bottom border
    for (i = 0; i < nb + 2; i++)
    {
        matrix[i] = border[idx + i];
    }

    return 0;
}

/**
 * The main function for the Jacobi solver program.
 */
int main(int argc, char *argv[])
{
    int i, rc, size, rank, NB = -1, MB = -1, P = -1, Q = -1;
    TYPE *om, *border = NULL, epsilon = 1e-6;
    MPI_Comm parent;

    // Parse command-line arguments
    for (i = 1; i < argc; i++)
    {
        if (!strcmp(argv[i], "-p"))
        {
            i++;
            P = atoi(argv[i]);
            continue;
        }
        if (!strcmp(argv[i], "-q"))
        {
            i++;
            Q = atoi(argv[i]);
            continue;
        }
        if (!strcmp(argv[i], "-NB"))
        {
            i++;
            NB = atoi(argv[i]);
            continue;
        }
        if (!strcmp(argv[i], "-MB"))
        {
            i++;
            MB = atoi(argv[i]);
            continue;
        }
        if (!strcmp(argv[i], "--debug"))
        {
            debug = 1;
            continue;
        }
        if (!strcmp(argv[i], "--use-scr-need-checkpoint"))
        {
            use_scr_need_checkpoint = 1;
            continue;
        }
    }

    // Check if required arguments are provided
    if (P < 1)
    {
        printf("Missing number of processes per row (-p #)\n");
        exit(-1);
    }
    if (Q < 1)
    {
        printf("Missing number of processes per column (-q #)\n");
        exit(-1);
    }
    if (NB == -1)
    {
        printf("Missing the first dimension of the matrix (-NB #)\n");
        exit(-1);
    }
    if (MB == -1)
    {
        MB = NB;
    }

    // Initialize the Jacobi CPU
    preinit_jacobi_cpu();

    // Initialize MPI
    MPI_Init(NULL, NULL);

    // Get the parent communicator
    MPI_Comm_get_parent(&parent);
    if (MPI_COMM_NULL == parent)
    {
        MPI_Comm_size(MPI_COMM_WORLD, &size);
        MPI_Comm_rank(MPI_COMM_WORLD, &rank);
    }

    // Initialize the border, matrix, and intermediate matrix
    om = (TYPE *)malloc(sizeof(TYPE) * (NB + 2) * (MB + 2));
    border = (TYPE *)malloc(sizeof(TYPE) * 2 * (NB + 2 + MB));

    if (MPI_COMM_NULL == parent)
    {
        int seed = rank * NB * MB;
        srand(seed);
        generate_border(border, 2 * (NB + 2 + MB));
        init_matrix(om, border, NB, MB);
    }

    // Set the error handler for MPI
    MPI_Comm_set_errhandler(MPI_COMM_WORLD, MPI_ERRORS_RETURN);

    rc = jacobi_cpu(om, NB, MB, P, Q, MPI_COMM_WORLD, 0 /* no epsilon */);

    if (rc < 0)
    {
        printf("The CPU Jacobi failed\n");
        goto cleanup_and_be_gone;
    }

cleanup_and_be_gone:
    // Free resources and finalize MPI
    free(om);
    free(border);

    MPI_Finalize();
    return 0;
}
