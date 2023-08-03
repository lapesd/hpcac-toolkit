#include <stdio.h>
#include <mpi.h>
#include <unistd.h>

int main(int argc, char **argv)
{
    int rank, size;
    char hostname[256];

    MPI_Init(&argc, &argv);               // Initialize MPI environment
    MPI_Comm_rank(MPI_COMM_WORLD, &rank); // Get the rank of the process
    MPI_Comm_size(MPI_COMM_WORLD, &size); // Get the number of processes

    gethostname(hostname, 255); // Get the hostname of the node

    printf("Hello from rank %d of %d running on node %s\n", rank, size, hostname);

    MPI_Finalize(); // Finalize the MPI environment

    return 0;
}
