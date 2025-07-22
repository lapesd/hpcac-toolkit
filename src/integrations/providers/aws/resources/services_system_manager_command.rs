use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use std::collections::HashMap;

use anyhow::{Result, bail};
use aws_sdk_ssm::error::SdkError;
use aws_sdk_ssm::types::CommandInvocationStatus;
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};

impl AwsInterface {
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

            let (status, stdout, stderr) = self
                .check_ssm_command_status(context, command_id, instance_id)
                .await?;

            match status {
                CommandInvocationStatus::Success => {
                    info!("SSM command '{}' completed successfully", command_id);
                    return Ok(stdout);
                }
                CommandInvocationStatus::InProgress | CommandInvocationStatus::Pending => {
                    info!("SSM command '{}' is still running...", command_id);
                }
                _ => {
                    println!("SSM Command '{}' aborted", command_id);
                    if !stdout.trim().is_empty() {
                        println!("Output: {}", stdout);
                    }
                    if !stderr.trim().is_empty() {
                        println!("Error: {}", stderr);
                    }
                    bail!("Failure running SSM command");
                }
            }

            sleep(poll_interval).await;
        }
    }
}
