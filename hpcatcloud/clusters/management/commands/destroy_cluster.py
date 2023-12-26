import subprocess

from django.core.management.base import BaseCommand


def destroy_cluster():
    tf_dir = "./tmp_terraform_dir"

    subprocess.run(
        ["terraform", "destroy", "-auto-approve"],
        cwd=tf_dir,
        check=True,
    )


class Command(BaseCommand):
    help = "Spawns a Cluster from a previously created ClusterConfiguration."

    def print_success(self, message):
        self.stdout.write(self.style.SUCCESS(message))

    def print_error(self, message):
        self.stdout.write(self.style.ERROR(message))

    def handle(self, *args, **options):
        destroy_cluster()

        self.print_error(f"All cluster cloud resources were deleted.")
