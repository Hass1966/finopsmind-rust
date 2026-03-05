use tracing::{info, warn};

use super::{get_avg_cpu, get_metric_sum, NewRecommendation, RuleEngine};

/// Approximate hourly pricing for common EC2 instance types (us-east-1, on-demand, Linux).
fn instance_hourly_price(instance_type: &str) -> f64 {
    match instance_type {
        "t2.nano" => 0.0058,
        "t2.micro" => 0.0116,
        "t2.small" => 0.023,
        "t2.medium" => 0.0464,
        "t2.large" => 0.0928,
        "t3.nano" => 0.0052,
        "t3.micro" => 0.0104,
        "t3.small" => 0.0208,
        "t3.medium" => 0.0416,
        "t3.large" => 0.0832,
        "m5.large" => 0.096,
        "m5.xlarge" => 0.192,
        "m5.2xlarge" => 0.384,
        "c5.large" => 0.085,
        "c5.xlarge" => 0.17,
        "c5.2xlarge" => 0.34,
        "r5.large" => 0.126,
        "r5.xlarge" => 0.252,
        _ => 0.10,
    }
}

const CPU_THRESHOLD: f64 = 20.0;
const LAMBDA_CPU_THRESHOLD: f64 = 10.0;
const LAMBDA_PACKETS_THRESHOLD: f64 = 50_000.0;
const LOOKBACK_DAYS: i64 = 14;
const HOURS_PER_MONTH: f64 = 730.0;

pub struct ServerlessMigrationRule;

