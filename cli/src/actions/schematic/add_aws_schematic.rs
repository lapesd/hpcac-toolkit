use std::process::exit;

use colored::Colorize;
use inquire::{Confirm, Select, Text};
use sqlx::PgPool;
use uuid::Uuid;

use hpcac_core::models::{
    aws_availability_zone::AwsAvailabilityZone, aws_cluster_schematic::AwsClusterSchematic,
    aws_instance_types::AwsMachineType, aws_machine_image::AwsMachineImage,
};

async fn prompt_for_schematic_name() -> String {
    loop {
        match Text::new("New AWS cluster schematic name:").prompt() {
            Ok(alias) => {
                if alias.chars().count() != 0 {
                    return alias;
                } else {
                    println!(
                        "{}: Schematic name can't be empty.\n",
                        "TRY AGAIN".yellow().bold()
                    )
                }
            }
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        }
    }
}

async fn prompt_for_schematic_description() -> String {
    loop {
        match Text::new("New AWS cluster schematic description:").prompt() {
            Ok(description) => {
                if description.chars().count() != 0 {
                    return description;
                } else {
                    println!(
                        "{}: Schematic description can't be empty.\n",
                        "TRY AGAIN".yellow().bold()
                    )
                }
            }
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        }
    }
}

async fn prompt_for_aws_availability_zone(pool: &PgPool) -> String {
    let aws_az_options: Vec<String> = match AwsAvailabilityZone::fetch_all(pool).await {
        Ok(azs) => azs.iter().map(|az| az.code.to_owned()).collect(),
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    };
    loop {
        match Select::new("Select the desired AWS Availability Zone:", aws_az_options).prompt() {
            Ok(az) => return az,
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        }
    }
}

async fn prompt_for_vcpus_lower_limit_filter() -> i32 {
    loop {
        match Text::new("What's the minimum amount of vCPUs to allocate for each worker node?")
            .prompt()
        {
            Ok(vcpus) => match vcpus.parse::<i32>() {
                Ok(vcpus) => {
                    if vcpus >= 1 {
                        return vcpus;
                    } else {
                        println!(
                            "{}: Lower vCPU limit must be an integer larger or equal than 1.\n",
                            "TRY AGAIN".yellow().bold()
                        )
                    }
                }
                Err(_) => {
                    println!(
                        "{}: Lower vCPU limit must be an integer larger or equal than 1.\n",
                        "TRY AGAIN".yellow().bold()
                    )
                }
            },
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        }
    }
}

async fn prompt_for_memory_lower_limit_filter() -> f64 {
    loop {
        match Text::new("What's the minimum amount of RAM memory to allocate for each worker node?")
            .prompt()
        {
            Ok(memory) => {
                match memory.parse::<f64>() {
                    Ok(memory) => {
                        if memory >= 0.5 {
                            return memory;
                        } else {
                            println!("{}: Lower memory limit must be a float larger or equal than 0.5.\n", "TRY AGAIN".yellow().bold())
                        }
                    }
                    Err(_) => {
                        println!(
                            "{}: Lower memory limit must be a float larger or equal than 0.5.\n",
                            "TRY AGAIN".yellow().bold()
                        )
                    }
                }
            }
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        }
    }
}

async fn prompt_for_aws_instance_type(pool: &PgPool, vcpus: i32, memory: f64) -> String {
    let flavor_options: Vec<String> =
        match AwsMachineType::filter_by_minimum_resources(pool, vcpus, memory).await {
            Ok(flavors) => flavors.iter().map(|flavor| flavor.describe()).collect(),
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        };
    loop {
        match Select::new("Select the desired AWS instance type:", flavor_options).prompt() {
            Ok(flavor_with_description) => {
                let instance_type: Vec<&str> = flavor_with_description.split(" ").collect();
                return instance_type[0].to_string();
            }
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        }
    }
}

