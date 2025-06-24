use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_elastic_ip(
        &self,
        context: &AwsClusterContext,
        node_index: usize,
    ) -> Result<String> {
        let eip_name = context.elastic_ip_name(node_index);

        let describe_elastic_ips_response = match context
            .ec2_client
            .describe_addresses()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:Name")
                    .values(&eip_name)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("Failed to describe Elastic IP '{}': {:?}", eip_name, e);
                bail!("Failure describing Elastic IP resources");
            }
        };

        for address in describe_elastic_ips_response.addresses() {
            if let Some(eip_id) = address.allocation_id() {
                info!("Found existing Elastic IP '{}': '{}'", eip_name, eip_id);
                return Ok(eip_id.to_string());
            }
        }

        info!(
            "Allocating new Elastic IP '{}' for Node {}...",
            eip_name, node_index
        );

        let create_elastic_ip_response = match context
            .ec2_client
            .allocate_address()
            .domain(aws_sdk_ec2::types::DomainType::Vpc)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::ElasticIp)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(&eip_name)
                            .build(),
                    )
                    .tags(context.cluster_id_tag.clone())
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failed to allocate Elastic IP '{}'", eip_name);
            }
        };

        if let Some(allocation_id) = create_elastic_ip_response.allocation_id() {
            info!(
                "Allocated new Elastic IP '{}' with allocation ID '{}'",
                eip_name, allocation_id
            );
            return Ok(allocation_id.to_string());
        }

        warn!("{:?}", create_elastic_ip_response);
        bail!("Failure finding the id of the created Elastic IP resource");
    }

    pub async fn cleanup_elastic_ip(
        &self,
        context: &AwsClusterContext,
        node_index: usize,
    ) -> Result<()> {
        let eip_name = context.elastic_ip_name(node_index);

        let describe_elastic_ips_response = match context
            .ec2_client
            .describe_addresses()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:Name")
                    .values(&eip_name)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing Elastic IP resources");
            }
        };

        let mut allocation_id = None;
        for address in describe_elastic_ips_response.addresses() {
            if let Some(alloc_id) = address.allocation_id() {
                allocation_id = Some(alloc_id.to_string());
                break;
            }
        }

        let allocation_id = match allocation_id {
            Some(id) => id,
            None => {
                info!("No Elastic IP found");
                return Ok(());
            }
        };

        info!(
            "Found Elastic IP to cleanup: '{}' (allocation identifier: '{}')",
            eip_name, allocation_id
        );

        let describe_eip_allocation_response = match context
            .ec2_client
            .describe_addresses()
            .allocation_ids(&allocation_id)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failure describing Elastic IP '{}' allocation identifier",
                    eip_name
                );
            }
        };

        for address in describe_eip_allocation_response.addresses() {
            if let Some(association_id) = address.association_id() {
                info!(
                    "Disassociating Elastic IP '{}' (association identifier: '{}')",
                    eip_name, association_id
                );

                match context
                    .ec2_client
                    .disassociate_address()
                    .association_id(association_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("Successfully disassociated Elastic IP '{}'", eip_name);
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        bail!(
                            "Failed to disassociate Elastic IP '{}': (association identifier: '{}')",
                            eip_name,
                            association_id
                        );
                    }
                }
            }
        }

        info!(
            "Releasing Elastic IP '{}' (allocation identifier: '{}')...",
            eip_name, allocation_id
        );

        match context
            .ec2_client
            .release_address()
            .allocation_id(&allocation_id)
            .send()
            .await
        {
            Ok(_) => {
                info!("Successfully released Elastic IP '{}'", eip_name);
                Ok(())
            }
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure releasing Elastic IP '{}'", eip_name);
            }
        }
    }

    pub async fn associate_elastic_ip_with_network_interface(
        &self,
        context: &AwsClusterContext,
        eip_id: &str,
        eni_id: &str,
    ) -> Result<String> {
        let describe_eip_response = match context
            .ec2_client
            .describe_addresses()
            .allocation_ids(eip_id)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("Failed to describe Elastic IP '{}': {:?}", eip_id, e);
                bail!("Failed to describe Elastic IP '{}'", eip_id);
            }
        };

        let mut public_ip = None;
        for address in describe_eip_response.addresses() {
            if let Some(ip) = address.public_ip() {
                public_ip = Some(ip.to_string());
            }

            if let Some(associated_eni_id) = address.network_interface_id() {
                if associated_eni_id == eni_id {
                    let ip_addr = public_ip.as_deref().unwrap();
                    info!(
                        "Elastic IP '{}' ({}) is already associated with Elastic Network Interface '{}'",
                        eip_id, ip_addr, eni_id
                    );
                    return Ok(public_ip.unwrap());
                }
            }
        }

        let public_ip = public_ip.ok_or_else(|| {
            anyhow::anyhow!("Could not find public IP for Elastic IP '{}'", eip_id)
        })?;

        info!(
            "Associating Elastic IP '{}' ({}) with Network Interface '{}'...",
            eip_id, public_ip, eni_id
        );

        let associate_eip_with_eni_response = match context
            .ec2_client
            .associate_address()
            .allocation_id(eip_id)
            .network_interface_id(eni_id)
            .allow_reassociation(true)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failed to associate Elastic IP '{}' ({}) with Elastic Network Interface '{}'",
                    eip_id,
                    public_ip,
                    eni_id
                );
            }
        };

        if let Some(association_id) = associate_eip_with_eni_response.association_id() {
            info!(
                "Successfully associated Elastic IP '{}' ({}) with Network Interface '{}' (association ID: '{}')",
                eip_id, public_ip, eni_id, association_id
            );
            return Ok(public_ip);
        }

        bail!(
            "No association id returned for Elastic IP association with Elastic Network Interface"
        );
    }
}