impl RuleEngine for ServerlessMigrationRule {
    fn evaluate<'a>(
        &'a self,
        config: &'a aws_config::SdkConfig,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Vec<NewRecommendation>>> + Send + 'a>> {
        Box::pin(async move {
            let ec2 = aws_sdk_ec2::Client::new(config);
            let cw = aws_sdk_cloudwatch::Client::new(config);

            let region = config
                .region()
                .map(|r| r.to_string())
                .unwrap_or_else(|| "us-east-1".into());

            let mut recommendations = Vec::new();

            let resp = ec2
                .describe_instances()
                .filters(
                    aws_sdk_ec2::types::Filter::builder()
                        .name("instance-state-name")
                        .values("running")
                        .build(),
                )
                .send()
                .await?;

            for reservation in resp.reservations() {
                for instance in reservation.instances() {
                    let instance_id = instance.instance_id().unwrap_or_default();
                    let instance_type = instance
                        .instance_type()
                        .map(|t| t.as_str().to_string())
                        .unwrap_or_else(|| "unknown".into());

                    if instance_id.is_empty() {
                        continue;
                    }

                    let avg_cpu = match get_avg_cpu(
                        &cw,
                        "AWS/EC2",
                        "InstanceId",
                        instance_id,
                        LOOKBACK_DAYS,
                    )
                    .await
                    {
                        Ok(Some(v)) => v,
                        Ok(None) => continue,
                        Err(e) => {
                            warn!(instance_id, error = %e, "Failed to get CPU metrics");
                            continue;
                        }
                    };

                    if avg_cpu >= CPU_THRESHOLD {
                        continue;
                    }

                    let name = instance
                        .tags()
                        .iter()
                        .find(|t| t.key() == Some("Name"))
                        .and_then(|t| t.value())
                        .unwrap_or("")
                        .to_string();

                    let hourly = instance_hourly_price(&instance_type);
                    let monthly_cost = hourly * HOURS_PER_MONTH;
                    // Serverless typically saves 40-70% for low-utilisation workloads
                    let estimated_savings = monthly_cost * 0.55;

                    let network_packets = get_metric_sum(
                        &cw,
                        "AWS/EC2",
                        "NetworkPacketsIn",
                        "InstanceId",
                        instance_id,
                        LOOKBACK_DAYS,
                    )
                    .await
                    .unwrap_or(f64::MAX);

                    let is_lambda_candidate =
                        avg_cpu < LAMBDA_CPU_THRESHOLD && network_packets < LAMBDA_PACKETS_THRESHOLD;

                    let (target, terraform_code) = if is_lambda_candidate {
                        let safe_name = name
                            .chars()
                            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
                            .collect::<String>();
                        let func_name = if safe_name.is_empty() {
                            instance_id.replace('-', "_")
                        } else {
                            safe_name
                        };

                        let tf = format!(
                            r#"# Migrate EC2 {instance_id} to AWS Lambda
# Avg CPU: {avg_cpu:.2}% | NetworkPacketsIn (14d): {network_packets:.0}

resource "aws_iam_role" "{func_name}_lambda_role" {{
  name               = "{func_name}-lambda-role"
  assume_role_policy = jsonencode({{
    Version = "2012-10-17"
    Statement = [{{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = {{ Service = "lambda.amazonaws.com" }}
    }}]
  }})
}}

resource "aws_iam_role_policy_attachment" "{func_name}_lambda_basic" {{
  role       = aws_iam_role.{func_name}_lambda_role.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}}

resource "aws_lambda_function" "{func_name}" {{
  function_name = "{func_name}"
  role          = aws_iam_role.{func_name}_lambda_role.arn
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  memory_size   = 256
  timeout       = 30

  # TODO: Replace with your application code package
  filename         = "lambda.zip"
  source_code_hash = filebase64sha256("lambda.zip")

  environment {{
    variables = {{
      MIGRATED_FROM = "{instance_id}"
    }}
  }}
}}"#
                        );
                        ("Lambda", tf)
                    } else {
                        let safe_name = name
                            .chars()
                            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
                            .collect::<String>();
                        let svc_name = if safe_name.is_empty() {
                            instance_id.replace('-', "_")
                        } else {
                            safe_name
                        };

                        let tf = format!(
                            r#"# Migrate EC2 {instance_id} to AWS Fargate
# Avg CPU: {avg_cpu:.2}% | NetworkPacketsIn (14d): {network_packets:.0}

resource "aws_ecs_cluster" "{svc_name}_cluster" {{
  name = "{svc_name}-cluster"

  setting {{
    name  = "containerInsights"
    value = "enabled"
  }}
}}

resource "aws_ecs_task_definition" "{svc_name}_task" {{
  family                   = "{svc_name}"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = "256"
  memory                   = "512"
  execution_role_arn       = aws_iam_role.{svc_name}_ecs_execution.arn

  container_definitions = jsonencode([{{
    name      = "{svc_name}"
    image     = "REPLACE_WITH_YOUR_IMAGE"
    cpu       = 256
    memory    = 512
    essential = true

    portMappings = [{{
      containerPort = 80
      hostPort      = 80
      protocol      = "tcp"
    }}]

    environment = [{{
      name  = "MIGRATED_FROM"
      value = "{instance_id}"
    }}]

    logConfiguration = {{
      logDriver = "awslogs"
      options = {{
        "awslogs-group"         = "/ecs/{svc_name}"
        "awslogs-region"        = "{region}"
        "awslogs-stream-prefix" = "ecs"
      }}
    }}
  }}])
}}

resource "aws_iam_role" "{svc_name}_ecs_execution" {{
  name               = "{svc_name}-ecs-execution"
  assume_role_policy = jsonencode({{
    Version = "2012-10-17"
    Statement = [{{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = {{ Service = "ecs-tasks.amazonaws.com" }}
    }}]
  }})
}}

resource "aws_iam_role_policy_attachment" "{svc_name}_ecs_execution_policy" {{
  role       = aws_iam_role.{svc_name}_ecs_execution.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}}

resource "aws_ecs_service" "{svc_name}_service" {{
  name            = "{svc_name}"
  cluster         = aws_ecs_cluster.{svc_name}_cluster.id
  task_definition = aws_ecs_task_definition.{svc_name}_task.arn
  desired_count   = 1
  launch_type     = "FARGATE"

  network_configuration {{
    # TODO: Replace with your VPC subnet IDs and security group IDs
    subnets          = ["REPLACE_WITH_SUBNET_IDS"]
    security_groups  = ["REPLACE_WITH_SG_IDS"]
    assign_public_ip = false
  }}
}}"#
                        );
                        ("Fargate", tf)
                    };

                    recommendations.push(NewRecommendation {
                        rec_type: "serverless_migration".into(),
                        provider: "aws".into(),
                        resource_id: instance_id.to_string(),
                        resource_type: "EC2 Instance".into(),
                        region: region.clone(),
                        account_id: String::new(),
                        estimated_savings,
                        estimated_savings_pct: 55.0,
                        current_config: serde_json::json!({
                            "instance_type": instance_type,
                            "state": "running",
                            "avg_cpu_14d": format!("{avg_cpu:.2}%"),
                            "network_packets_in_14d": network_packets,
                            "name": name,
                        }),
                        recommended_config: serde_json::json!({
                            "action": "migrate_to_serverless",
                            "target": target,
                            "reason": format!(
                                "Average CPU {avg_cpu:.2}% < {CPU_THRESHOLD}% over {LOOKBACK_DAYS} days"
                            ),
                        }),
                        impact: "high".into(),
                        effort: "high".into(),
                        risk: "medium".into(),
                        rule_id: "serverless-migration".into(),
                        severity: if estimated_savings > 100.0 {
                            "high".into()
                        } else {
                            "medium".into()
                        },
                        details: serde_json::json!({
                            "migration_target": target,
                            "avg_cpu": avg_cpu,
                            "network_packets_in": network_packets,
                        }),
                        terraform_code: Some(terraform_code),
                    });
                }
            }

            info!(count = recommendations.len(), "Serverless migration rule completed");
            Ok(recommendations)
        })
    }
}
