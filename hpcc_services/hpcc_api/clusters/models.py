from django.db import models


class ClusterConfiguration(models.Model):
    CLOUD_PROVIDER_CHOICES = [
        ("aws", "Amazon Web Services"),
        ("aws-spot", "Spot Amazon Web Services"),
        ("vultr", "Vultr"),
        ("aws-FSxL", 'FSx-Lustre'),
    ]

    DEFAULTS = {
        "aws": {
            "region": "us-east-1",
            "availability_zone": "us-east-1a",
            "master_ami": "ami-0c88d865df36afa1f",
            "master_ebs": 10,
            "master_rbs": 10,
            "master_instance_type": "t2.micro",
            "worker_count": 1,
            "worker_ami": "ami-0c88d865df36afa1f",
            "worker_ebs": 10,
            "worker_rbs": 10,
            "worker_instance_type": "t2.micro",
        },
        "aws-spot": {
            "region": "us-east-1",
            "availability_zone": "us-east-1a",
            "master_ami": "ami-0c88d865df36afa1f",
            "master_ebs": 10,
            "master_rbs": 10,
            "master_instance_type": "t2.micro",
            "worker_count": 1,
            "worker_ami": "ami-0c88d865df36afa1f",
            "worker_ebs": 10,
            "worker_rbs": 10,
            "worker_instance_type": "t2.micro",
            "worker_spot_price": 0.5,
        },
        "vultr": {},
        "aws-FSxL": {
            "region": "us-east-1",
            "availability_zone": "us-east-1a",
            "master_ami": "ami-0c88d865df36afa1f",
            "master_fsx": 10,
            "master_rbs": 10,
            "master_instance_type": "t2.micro",
            "worker_count": 1,
            "worker_ami": "ami-0c88d865df36afa1f",
            "worker_fsx": 10,
            "worker_rbs": 10,
            "worker_instance_type": "t2.micro",
        },
    }

    label = models.TextField(
        unique=True,
        null=False,
        blank=False,
    )
    cloud_provider = models.CharField(
        max_length=10,
        choices=CLOUD_PROVIDER_CHOICES,
        null=False,
    )
    minio_bucket_name = models.TextField(
        null=False,
        blank=False,
    )

    def __str__(self):
        return f"ClusterConfig {self.id}"
