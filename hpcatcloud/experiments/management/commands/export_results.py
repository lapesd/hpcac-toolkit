import io
import sys
import csv

from django.core.management.base import BaseCommand

from hpcatcloud.experiments.models import MPIExperiment


class Command(BaseCommand):
    help = "Export MPI workload runs results."

    def print_success(self, message):
        self.stdout.write(self.style.SUCCESS(message))

    def print_error(self, message):
        self.stdout.write(self.style.ERROR(message))

    def handle(self, *args, **options):
        try:
            # Compile target application
            self.print_success(f"Exporting saved MPI jobs in CSV format...")

            csv_results_file_path = "./exported_results.csv"

            with open(csv_results_file_path, mode='w', newline='') as file:
                writer = csv.writer(file)

                # Write the header
                writer.writerow([
                    'Label', 'Launched At', 'Completed At', 'Cluster Size',
                    'Cluster Has EFA', 'Cluster Has FSX', 'Cluster Is Ephemeral',
                    'Cluster Instance Type', 'FT Technology', 'CKPT Strategy',
                    'Number Of Failures', 'Job Successfully Completed',
                    'Time Spent Spawning Cluster', 'Time Spent Setting Up Job',
                    'Time Spent Checkpointing', 'Time Spent Restoring Cluster',
                    'Time Spent Executing', 'Total Time Spent'
                ])

                # Write the data rows
                for experiment in MPIExperiment.objects.all():
                    writer.writerow([
                        experiment.label, experiment.launched_at, experiment.completed_at, experiment.cluster_size,
                        experiment.cluster_has_efa, experiment.cluster_has_fsx, experiment.cluster_is_ephemeral,
                        experiment.cluster_instance_type, experiment.ft_technology, experiment.ckpt_strategy,
                        experiment.number_of_failures, experiment.job_successfully_completed,
                        experiment.time_spent_spawning_cluster, experiment.time_spent_setting_up_job,
                        experiment.time_spent_checkpointing, experiment.time_spent_restoring_cluster,
                        experiment.time_spent_executing, experiment.total_time_spent
                    ])

            self.print_success(f"MPI jobs exported successfully to {csv_results_file_path}")

        except Exception as error:
            self.print_error(f"CommandError: {error}")
            sys.exit(1)
