use crate::database::models::Cluster;
use crate::integrations::providers::aws::AwsInterface;

use anyhow::Result;
use aws_sdk_sqs::types::QueueAttributeName;
use std::collections::HashMap;

impl AwsInterface {
    /// Publishes a synthetic spot interruption notice for one node onto this
    /// cluster's own interruption queue, in the shape EventBridge delivers.
    ///
    /// `poll_spot_interruption_queue` reads only `detail.instance-id` from the
    /// message and then validates the instance's `ClusterId` tag against EC2, so
    /// a notice injected here drives exactly the same code path as a genuine one
    /// from AWS: SIGUSR1 to the MPI job for a preemptive flush, and replacement
    /// provisioning started at notice time rather than at node death.
    ///
    /// Used by V-B scenario (i), where the notice must arrive at a controlled
    /// moment. Pair it with `cluster test-failure` on the same node roughly two
    /// minutes later to reproduce AWS reclaiming the instance after its warning.
    pub async fn send_simulated_spot_interruption(
        &self,
        cluster: &Cluster,
        node_private_ip: &str,
    ) -> Result<()> {
        let context = self.create_cluster_context(cluster).await?;

        let instance_id = match self
            .find_elastic_compute_instance_by_private_ip(&context, node_private_ip)
            .await?
        {
            Some(id) => id,
            None => anyhow::bail!(
                "No running instance with private IP '{}' in Cluster '{}'",
                node_private_ip,
                cluster.display_name
            ),
        };

        let queue_url = match self
            .get_spot_interruption_queue_url(&cluster.id, &cluster.region)
            .await?
        {
            Some(url) => url,
            None => anyhow::bail!(
                "Cluster '{}' has no spot interruption queue. The queue is only created \
                 at spawn time when at least one node uses 'allocation_mode: spot'.",
                cluster.display_name
            ),
        };

        let body = serde_json::json!({
            "version": "0",
            "source": "aws.ec2",
            "detail-type": "EC2 Spot Instance Interruption Warning",
            "detail": {
                "instance-id": instance_id,
                "instance-action": "terminate"
            }
        })
        .to_string();

        let sqs = self.get_sqs_client(&cluster.region).await?;
        sqs.send_message()
            .queue_url(&queue_url)
            .message_body(body)
            .send()
            .await?;

        tracing::info!(
            "Injected spot interruption notice for Instance '{}' (private_ip='{}')",
            instance_id,
            node_private_ip
        );
        Ok(())
    }

    /// Creates an SQS queue and an EventBridge rule that routes EC2 spot interruption
    /// warnings for this cluster into the queue. Returns the queue URL.
    /// Idempotent: safe to call again after a crash.
    pub async fn ensure_spot_interruption_queue(
        &self,
        cluster_id: &str,
        region: &str,
    ) -> Result<String> {
        let sqs = self.get_sqs_client(region).await?;
        let eb = self.get_eventbridge_client(region).await?;

        let queue_name = format!("{}-spot-interruptions", cluster_id);
        let rule_name = format!("{}-spot-interruption-rule", cluster_id);

        // create_queue is idempotent — returns existing URL if queue already exists
        let queue_url = sqs
            .create_queue()
            .queue_name(&queue_name)
            .send()
            .await?
            .queue_url
            .ok_or_else(|| anyhow::anyhow!("SQS did not return a queue URL"))?;

        let attr_resp = sqs
            .get_queue_attributes()
            .queue_url(&queue_url)
            .attribute_names(QueueAttributeName::QueueArn)
            .send()
            .await?;
        let queue_arn = attr_resp
            .attributes()
            .and_then(|m| m.get(&QueueAttributeName::QueueArn))
            .ok_or_else(|| anyhow::anyhow!("Could not retrieve queue ARN"))?
            .clone();

        // EventBridge rule matching all EC2 spot interruption warnings
        let rule_arn = eb
            .put_rule()
            .name(&rule_name)
            .event_pattern(
                r#"{"source":["aws.ec2"],"detail-type":["EC2 Spot Instance Interruption Warning"]}"#,
            )
            .state(aws_sdk_eventbridge::types::RuleState::Enabled)
            .send()
            .await?
            .rule_arn
            .ok_or_else(|| anyhow::anyhow!("EventBridge did not return a rule ARN"))?;

        // Allow EventBridge to publish to the queue
        let policy = serde_json::json!({
            "Version": "2012-10-17",
            "Statement": [{
                "Effect": "Allow",
                "Principal": {"Service": "events.amazonaws.com"},
                "Action": "SQS:SendMessage",
                "Resource": queue_arn,
                "Condition": {
                    "ArnEquals": {"aws:SourceArn": rule_arn}
                }
            }]
        });
        let mut attrs = HashMap::new();
        attrs.insert(QueueAttributeName::Policy, policy.to_string());
        sqs.set_queue_attributes()
            .queue_url(&queue_url)
            .set_attributes(Some(attrs))
            .send()
            .await?;

        // Wire rule → queue
        eb.put_targets()
            .rule(&rule_name)
            .targets(
                aws_sdk_eventbridge::types::Target::builder()
                    .id("sqs-target")
                    .arn(&queue_arn)
                    .build()?,
            )
            .send()
            .await?;

        tracing::info!(
            "Spot interruption queue '{}' ready for cluster '{}'",
            queue_name,
            cluster_id
        );
        Ok(queue_url)
    }

