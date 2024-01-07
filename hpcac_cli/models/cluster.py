from tortoise.models import Model
from tortoise import fields


class Cluster(Model):
    id = fields.IntField(pk=True)
    instance_type = fields.CharField(max_length=50)
    nodes = fields.IntField()
    is_transient = fields.BooleanField(default=False)
    use_efs = fields.BooleanField(default=True)
    use_fsx = fields.BooleanField(default=False)

    def __str__(self):
        return f"Cluster: {self.nodes}x {self.instance_type}"


async def create_user():
    cluster = await Cluster.create(instance_type="test")
    print(f"Created Cluster: {cluster}")
