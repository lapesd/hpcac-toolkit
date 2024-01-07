import json

import boto3

from hpcac_cli.utils.logger import info


ec2 = boto3.client('ec2')


def get_cluster_details(cluster_config: dict, instance_types: dict) -> dict:
    pricing = boto3.client('pricing', region_name=cluster_config["region"])

    # Paginator for instance types
    paginator = ec2.get_paginator('describe_instance_types')
    page_iterator = paginator.paginate()
    instance_types = {}
    for page in page_iterator:
        for instance_type in page['InstanceTypes']:
            type_name = instance_type['InstanceType']
            vcpus = instance_type['VCpuInfo']['DefaultVCpus']
            memory = instance_type['MemoryInfo']['SizeInMiB']

            # Get EC2 pricing info
            price_filter = [
                {'Type': 'TERM_MATCH', 'Field': 'instanceType', 'Value': type_name},
                {'Type': 'TERM_MATCH', 'Field': 'operatingSystem', 'Value': 'Linux'},
                {'Type': 'TERM_MATCH', 'Field': 'preInstalledSw', 'Value': 'NA'},
                #{'Type': 'TERM_MATCH', 'Field': 'location', 'Value': 'US East (N. Virginia)'},
                {'Type': 'TERM_MATCH', 'Field': 'tenancy', 'Value': 'shared'}
            ]
            price_data = pricing.get_products(ServiceCode='AmazonEC2', Filters=price_filter)
            price_per_hour = "N/A"
            if price_data['PriceList']:
                price_json = json.loads(price_data['PriceList'][0])
                terms = price_json['terms']['OnDemand']
                term_attributes = next(iter(terms.values()))['priceDimensions']
                price_per_hour = next(iter(term_attributes.values()))['pricePerUnit']['USD']

            instance_types[type_name] = {'vcpus': vcpus, 'memory_miB': memory, 'price_per_hour': price_per_hour}

    return instance_types