async fn prompt_for_worker_count() -> i32 {
    loop {
        match Text::new("Type in the number of worker nodes for the cluster:").prompt() {
            Ok(worker_count) => match worker_count.parse::<i32>() {
                Ok(worker_count) => {
                    if worker_count >= 1 {
                        return worker_count;
                    } else {
                        println!(
                            "{}: Worker count must be an integer larger or equal than 1.\n",
                            "TRY AGAIN".yellow().bold()
                        )
                    }
                }
                Err(_) => {
                    println!(
                        "{}: Worker count must be an integer larger or equal than 1.\n",
                        "TRY AGAIN".yellow().bold()
                    )
                }
            },
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        }
    }
}

async fn prompt_for_aws_machine_image(pool: &PgPool) -> String {
    let aws_ami_options: Vec<String> = match AwsMachineImage::fetch_all(pool).await {
        Ok(amis) => amis.iter().map(|ami| ami.describe()).collect(),
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    };
    loop {
        match Select::new(
            "Select the base Amazon Machine Image (AMI):",
            aws_ami_options,
        )
        .prompt()
        {
            Ok(ami_with_decription) => {
                let ami: Vec<&str> = ami_with_decription.split(" ").collect();
                return ami[0].to_string();
            }
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        }
    }
}

async fn prompt_for_nfs_install() -> bool {
    match Confirm::new("Install a Network File System (NFS) for the cluster?")
        .with_default(true)
        .prompt()
    {
        Ok(nfs) => nfs,
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    }
}

async fn prompt_for_worker_ebs() -> i32 {
    match Select::new(
        "Select the desired Elastic Block Storage (EBS) size in GBs to attach to each worker node:",
        vec![10, 100],
    )
    .prompt()
    {
        Ok(worker_ebs_size) => worker_ebs_size.to_owned(),
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    }
}

async fn prompt_for_spot_cluster() -> bool {
    match Confirm::new("Request SPOT workers?")
        .with_default(false)
        .prompt()
    {
        Ok(spot_cluster) => spot_cluster,
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    }
}

async fn prompt_for_blcr_support() -> bool {
    match Confirm::new("Add BLCR support?")
        .with_default(false)
        .prompt()
    {
        Ok(support) => support,
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    }
}

async fn prompt_for_criu_support() -> bool {
    match Confirm::new("Add CRIU support?")
        .with_default(false)
        .prompt()
    {
        Ok(support) => support,
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    }
}

async fn prompt_for_ulfm_support() -> bool {
    match Confirm::new("Add ULFM support?")
        .with_default(false)
        .prompt()
    {
        Ok(support) => support,
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    }
}

pub async fn action(pool: &PgPool) {
    let az = prompt_for_aws_availability_zone(pool).await;
    let spot_cluster = prompt_for_spot_cluster().await;

    let min_vcpus = prompt_for_vcpus_lower_limit_filter().await;
    let min_memory = prompt_for_memory_lower_limit_filter().await;
    let instance_type = prompt_for_aws_instance_type(pool, min_vcpus, min_memory).await;
    let worker_count = prompt_for_worker_count().await;
    let workers_ami = prompt_for_aws_machine_image(pool).await;
    let workers_ebs = prompt_for_worker_ebs().await;

    let nfs_support = prompt_for_nfs_install().await;

    let blcr_support = if spot_cluster {
        prompt_for_blcr_support().await
    } else {
        false
    };
    let criu_support = if spot_cluster {
        prompt_for_criu_support().await
    } else {
        false
    };
    let ulfm_support = if spot_cluster {
        prompt_for_ulfm_support().await
    } else {
        false
    };

    let alias = prompt_for_schematic_name().await;
    let description = prompt_for_schematic_description().await;

    let schematic = AwsClusterSchematic {
        uuid: Uuid::new_v4(),
        alias: alias.clone(),
        description,
        az,
        master_ami: workers_ami.clone(),
        master_flavor: instance_type.clone(),
        master_ebs: 10,
        spot_cluster,
        worker_count,
        workers_ami,
        workers_flavor: instance_type,
        workers_ebs,
        nfs_support,
        criu_support,
        blcr_support,
        ulfm_support,
    };
    match schematic.insert(pool).await {
        Ok(_) => {
            println!("Cluster schematic `{}` created! Use `hpcc schematic ls` to list all available schematics.", alias.green());
        }
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    };
}
