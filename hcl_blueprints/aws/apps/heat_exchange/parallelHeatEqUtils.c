#include <mpi.h>


/// Exchange ghost cells between neighbor domains
/// \param x
/// \param neighbors
/// \param comm2d
/// \param column_type
/// \param localRank
/// \param xs
/// \param ys
/// \param xe
/// \param ye
/// \param yCell
void updateBoundaries(double** x, int neighbors[], MPI_Comm comm2d, MPI_Datatype column_type, int localRank, int* xs,
                      int* ys, int* xe, int* ye, int yCell) {
    int S = 0, E = 1, N = 2, W = 3;
    int flag;
    MPI_Status status;
    /****************** North/South communication ******************/
    flag = 1;
    /* Send my boundary to North and receive from South */
    MPI_Sendrecv(&x[xe[localRank]][ys[localRank]], yCell, MPI_DOUBLE, neighbors[N],
                 flag, &x[xs[localRank]-1][ys[localRank]], yCell,MPI_DOUBLE,
                 neighbors[S], flag, comm2d, &status);

    /* Send my boundary to South and receive from North */
    MPI_Sendrecv(&x[xs[localRank]][ys[localRank]], yCell, MPI_DOUBLE, neighbors[S],
                 flag, &x[xe[localRank]+1][ys[localRank]], yCell,MPI_DOUBLE,
                 neighbors[N], flag, comm2d, &status);
    /****************** East/West communication ********************/
    flag = 2;
    /* Send my boundary to East and receive from West */
    MPI_Sendrecv(&x[xs[localRank]][ye[localRank]], 1, column_type, neighbors[E],
                 flag, &x[xs[localRank]][ys[localRank]-1], 1, column_type,
                 neighbors[W], flag, comm2d, &status);

    /* Send my boundary to West and receive from East */
    MPI_Sendrecv(&x[xs[localRank]][ys[localRank]], 1, column_type, neighbors[W],
                 flag, &x[xs[localRank]][ye[localRank]+1], 1, column_type,
                 neighbors[E], flag, comm2d, &status);
}


/// This subroutine computes next values in subdomain of the local MPI rank
/// \param x0
/// \param x
/// \param dt
/// \param hx
/// \param hy
/// \param diff
/// \param localRank
/// \param xs
/// \param ys
/// \param xe
/// \param ye
/// \param k0
void computeNext(double** x0, double** x, double dt, double hx, double hy,
                 double* diff, int localRank, int* xs, int* ys, int* xe, int* ye, double k0) {

    /* Index variables */
    int i, j;

    /* Factors for the stencil */
    double xDiag, yDiag, xWeight, yWeight;

    /* Local variable for computing difference */
    double local_diff;

    /*
      The stencil of the explicit operator for the heat equation
      on a regular rectangular grid using a five point finite difference
      scheme in space is :

      |                                    weightx * x[i-1][j]                                   |
      |                                                                                          |
      | weighty * x[i][j-1]   (diagx * weightx + diagy * weighty) * x[i][j]  weighty * x[i][j+1] |
      |                                                                                          |
      |                                    weightx * x[i+1][j]                                   | */

    xDiag = -2.0 + hx*hx/(2*k0*dt);
    yDiag = -2.0 + hy*hy/(2*k0*dt);
    xWeight = k0*dt/(hx*hx);
    yWeight = k0*dt/(hy*hy);

    /* Perform an explicit update on the points within the domain.
      Optimization : inner loop on columns index (second index) since
      C is row major */
    for (i=xs[localRank];i<=xe[localRank];i++) {
        for (j = ys[localRank]; j <= ye[localRank]; j++) {
            x[i][j] = xWeight * (x0[i - 1][j] + x0[i + 1][j] + x0[i][j] * xDiag) +
                      yWeight * (x0[i][j - 1] + x0[i][j + 1] + x0[i][j] * yDiag);
        }
    }

    /* Compute the difference into domain for convergence.
      Update the value x0(i,j).
      Optimization : inner loop on columns index (second index) since
      C is row major */
    *diff = 0.0;
    for (i=xs[localRank];i<=xe[localRank];i++) {
        for (j = ys[localRank]; j <= ye[localRank]; j++) {
            local_diff = x0[i][j] - x[i][j];
            *diff += local_diff * local_diff;
            x0[i][j] = x[i][j];
        }
    }
}


/// This subroutine sets up the initial temperatures on the domain borders and inner cells
/// \param x0
/// \param size_total_x
/// \param size_total_y
/// \param temp1_init
/// \param temp2_init
void initValues(double** x0, int size_total_x, int size_total_y, double temp1_init, double temp2_init) {
    /* Index variables */
    int i, j;

    /* Setup temp1_init on borders */
    for (i=0;i<=size_total_x-1;i++) {
        x0[i][0] = temp1_init;
        x0[i][size_total_y-1] = temp1_init;
    }

    for (j=0;j<=size_total_y-1;j++) {
        x0[0][j] = temp1_init;
        x0[size_total_x-1][j] = temp1_init;
    }

    for (i=0;i<=size_total_x-2;i++) {
        x0[i][1] = temp1_init;
        x0[i][size_total_y-2] = temp1_init;
    }

    for (j=1;j<=size_total_y-2;j++) {
        x0[1][j] = temp1_init;
        x0[size_total_x-2][j] = temp1_init;
    }

    /* Setup temp2_init inside */
    for (i=2;i<=size_total_x-3;i++) {
        for (j=2;j<=size_total_y-3;j++) {
            x0[i][j] = temp2_init;
        }
    }
}


/// This subroutine computes the coordinates xs, xe, ys, ye, for each cell on the grid, respecting processes topology
/// \param xs
/// \param ys
/// \param xe
/// \param ye
/// \param xCell
/// \param yCell
/// \param x_domains
/// \param y_domains
void processToMap(int *xs, int *ys, int *xe, int *ye, int xCell, int yCell, int x_domains, int y_domains) {
    /* Index variables */
    int i, j;

    /* Computation of starting ys,ye on (Ox) standard axis
      for the first column of global domain,
      Convention x(i,j) with i row and j column */
    for (i=0;i<x_domains;i++) {
        ys[i] = 2;
        /* Here, ye(0:(x_domains-1)) = 2+yCell-1 */
        ye[i] = ys[i]+yCell-1;
    }

    /* Computation of ys,ye on (Ox) standard axis
      for all other cells of global domain */
    for (i=1;i<y_domains;i++) {
        for (j = 0; j < x_domains; j++) {
            ys[i * x_domains + j] = ys[(i - 1) * x_domains + j] + yCell + 2;
            ye[i * x_domains + j] = ys[i * x_domains + j] + yCell - 1;
        }
    }

    /* Computation of starting xs,xe on (Oy) standard axis
      for the first row of global domain,
      Convention x(i,j) with i row and j column */
    for (i=0;i<y_domains;i++) {
        xs[i*x_domains] = 2;
        /* Here, xe(i*x_domains) = 2+xCell-1 */
        xe[i*x_domains] = xs[i*x_domains]+xCell-1;
    }

    /* Computation of xs,xe on (Oy) standard axis
      for all other cells of global domain */
    for (i=1;i<=y_domains;i++) {
        for (j=1;j<x_domains;j++) {
            xs[(i-1)*x_domains+j] = xs[(i-1)*x_domains+(j-1)]+xCell+2;
            xe[(i-1)*x_domains+j] = xs[(i-1)*x_domains+j]+xCell-1;
        }
    }
}
