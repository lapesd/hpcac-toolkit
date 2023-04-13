#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <mpi.h>
#include "parallelHeatEq.h"

#define min(a, b) ((a) <= (b) ? (a) : (b))


int main(int argc, char *argv[]) {

    int i, j, k, l;

    FILE* output_file;

    int cores, n_nodes;
    int size_x, size_y, x_domains, y_domains;
    int size_global_x, size_global_y;
    int size_total_x, size_total_y;

    /* Current process */
    int local_rank;

    /* Arrays */
    double **x;
    double **x0;
    double *xTemp;
    double *xTempFinal;

    /* Space and time steps */
    double dt, dt1, dt2, hx, hy;

    /* Current local square difference */
    double localDiff;

    /* Current global difference and limit convergence */
    double result, epsilon;

    /* Convergence pseudo-boolean */
    int converged = 0;

    /* Time and step variables */
    double t;
    int step;

    /* Max step */
    int maxSteps;

    /* Variables for clock */
    double time_init, time_final, elapsed_time;

    /* Various variables for MPI implementation */
    int nMpiRanks, nDimensions;
    MPI_Comm comm, comm2d;
    int domains[2];
    int periodicity[2];
    int reorganisation = 0;
    MPI_Datatype column_type;
    int S = 0, E = 1, N = 2, W = 3;
    int neighbors[4];
    int xCell, yCell;
    int *xs, *ys, *xe, *ye;

    /* Physical parameters */
    double temp1_init, temp2_init, k0;

    /* temp1_init: temperature init on borders */
    temp1_init = 10.0;

    /* temp2_init: temperature init inside */
    temp2_init = -10.0;

    /* Diffusivity coefficient */
    k0 = 1;

    /* MPI Initialization */
    MPI_Init(&argc, &argv);
    comm = MPI_COMM_WORLD;
    MPI_Comm_size(comm,&nMpiRanks);
    MPI_Comm_rank(comm,&local_rank);

    /* Assign input parameters to variables */
    size_x    = 128; //atoi(argv[1]);  // 512;
    size_y    = 128; //atoi(argv[2]);  // 512;
    x_domains = 1; //atoi(argv[3]);  // 2;

    cores = 1;
    n_nodes   = 2;
    y_domains = cores*n_nodes; //atoi(argv[4]);  // 2;

    maxSteps  = 10000;
    dt1       = 1.0e-1;
    epsilon   = 1.0e-1;

    /* Warning message if dimensions and number of processes don't match */
    if ((local_rank==0) && (nMpiRanks!=(x_domains*y_domains))) {
        printf("Number of processes not equal to Number of subdomains\n");
    }

    /* Various other variables */
    size_global_x = size_x+2;
    size_global_y = size_y+2;
    hx = 1.0/(double)(size_global_x);
    hy = 1.0/(double)(size_global_y);
    dt2 = 0.25*(min(hx,hy)*min(hx,hy))/k0;
    size_total_x = size_x+2*x_domains+2;
    size_total_y = size_y+2*y_domains+2;

    /* Take a right time step for convergence */
    if (dt1>=dt2) {
        if (local_rank==0) {
            printf("\n  Time step too large, taking convergence criterion.\n");
        }
        dt = dt1;
    } else {
        dt = dt1;
    }

    /* Allocate final 1D array */
    xTempFinal = malloc(size_x*size_y*sizeof(*xTempFinal));

    /* Allocate 2D contiguous arrays x and x0 */
    /* Allocate size_total_x rows */
    x = malloc(size_total_x*sizeof(*x));
    x0 = malloc(size_total_x*sizeof(*x0));
    /* Allocate x[0] and x0[0] for contiguous arrays */
    x[0] = malloc(size_total_x*size_total_y*sizeof(**x));
    x0[0] = malloc(size_total_x*size_total_y*sizeof(**x0));
    /* Loop on rows */
    for (i=1;i<size_total_x;i++) {
        /* Increment size_total_x block on x[i] and x0[i] address */
        x[i] = x[0] + i*size_total_y;
        x0[i] = x0[0] + i*size_total_y;
    }

    /* Allocate coordinates of processes */
    xs = malloc(nMpiRanks*sizeof(int));
    xe = malloc(nMpiRanks*sizeof(int));
    ys = malloc(nMpiRanks*sizeof(int));
    ye = malloc(nMpiRanks*sizeof(int));

    /* Create 2D cartesian grid */
    periodicity[0] = 0;
    periodicity[1] = 0;
    /* Number of dimensions */
    nDimensions = 2;
    /* Invert (Ox,Oy) classic convention */
    domains[0] = y_domains;
    domains[1] = x_domains;
    MPI_Cart_create(comm, nDimensions, domains, periodicity, reorganisation, &comm2d);

    /* Identify neighBors */
    neighbors[0] = MPI_PROC_NULL;
    neighbors[1] = MPI_PROC_NULL;
    neighbors[2] = MPI_PROC_NULL;
    neighbors[3] = MPI_PROC_NULL;

    /* Left/West and Right/East neighBors */
    MPI_Cart_shift(comm2d, 0, 1, &neighbors[W], &neighbors[E]);

    /* Bottom/South and Upper/North neighBors */
    MPI_Cart_shift(comm2d, 1, 1, &neighbors[S], &neighbors[N]);

    /* Number of cells for each domain partition */
    xCell = (size_x/x_domains);
    yCell = (size_y/y_domains);

    /* Allocate subdomain 1D array */
    xTemp = malloc(xCell*yCell*sizeof(*xTemp));

    /* Compute xs, xe, ys, ye for each cell on the grid */
    processToMap(xs, ys, xe, ye, xCell, yCell, x_domains, y_domains);

    /* Create column data type to communicate with East and West neighBors */
    MPI_Type_vector(xCell, 1, size_total_y, MPI_DOUBLE, &column_type);
    MPI_Type_commit(&column_type);

    /* Initialize values */
    initValues(x0, size_total_x, size_total_y, temp1_init, temp2_init);

    /* Update the boundaries */
    updateBoundaries(x0, neighbors, comm2d, column_type, local_rank, xs, ys, xe, ye, yCell);

    /* Initialize step and time */
    step = 0;
    t = 0.0;

    /* Starting time */
    time_init = MPI_Wtime();

    /* Main loop : until convergence */
    while(!converged) {
        /* Increment step and time */
        step = step+1;
        t = t+dt;
        /* Perform one step of the explicit scheme */
        computeNext(x0, x, dt, hx, hy, &localDiff, local_rank, xs, ys, xe, ye, k0);
        /* Update the partial solution along the interface */
        updateBoundaries(x0, neighbors, comm2d, column_type, local_rank, xs, ys, xe, ye, yCell);
        /* Sum reduction to get global difference */
        MPI_Allreduce(&localDiff, &result, 1, MPI_DOUBLE, MPI_SUM, comm);
        /* Current global difference with convergence */
        result= sqrt(result);

        if (local_rank == 0) {
            printf("Completed step %i\n", step);
        }

        /* Break if convergence reached or step greater than maxSteps */
        if (step>maxSteps) {
            /* Ending time */
            time_final = MPI_Wtime();
            /* Elapsed time */
            elapsed_time = time_final - time_init;
            printf("  Wall Clock = %.9f\n", elapsed_time);
            break;
        };
    }

    /* Ending time */
    time_final = MPI_Wtime();
    /* Elapsed time */
    elapsed_time = time_final - time_init;

    /* Gather all subdomains :
      inner loop on columns index (second index)
      to optimize since C is row major */
    j=1;
    for (i=xs[local_rank];i<=xe[local_rank];i++) {
        for (k=0;k<yCell;k++) {
            xTemp[(j - 1) * yCell + k] = x0[i][ys[local_rank] + k];
        }
        j = j + 1;
    }

    /* Perform gathering */
    MPI_Gather(xTemp, xCell*yCell, MPI_DOUBLE, xTempFinal, xCell*yCell, MPI_DOUBLE, 0, comm);

    /* Print results */
    if (local_rank == 0) {
        printf("  Time step = %.9e\n", dt);
        printf("  Convergence = %.9f after %d steps\n", epsilon, step);
        printf("  Problem size = %d\n", size_x * size_y);
        printf("  Wall Clock = %.9f\n", elapsed_time);
        printf("  Computed solution in outputPar.dat\n");

        /* Store solution into output file :
        x_domains = width
        y_domains = height */
        output_file=fopen("output.dat","w");
        for (i=1;i<=size_x+1;i++)
        fprintf(output_file,"%15.11f ",temp1_init);
        fprintf(output_file,"%15.11f\n",temp1_init);
        for (i=1;i<=y_domains;i++)
        for (j=0;j<yCell;j++) {
            fprintf(output_file,"%15.11f ",temp1_init);
            for (k=1;k<=x_domains;k++) {
                for (l=0;l<xCell;l++) {
                    fprintf(output_file, "%15.11f ", xTempFinal[(i - 1) * x_domains * xCell * yCell + (k - 1) * xCell * yCell + l * yCell + j]);
                }
            }
            fprintf(output_file,"%15.11f\n",temp1_init);
        }
        for (i=1;i<=size_x+1;i++)
        fprintf(output_file,"%15.11f ",temp1_init);
        fprintf(output_file,"%15.11f\n",temp1_init);
        fclose(output_file);
    }

    /* Deallocate all arrays */
    free(x[0]);
    free(x);
    free(x0[0]);
    free(x0);
    free(xTemp);
    free(xTempFinal);
    free(xs);
    free(ys);
    free(xe);
    free(ye);

    /* Free column type */
    MPI_Type_free(&column_type);

    /* Finish MPI */
    MPI_Finalize();

    return 0;
}
