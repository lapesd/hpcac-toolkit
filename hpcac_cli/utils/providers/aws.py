import json
import time
from typing import Optional

import boto3

from hpcac_cli.utils.logger import Logger


log = Logger()


async def get_instance_type_details(
    instance_type: str, region: str = "us-east-1"
) -> dict:
    ec2 = boto3.client("ec2", region_name=region)
    pricing = boto3.client("pricing", region_name=region)

    # Describe the specific instance type
    response = ec2.describe_instance_types(InstanceTypes=[instance_type])
    instance_details = response["InstanceTypes"][0]
    vcpus = instance_details["VCpuInfo"]["DefaultVCpus"]
    memory = instance_details["MemoryInfo"]["SizeInMiB"]

    # Function to get pricing
    def get_price(instance_type, os="Linux", service="AmazonEC2", term="OnDemand"):
        price_filter = [
            {"Type": "TERM_MATCH", "Field": "instanceType", "Value": instance_type},
            {"Type": "TERM_MATCH", "Field": "operatingSystem", "Value": os},
            {"Type": "TERM_MATCH", "Field": "preInstalledSw", "Value": "NA"},
            {"Type": "TERM_MATCH", "Field": "tenancy", "Value": "shared"},
            {
                "Type": "TERM_MATCH",
                "Field": "location",
                "Value": "US East (N. Virginia)",
            },
        ]
        price_data = pricing.get_products(ServiceCode=service, Filters=price_filter)
        if price_data["PriceList"]:
            price_json = json.loads(price_data["PriceList"][0])
            terms = price_json["terms"][term]
            term_attributes = next(iter(terms.values()))["priceDimensions"]
            return next(iter(term_attributes.values()))["pricePerUnit"]["USD"]
        return "N/A"

    # Get On-Demand price
    on_demand_price = get_price(instance_type)

    # Get Spot price (Note: Spot price varies frequently. This is a more complex task and might not be as straightforward)
    # For simplicity, we are not including spot pricing here. It's generally retrieved from EC2 Spot Price history.

    return {
        "vcpus_per_node": vcpus,
        "memory_per_node": memory,
        "on_demand_price_per_hour": on_demand_price,
        # 'spot_price_per_hour': spot_price  # Spot price retrieval can be added here
    }


def get_cluster_efs_dns_name(cluster_tag: str, region: str) -> Optional[str]:
    log.debug(
        text=f"Searching for EFS with cluster tag `{cluster_tag}` in region `{region}`...",
        detail="get_cluster_efs_dns_name",
    )
    efs_client = boto3.client("efs", region_name=region)
    file_systems = efs_client.describe_file_systems()
    for fs in file_systems["FileSystems"]:
        tags = efs_client.describe_tags(FileSystemId=fs["FileSystemId"])["Tags"]
        if any(tag["Value"] == cluster_tag for tag in tags):
            log.debug(
                text=f"EFS with cluster tag `{cluster_tag}` found: {fs['FileSystemId']}",
                detail="get_cluster_efs_dns_name",
            )

            efs_id = fs["FileSystemId"]
            efs_state = fs["LifeCycleState"]
            while efs_state != "available":
                log.warning(
                    f"Couldn't reach the Cluster EFS for some reason, retrying in 10s...",
                    detail="get_cluster_efs_dns_name",
                )
                time.sleep(10)
                efs_state = efs_client.describe_file_systems(FileSystemId=efs_id)[
                    "FileSystems"
                ][0]["LifeCycleState"]

            dns_name = f"{efs_id}.efs.{region}.amazonaws.com"
            log.debug(
                text=f"Amazon EFS `{dns_name}` is ready!",
                detail="get_cluster_efs_dns_name",
            )
            return dns_name

    return None


def get_running_nodes_ips(cluster) -> list[str]:
    ec2 = boto3.client("ec2", region_name=cluster.region)
    filters = [
        {"Name": "tag:cost_allocation_tag", "Values": [cluster.cluster_tag]},
        {"Name": "instance-state-code", "Values": ["16"]}
    ]

    cluster_ips = []

    response = ec2.describe_instances(Filters=filters)
    for reservation in response["Reservations"]:
        log.debug(
            f"boto3 ec2 client response: ```\n{reservation}\n```",
            detail="get_running_nodes_ips",
        )
        if len(reservation["Instances"]) != 1:
            raise Exception(
                "Malformed response in `get_running_nodes_ips`"
            )
        instance = reservation["Instances"][0]
        instance_ip = instance.get("PublicIpAddress", None)
        instance_state = instance["State"]["Name"]
        log.info(
            f"Detected AWS Instance: `{instance_ip}` with State: `{instance_state}`",
        )
        if instance_state == "running" and instance_ip is not None:
            cluster_ips.append(instance_ip)

    return cluster_ips
