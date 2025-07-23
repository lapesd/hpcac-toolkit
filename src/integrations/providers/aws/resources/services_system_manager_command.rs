use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use std::collections::HashMap;

use anyhow::{Result, bail};
use aws_sdk_ssm::error::SdkError;
use aws_sdk_ssm::types::{
    CommandInvocationStatus, InstanceInformationFilter, InstanceInformationFilterKey, PingStatus,
};
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn wait_for_ssm_agent_ready(
        &self,
        context: &AwsClusterContext,
        instance_id: &str,
        max_wait_time: Duration,
    ) -> Result<()> {
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() > max_wait_time {
                bail!(
                    "SSM agent readiness check timed out for instance '{}'",
                    instance_id
                );
            }

            // Check if instance is managed by SSM
            let response = context
                .ssm_client
                .describe_instance_information()
                .instance_information_filter_list(
                    InstanceInformationFilter::builder()
                        .key(InstanceInformationFilterKey::InstanceIds)
                        .value_set(instance_id)
                        .build()?,
                )
                .send()
                .await?;

            if !response.instance_information_list().is_empty() {
                let instance_info = &response.instance_information_list()[0];
                if instance_info.ping_status() == Some(&PingStatus::Online) {
                    info!("SSM agent is ready for instance '{}'", instance_id);
                    return Ok(());
                }
            }

            info!(
                "Waiting for SSM agent to be ready for instance '{}'...",
                instance_id
            );
            sleep(Duration::from_secs(10)).await;
        }
    }

    pub async fn create_ssm_command(
        &self,
        context: &AwsClusterContext,
        ec2_instance_id: &str,
        command: String,
    ) -> Result<String> {
        info!(
            "Creating SSM Command for EC2 instance (id='{}')...",
            ec2_instance_id,
        );

        // Wrap command in shell script and run as ec2-user
        let wrapped_command = format!(
            r#"#!/bin/bash
set -e
sudo -u ec2-user -i bash << 'EOF'
{}
EOF"#,
            command
        );

        info!("SSM Command: `{}`", wrapped_command);

        let mut parameters = HashMap::new();
        parameters.insert("commands".to_string(), vec![wrapped_command]);

        let max_retries = 5;
        let base_delay = Duration::from_secs(30);

        for attempt in 1..=max_retries {
            info!(
                "SSM command creation attempt {} of {}",
                attempt, max_retries
            );

            let result = context
                .ssm_client
                .send_command()
                .instance_ids(ec2_instance_id)
                .document_name("AWS-RunShellScript")
                .set_parameters(Some(parameters.clone()))
                .send()
                .await;

            match result {
                Ok(response) => {
                    let ssm_command_id = match response.command() {
                        Some(cmd) => match cmd.command_id() {
                            Some(id) => id,
                            None => {
                                error!("AWS SDK client response: '{:?}'", response);
                                bail!(
                                    "Unexpected response from AWS when sending SSM Command to EC2 Instance (id='{}')",
                                    ec2_instance_id
                                );
                            }
                        },
                        None => {
                            error!("AWS SDK client response: '{:?}'", response);
                            bail!(
                                "Unexpected response from AWS when sending SSM Command to EC2 Instance (id='{}')",
                                ec2_instance_id
                            );
                        }
                    };

                    info!(
                        "Successfully created SSM Command (id='{}') for EC2 Instance (id='{}') on attempt {}",
                        ssm_command_id, ec2_instance_id, attempt
                    );

                    info!("Waiting for command invocation to become available...");
                    sleep(Duration::from_secs(5)).await;

                    return Ok(ssm_command_id.to_string());
                }
                Err(aws_error) => {
                    let should_retry = matches!(&aws_error, SdkError::ServiceError(_));

                    if should_retry && attempt < max_retries {
                        let delay = base_delay * attempt as u32;
                        warn!(
                            "SSM command creation failed on attempt {}. Retrying in {:?}...",
                            attempt, delay
                        );
                        warn!("Error details: {:?}", aws_error);
                        sleep(delay).await;
                        continue;
                    } else {
                        error!(
                            "SSM command creation failed after {} attempts: {:?}",
                            attempt, aws_error
                        );
                        bail!(
                            "Failed to create SSM Command for EC2 Instance (id='{}') after {} attempts",
                            ec2_instance_id,
                            attempt
                        );
                    }
                }
            }
        }

        bail!(
            "Failed to create SSM Command for EC2 Instance (id='{}') after {} attempts",
            ec2_instance_id,
            max_retries
        );
    }

    pub async fn check_ssm_command_status(
        &self,
        context: &AwsClusterContext,
        command_id: &str,
        instance_id: &str,
    ) -> Result<(CommandInvocationStatus, String, String)> {
        let response = context
            .ssm_client
            .get_command_invocation()
            .command_id(command_id)
            .instance_id(instance_id)
            .send()
            .await?;

        info!("AWS get command invocation response: {:?}", response);

        let stdout = response.standard_output_content().unwrap_or("").to_string();
        let stderr = response.standard_error_content().unwrap_or("").to_string();

        match response.status() {
            Some(status) => Ok((status.clone(), stdout, stderr)),
            None => {
                bail!("SSM command '{}' has no status", command_id);
            }
        }
    }

    pub async fn poll_ssm_command_until_completion(
        &self,
        context: &AwsClusterContext,
        command_id: &str,
        instance_id: &str,
        max_wait_time: std::time::Duration,
        poll_interval: std::time::Duration,
    ) -> Result<String> {
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() > max_wait_time {
                bail!(
                    "SSM command '{}' timed out after {:?}",
                    command_id,
                    max_wait_time
                );
            }

            let check_result = self
                .check_ssm_command_status(context, command_id, instance_id)
                .await;

            let (status, stdout, stderr) = match check_result {
                Ok(result) => result,
                Err(e) => {
                    let error_string = e.to_string();
                    if error_string.contains("InvocationDoesNotExist") {
                        warn!(
                            "Command invocation '{}' not yet available for instance '{}', continuing to poll...",
                            command_id, instance_id
                        );
                        (
                            CommandInvocationStatus::Pending,
                            String::new(),
                            String::new(),
                        )
                    } else {
                        bail!("Unhandled error with SSM command: {:?}", e);
                    }
                }
            };

            match status {
                CommandInvocationStatus::Success => {
                    info!("SSM command '{}' completed successfully", command_id);
                    return Ok(stdout);
                }
                CommandInvocationStatus::InProgress | CommandInvocationStatus::Pending => {
                    info!("SSM command '{}' is still running...", command_id);
                }
                CommandInvocationStatus::Failed
                | CommandInvocationStatus::Cancelled
                | CommandInvocationStatus::TimedOut
                | CommandInvocationStatus::Cancelling => {
                    println!(
                        "SSM Command '{}' failed with status: {:?}",
                        command_id, status
                    );
                    if !stdout.trim().is_empty() {
                        println!("Output: {}", stdout);
                    }
                    if !stderr.trim().is_empty() {
                        println!("Error: {}", stderr);
                    }
                    bail!("SSM command failed with status: {:?}", status);
                }
                _ => {
                    println!(
                        "SSM Command '{}' has unexpected status: {:?}",
                        command_id, status
                    );
                    if !stdout.trim().is_empty() {
                        println!("Output: {}", stdout);
                    }
                    if !stderr.trim().is_empty() {
                        println!("Error: {}", stderr);
                    }
                    bail!("SSM command has unexpected status: {:?}", status);
                }
            }

            sleep(poll_interval).await;
        }
    }
}
