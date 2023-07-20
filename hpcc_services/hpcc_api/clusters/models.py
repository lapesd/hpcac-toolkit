from django.db import models


class ClusterConfiguration(models.Model):
    CLOUD_PROVIDER_CHOICES = [
        ("aws", "Amazon Web Services"),
        ("aws-spot", "Spot Amazon Web Services"),
        ("vultr", "Vultr"),
    ]

    DEFAULTS = {
        "aws": {
            "region": "us-east-1",
            "availability_zone": "us-east-1a",
            "master_ami": "ami-0c88d865df36afa1f",
            "master_rbs_size": 10,
            "master_rbs_type": "io1",
            "master_rbs_iops": 150,
            "master_instance_type": "t2.micro",
            "worker_count": 1,
            "worker_ami": "ami-0c88d865df36afa1f",
            "worker_rbs_size": 10,
            "worker_rbs_type": "io1",
            "worker_rbs_iops": 150,
            "worker_instance_type": "t2.micro",
            "experiment_tag": "generic-test",
            "instance_username": "ec2-user",
        },
        "aws-spot": {
            "region": "us-east-1",
            "availability_zone": "us-east-1a",
            "master_ami": "ami-0c88d865df36afa1f",
            "master_rbs_size": 10,
            "master_rbs_type": "io1",
            "master_rbs_iops": 150,
            "master_instance_type": "t2.micro",
            "worker_count": 1,
            "worker_ami": "ami-0c88d865df36afa1f",
            "worker_rbs_size": 10,
            "worker_rbs_type": "io1",
            "worker_rbs_iops": 150,
            "worker_instance_type": "t2.micro",
            "worker_spot_price": 0.5,
            "experiment_tag": "generic-test",
            "instance_username": "ec2-user",
        },
        "vultr": {},
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
