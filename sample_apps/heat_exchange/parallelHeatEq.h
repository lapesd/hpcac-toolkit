void processToMap(int *xs, int *ys, int *xe, int *ye, int xCell, int yCell, int x_domains, int y_domains);

void initValues(double** x0, int size_x, int size_y, double temp1, double temp2);

void updateBoundaries(double** x, int neighbors[], MPI_Comm comm, MPI_Datatype datatype, int me, int* xs, int* ys, int* xe, int* ye, int yCell);

void computeNext(double** x0, double** x, double dt, double hx, double hy, double* diff, int me, int* xs, int* ys, int* xe, int* ye, double k0);
