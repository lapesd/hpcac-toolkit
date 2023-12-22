from django.db import models


class Node(models.Model):
    LOCAL_STORAGE_TYPE_CHOICES = [
        # See AWS docs: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/ebs-volume-types.html
        ("gp2", "General Purpose SSD v2"),
        ("gp3", "General Purpose SSD v3"),
        ("io2", "Provisioned IOPS SSD"),
    ]
    NETWORK_ADAPTER_TYPE_CHOICES = [
        ("ena", "Elastic Network Adapter"),
        ("efa", "Elastic Fabric Adapter"),
    ]
    INSTANCE_TYPE_CHOICES = [

    ]

    label = models.TextField(
        unique=True,
        null=False,
        blank=False,
    )
    public_ip = models.GenericIPAddressField(
        null=True,
    )
    private_ip = models.GenericIPAddressField(
        null=True,
    )
    is_healthy = models.BooleanField(
        default=False,
        null=False,
    )
    latest_health_status_at = models.DateTimeField(
        auto_now_add=True,
    )
    is_ephemeral = models.BooleanField(
        default=False,
        null=False,
    )
    vcpus = models.IntegerField(
        null=False,
    )
    volatile_memory = models.IntegerField(
        null=False,
    )
    persistent_local_storage_size = models.IntegerField(
        default=10,
        null=False,
    )
    persistent_local_storage_type = models.CharField(
        max_length=10,
        choices=LOCAL_STORAGE_TYPE_CHOICES,
        null=False,
    )
    network_adapter_type = models.CharField(
        max_length=10,
        choices=NETWORK_ADAPTER_TYPE_CHOICES,
        null=False, 
    )


class Cluster(models.Model):
    PROVIDER_CHOICES = [
        ("aws", "Amazon Web Services"),
    ]

    label = models.TextField(
        unique=True,
        null=False,
        blank=False,
    )
    provider = models.CharField(
        max_length=10,
        choices=PROVIDER_CHOICES,
        null=False,
    )
    node_to_node_bandwidth = models.IntegerField(
        default=None,
        null=True,
    )
    node_to_node_latency = models.IntegerField(
        default=None,
        null=True,
    )
    shared_storage_path = models.TextField(
        default="",
        null=False,
        blank=True,
    )
    spawn_time = models.IntegerField(
        null=True,
    )

    def __str__(self):
        return f"Cluster {self.id}"


class ClusterConfiguration(models.Model):
    CLOUD_PROVIDER_CHOICES = [
        ("aws", "Amazon Web Services"),
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
            "use_spot": False,
            "use_nfs": True,
            "use_fsx": False,
            "use_efa": False,
        },
        "vultr": {
            # TODO review and add Vultr base params
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
    nodes = models.IntegerField(
        null=False,
    )
    transient = models.BooleanField(
        default=False,
        null=False,
    )
    fsx = models.BooleanField(
        default=False,
        null=False,
    )
    nfs = models.BooleanField(
        default=False,
        null=False,
    )
    efa = models.BooleanField(
        default=False,
        null=False,
    )
    username = models.TextField(
        default="ec2-user",
        null=False,
        blank=False,
    )
    entrypoint_ip = models.GenericIPAddressField(
        null=True,
    )
    minio_bucket_name = models.TextField(
        null=False,
        blank=False,
    )
    vcpus = models.IntegerField(
        null=False,
    )
    spawn_time = models.IntegerField(
        default=0,
    )

    def __str__(self):
        return f"ClusterConfig {self.id}"