    /// Drains the spot interruption SQS queue and returns the private IPs of any nodes
    /// belonging to this cluster that have received an interruption notice.
    pub async fn poll_spot_interruption_queue(
        &self,
        cluster_id: &str,
        region: &str,
        queue_url: &str,
    ) -> Result<Vec<String>> {
        let sqs = self.get_sqs_client(region).await?;
        let ec2 = self.get_ec2_client(region).await?;

        let messages = sqs
            .receive_message()
            .queue_url(queue_url)
            .max_number_of_messages(10)
            .wait_time_seconds(0)
            .send()
            .await?
            .messages
            .unwrap_or_default();

        let mut affected_ips = Vec::new();

        for msg in &messages {
            if let Some(body) = &msg.body {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(body) {
                    if let Some(instance_id) = event["detail"]["instance-id"].as_str() {
                        // Verify the instance belongs to this cluster before acting
                        match ec2
                            .describe_instances()
                            .instance_ids(instance_id)
                            .send()
                            .await
                        {
                            Ok(resp) => {
                                for reservation in resp.reservations() {
                                    for instance in reservation.instances() {
                                        let belongs = instance.tags().iter().any(|t| {
                                            t.key() == Some("ClusterId")
                                                && t.value() == Some(cluster_id)
                                        });
                                        if belongs {
                                            if let Some(ip) = instance.private_ip_address() {
                                                tracing::warn!(
                                                    "Spot interruption notice for instance '{}' (private_ip='{}')",
                                                    instance_id,
                                                    ip
                                                );
                                                affected_ips.push(ip.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Could not describe instance '{}': {}",
                                    instance_id,
                                    e
                                );
                            }
                        }
                    }
                }
            }

            // Always delete the message to avoid reprocessing
            if let Some(receipt) = &msg.receipt_handle {
                let _ = sqs
                    .delete_message()
                    .queue_url(queue_url)
                    .receipt_handle(receipt)
                    .send()
                    .await;
            }
        }

        Ok(affected_ips)
    }

    /// Resolves the queue URL for this cluster's spot interruption queue.
    /// Returns None if the queue does not exist (i.e., cluster has no spot nodes).
    pub async fn get_spot_interruption_queue_url(
        &self,
        cluster_id: &str,
        region: &str,
    ) -> Result<Option<String>> {
        let sqs = self.get_sqs_client(region).await?;
        let queue_name = format!("{}-spot-interruptions", cluster_id);
        match sqs.get_queue_url().queue_name(&queue_name).send().await {
            Ok(resp) => Ok(resp.queue_url),
            Err(_) => Ok(None),
        }
    }

    /// Removes the EventBridge rule and SQS queue created for this cluster.
    pub async fn cleanup_spot_interruption_queue(
        &self,
        cluster_id: &str,
        region: &str,
        queue_url: &str,
    ) -> Result<()> {
        let sqs = self.get_sqs_client(region).await?;
        let eb = self.get_eventbridge_client(region).await?;

        let rule_name = format!("{}-spot-interruption-rule", cluster_id);

        let _ = eb
            .remove_targets()
            .rule(&rule_name)
            .ids("sqs-target")
            .send()
            .await;

        let _ = eb.delete_rule().name(&rule_name).send().await;

        let _ = sqs.delete_queue().queue_url(queue_url).send().await;

        tracing::info!(
            "Spot interruption queue cleaned up for cluster '{}'",
            cluster_id
        );
        Ok(())
    }
}
