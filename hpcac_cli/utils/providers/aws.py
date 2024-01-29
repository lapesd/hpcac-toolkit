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


def get_cluster_nodes_ip_addresses(
    cluster_tag: str, number_of_nodes: int, region: str
) -> list[str]:
    # TODO: check if this function works with spot instances
    ec2 = boto3.client("ec2", region_name=region)
    filters = [{"Name": "tag:cost_allocation_tag", "Values": [cluster_tag]}]

    while True:
        dangling_instances = False
        ip_addresses = []

        response = ec2.describe_instances(Filters=filters)
        for reservation in response["Reservations"]:
            log.debug(
                f"boto3 ec2 client response: ```\n{reservation}\n```",
                detail="get_cluster_nodes_ip_addresses",
            )
            if len(reservation["Instances"]) != 1:
                raise Exception(
                    "Malformed response in `get_cluster_nodes_ip_addresses`"
                )
            instance = reservation["Instances"][0]
            instance_ip = instance.get("PublicIpAddress", None)
            instance_state = instance["State"]["Name"]
            if instance_state == "running" and instance_ip is not None:
                log.debug(
                    f"Detected AWS Instance: `{instance_ip}` with State: `{instance_state}`",
                    detail="get_cluster_nodes_ip_addresses",
                )
                ip_addresses.append(instance_ip)
            elif instance_state == "terminated":
                log.debug(
                    f"Detected AWS Instance: `{instance_ip}` with State: `{instance_state}`",
                    detail="get_cluster_nodes_ip_addresses",
                )
            else:
                log.warning(
                    f"Detected BAD AWS Instance in state `{instance_state}`",
                    detail="get_cluster_nodes_ip_addresses",
                )
                dangling_instances = True

        if len(ip_addresses) == number_of_nodes and not dangling_instances:
            return ip_addresses
        else:
            log.warning(
                f"AWS Instances not ready yet, sleeping for 10s and retrying..."
            )
            time.sleep(10)
