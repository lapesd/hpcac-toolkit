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

    def handle(self, *args, **options):
        destroy_cluster()

        self.stdout.write(
            self.style.SUCCESS(f"Successfully DESTROYED all created cloud resources.")
        )
