import json

import boto3


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
