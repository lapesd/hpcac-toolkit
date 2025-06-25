use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use aws_sdk_ssm::error::SdkError;
use aws_sdk_ssm::operation::send_command::SendCommandError;
use aws_sdk_ssm::types::CommandInvocationStatus;
use std::collections::HashMap;
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn send_ssm_command_to_ec2_instance(
        &self,
        context: &AwsClusterContext,
        ec2_instance_id: &str,
        command: String,
    ) -> Result<String> {
        info!(
            "Sending SSM Command to EC2 instance (id='{}')...",
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
            info!("SSM command attempt {} of {}", attempt, max_retries);

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
                        Some(ssm_command) => match ssm_command.command_id() {
                            Some(id) => id,
                            None => {
                                warn!("{:?}", response);
                                bail!(
                                    "Unexpected response from AWS when sending SSM Command to EC2 Instance (id='{}')",
                                    ec2_instance_id
                                );
                            }
                        },
                        None => {
                            warn!("{:?}", response);
                            bail!(
                                "Unexpected response from AWS when sending SSM Command to EC2 Instance (id='{}')",
                                ec2_instance_id
                            );
                        }
                    };

                    info!("Command response: {:?}", response);
                    info!(
                        "Successfully sent SSM Command (id='{}') to EC2 Instance (id='{}') on attempt {}",
                        ssm_command_id, ec2_instance_id, attempt
                    );
                    return Ok(ssm_command_id.to_string());
                }
                Err(e) => {
                    let (should_retry, error_type) = classify_error(&e);

                    if should_retry && attempt < max_retries {
                        let delay = base_delay * attempt as u32;
                        warn!(
                            "SSM command failed with {} error on attempt {}. Retrying in {:?}...",
                            error_type, attempt, delay
                        );
                        warn!("Error details: {:?}", e);

                        sleep(delay).await;
                        continue;
                    } else {
                        if should_retry {
                            error!(
                                "SSM command failed after {} attempts with {} error",
                                max_retries, error_type
                            );
                        } else {
                            error!(
                                "SSM command failed with non-retryable error ({}): {:?}",
                                error_type, e
                            );
                        }

                        bail!(
                            "Failure sending SSM Command to EC2 Instance (id='{}') after {} attempts",
                            ec2_instance_id,
                            attempt
                        );
                    }
                }
            }
        }

        bail!(
            "Failed to send SSM Command to EC2 Instance (id='{}') after {} attempts",
            ec2_instance_id,
            max_retries
        );
    }

    pub async fn send_and_wait_for_ssm_command(
        &self,
        context: &AwsClusterContext,
        ec2_instance_id: &str,
        command: String,
    ) -> Result<String> {
        let max_retries = 5;
        let base_delay = Duration::from_secs(30);

        for attempt in 1..=max_retries {
            info!(
                "SSM command attempt {} of {} for instance '{}'",
                attempt, max_retries, ec2_instance_id
            );

            let command_id = match self
                .send_ssm_command_to_ec2_instance(context, ec2_instance_id, command.clone())
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    let (should_retry, error_type) = classify_error_from_anyhow(&e);
                    if should_retry && attempt < max_retries {
                        let delay = base_delay * attempt as u32;
                        warn!(
                            "Failed to send SSM command on attempt {} with {} error. Retrying in {:?}...",
                            attempt, error_type, delay
                        );
                        sleep(delay).await;
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            };

            // Wait for completion
            match self
                .wait_for_ssm_command_completion_single_attempt(
                    context,
                    &command_id,
                    ec2_instance_id,
                )
                .await
            {
                Ok(output) => return Ok(output),
                Err(e) => {
                    let error_message = format!("{}", e);

                    if attempt < max_retries && is_retryable_command_error(&error_message) {
                        let delay = base_delay * attempt as u32;
                        warn!(
                            "SSM command '{}' failed with retryable error on attempt {}. Retrying entire operation in {:?}...",
                            command_id, attempt, delay
                        );
                        warn!("Error details: {}", error_message);
                        sleep(delay).await;
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        bail!(
            "SSM command failed after {} attempts for instance '{}'",
            max_retries,
            ec2_instance_id
        );
    }

    async fn wait_for_ssm_command_completion_single_attempt(
        &self,
        context: &AwsClusterContext,
        command_id: &str,
        instance_id: &str,
    ) -> Result<String> {
        let timeout_seconds = 1800; // Increased timeout from 300 to 1800 seconds (30 minutes)
        let timeout = Duration::from_secs(timeout_seconds);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() > timeout {
                bail!(
                    "SSM command '{}' timed out after {} seconds",
                    command_id,
                    timeout_seconds
                );
            }

            let invocation_response = context
                .ssm_client
                .get_command_invocation()
                .command_id(command_id)
                .instance_id(instance_id)
                .send()
                .await;

            match invocation_response {
                Ok(response) => match response.status() {
                    Some(CommandInvocationStatus::Success) => {
                        info!("SSM command '{}' completed successfully", command_id);
                        let output = response.standard_output_content().unwrap_or("").to_string();
                        return Ok(output);
                    }
                    Some(CommandInvocationStatus::Failed) => {
                        let error_output =
                            response.standard_error_content().unwrap_or("Unknown error");
                        bail!("SSM command '{}' failed: {}", command_id, error_output);
                    }
                    Some(CommandInvocationStatus::Cancelled) => {
                        bail!("SSM command '{}' was cancelled", command_id);
                    }
                    Some(CommandInvocationStatus::TimedOut) => {
                        bail!("SSM command '{}' timed out", command_id);
                    }
                    Some(CommandInvocationStatus::Cancelling) => {
                        info!("SSM command '{}' is being cancelled...", command_id);
                    }
                    Some(CommandInvocationStatus::InProgress)
                    | Some(CommandInvocationStatus::Pending) => {
                        info!("SSM command '{}' is still running...", command_id);
                    }
                    Some(status) => {
                        info!("SSM command '{}' status: {:?}", command_id, status);
                    }
                    None => {
                        warn!("SSM command '{}' has no status", command_id);
                    }
                },
                Err(e) => {
                    // Handle InvocationDoesNotExist and other API errors
                    let error_str = format!("{:?}", e);
                    if error_str.contains("InvocationDoesNotExist") {
                        warn!(
                            "SSM command invocation '{}' no longer exists, treating as failed",
                            command_id
                        );
                        bail!("SSM command '{}' invocation no longer exists", command_id);
                    } else {
                        // For other API errors, continue retrying for a bit
                        warn!(
                            "Error checking SSM command '{}' status: {:?}",
                            command_id, e
                        );
                        if start_time.elapsed() > Duration::from_secs(30) {
                            bail!(
                                "Persistent error checking SSM command '{}' status: {:?}",
                                command_id,
                                e
                            );
                        }
                    }
                }
            }

            sleep(Duration::from_secs(2)).await;
        }
    }
}

fn classify_error(error: &SdkError<SendCommandError>) -> (bool, &'static str) {
    match error {
        SdkError::ServiceError(service_err) => match service_err.err() {
            // Specific retryable errors
            SendCommandError::InvalidInstanceId(invalid_instance_err) => {
                if let Some(message) = invalid_instance_err.message() {
                    if message.contains("Instances not in a valid state for account") {
                        (true, "invalid instance state")
                    } else {
                        (false, "invalid instance ID")
                    }
                } else {
                    (false, "invalid instance ID")
                }
            }
            SendCommandError::InternalServerError(_) => (true, "internal server error"),
            // Add other retryable service errors as needed
            _ => (false, "service error"),
        },
        // Network and timeout errors are generally retryable
        SdkError::TimeoutError(_) => (true, "timeout"),
        SdkError::DispatchFailure(_) => (true, "network/dispatch failure"),
        SdkError::ResponseError(_) => (true, "response error"),
        // Construction errors are usually not retryable
        SdkError::ConstructionFailure(_) => (false, "construction failure"),
        // Generic fallback - check if it's a command execution error that might be retryable
        _ => {
            let error_str = format!("{:?}", error);
            if is_retryable_command_error(&error_str) {
                (true, "retryable command error")
            } else {
                (false, "unknown error")
            }
        }
    }
}

fn classify_error_from_anyhow(error: &anyhow::Error) -> (bool, &'static str) {
    let error_str = format!("{}", error);
    if is_retryable_command_error(&error_str) {
        (true, "retryable command error")
    } else {
        (false, "unknown error")
    }
}

fn is_retryable_command_error(error_message: &str) -> bool {
    let retryable_patterns = [
        // DNS resolution failures
        "Failed to resolve server",
        "Name or service not known",
        "Temporary failure in name resolution",
        "Host not found",
        // Network connectivity issues
        "Connection timed out",
        "Connection refused",
        "Network is unreachable",
        "No route to host",
        // NFS specific errors that might be transient
        "mount.nfs",
        "mount.nfs4",
        "RPC: Remote system error",
        "RPC: Program not registered",
        // Other transient command failures
        "exit status 32", // Common NFS mount failure exit code
        "Resource temporarily unavailable",
        "Device or resource busy",
        // SSM API errors that might be transient
        "InvocationDoesNotExist",
        "invocation no longer exists",
    ];

    retryable_patterns
        .iter()
        .any(|pattern| error_message.contains(pattern))
}
