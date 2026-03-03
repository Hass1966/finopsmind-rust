# FinOpsMind

Cloud financial operations intelligence platform built with Rust. FinOpsMind aggregates cost data from multiple cloud providers, detects anomalies, generates forecasts, and provides actionable optimization recommendations.

## Features

- **Multi-Cloud Cost Tracking** — Ingest and normalize cost data from AWS, Azure, and GCP
- **Anomaly Detection** — Z-score based detection with configurable sensitivity and AI-powered root cause analysis
- **Cost Forecasting** — ETS (Exponential Smoothing) time-series models via the augurs crate
- **Budget Management** — Create budgets with thresholds and get real-time alerts when spending exceeds limits
- **Optimization Recommendations** — AI-generated recommendations with estimated savings and Terraform code
- **Automated Remediation** — Propose, approve, execute, and rollback remediation actions with audit trails
- **Policy Enforcement** — Define cost policies and automatically detect violations
- **AI Chat Assistant** — Natural language queries against your cost data (Ollama or Anthropic)
- **Real-Time Notifications** — WebSocket-based alerts for anomalies, budget breaches, and new recommendations
- **Executive Reports** — Summary dashboards, period-over-period comparisons, CSV/JSON exports
- **Cost Allocation** — Tag-based cost allocation with untagged resource tracking

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    React Frontend                       │
│          (Vite + TypeScript + Tailwind CSS)             │
└──────────────────────┬──────────────────────────────────┘
                       │ REST API + WebSocket
┌──────────────────────▼──────────────────────────────────┐
│                   Axum HTTP Server                      │
│  ┌──────────┐ ┌──────────┐ ┌────────────┐ ┌─────────┐  │
│  │  Auth    │ │ Handlers │ │ Background │ │   WS    │  │
│  │Middleware│ │(REST API)│ │   Jobs     │ │  Hub    │  │
│  └──────────┘ └──────────┘ └────────────┘ └─────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌────────────┐ ┌─────────┐  │
│  │   ML     │ │  Cloud   │ │  Crypto    │ │  Cache  │  │
│  │(ETS, Z)  │ │(AWS,Azure│ │ (AES-256)  │ │ (Redis) │  │
│  └──────────┘ └──────────┘ └────────────┘ └─────────┘  │
└──────────┬───────────────────────┬──────────────────────┘
           │                       │
  ┌────────▼────────┐    ┌────────▼────────┐
  │  PostgreSQL 16  │    │    Redis 7      │
  │  (Data Store)   │    │   (Cache)       │
  └─────────────────┘    └─────────────────┘
```

**Key technologies:** Rust, Axum 0.7, SQLx (PostgreSQL), Redis, augurs (forecasting), linfa (ML), AES-256-GCM encryption, JWT authentication.

## Prerequisites

- **Rust** 1.82+ (install via [rustup](https://rustup.rs))
- **PostgreSQL** 16+
- **Redis** 7+
- **Ollama** (optional, for AI chat — or set Anthropic API key)

## Local Setup

1. **Clone the repository**
   ```bash
   git clone <repo-url>
   cd finopsmind-rust
   ```

2. **Configure environment**
   ```bash
   cp .env.example .env
   # Edit .env with your database credentials, JWT secret, and encryption key
   ```

3. **Start dependencies with Docker Compose** (optional)
   ```bash
   docker compose up -d postgres redis
   ```

4. **Run the application**
   ```bash
   cargo run
   ```
   The server starts on `http://localhost:8080`.

5. **Or run everything in Docker**
   ```bash
   docker compose up -d
   ```

## API Endpoints

### Public
| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| POST | `/api/v1/auth/login` | User login |
| POST | `/api/v1/auth/signup` | User registration |

### Authenticated (Bearer token required)
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/auth/me` | Current user info |
| POST | `/api/v1/auth/api-keys` | Generate API key |
| GET | `/api/v1/costs/summary` | Cost summary with caching |
| GET | `/api/v1/costs/trend` | Cost trend time-series |
| GET | `/api/v1/costs/breakdown` | Cost breakdown by dimension |
| GET | `/api/v1/costs/export` | Export costs as CSV |
| GET/POST | `/api/v1/budgets` | List / create budgets |
| GET/PUT/DELETE | `/api/v1/budgets/:id` | Get / update / delete budget |
| GET | `/api/v1/anomalies` | List anomalies |
| GET | `/api/v1/anomalies/summary` | Anomaly statistics |
| PATCH | `/api/v1/anomalies/:id` | Update anomaly |
| POST | `/api/v1/anomalies/:id/acknowledge` | Acknowledge anomaly |
| POST | `/api/v1/anomalies/:id/resolve` | Resolve anomaly |
| GET | `/api/v1/recommendations` | List recommendations |
| POST | `/api/v1/recommendations/generate` | Generate recommendations |
| GET | `/api/v1/recommendations/summary` | Recommendation stats |
| GET | `/api/v1/recommendations/:id` | Get recommendation |
| PUT | `/api/v1/recommendations/:id/status` | Update status |
| POST | `/api/v1/recommendations/:id/dismiss` | Dismiss recommendation |
| GET | `/api/v1/recommendations/:id/terraform` | Get Terraform code |
| GET | `/api/v1/forecasts` | List forecasts |
| GET | `/api/v1/forecasts/latest` | Latest forecast |
| GET/POST | `/api/v1/providers` | List / add cloud providers |
| PUT/DELETE | `/api/v1/providers/:id` | Update / remove provider |
| POST | `/api/v1/providers/:id/test` | Test provider connection |
| POST | `/api/v1/providers/:id/sync` | Trigger cost sync |
| GET/POST | `/api/v1/remediations` | List / propose remediations |
| GET | `/api/v1/remediations/summary` | Remediation stats |
| POST | `/api/v1/remediations/:id/approve` | Approve action |
| POST | `/api/v1/remediations/:id/reject` | Reject action |
| POST | `/api/v1/remediations/:id/cancel` | Cancel action |
| POST | `/api/v1/remediations/:id/rollback` | Rollback action |
| GET/POST | `/api/v1/remediations/rules` | Auto-approval rules |
| GET/POST | `/api/v1/policies` | List / create policies |
| GET | `/api/v1/policies/summary` | Policy summary |
| GET | `/api/v1/policies/violations` | Policy violations |
| GET | `/api/v1/policies/:id` | Get policy |
| GET | `/api/v1/reports/executive-summary` | Executive summary |
| GET | `/api/v1/reports/comparison` | Cost comparison |
| GET | `/api/v1/reports/export/csv` | Export CSV report |
| GET | `/api/v1/reports/export/json` | Export JSON report |
| POST | `/api/v1/chat` | AI chat |
| GET/PUT | `/api/v1/settings` | Organization settings |
| GET | `/api/v1/allocations` | Cost allocations |
| GET | `/api/v1/allocations/untagged` | Untagged costs |

### WebSocket
| Path | Description |
|------|-------------|
| GET `/ws?token=<jwt>` | Real-time notifications |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | Built from config.yaml fields |
| `REDIS_URL` | Redis connection string | `redis://localhost:6379` |
| `AUTH__JWT_SECRET` | JWT signing secret | — |
| `AUTH__ENCRYPTION_KEY` | AES-256 encryption key for credentials | — |
| `LLM__PROVIDER` | LLM provider (`anthropic` or `ollama`) | `ollama` |
| `LLM__URL` | LLM API endpoint | `http://localhost:11434/api/chat` |
| `LLM__MODEL` | Model name | `llama3.1:8b` |
| `LLM__API_KEY` | API key for the LLM provider | — |
| `SERVER__HOST` | Server bind address | `0.0.0.0` |
| `SERVER__PORT` | Server port | `8080` |

All variables override their corresponding values in `config.yaml`.

## Deployment

### Docker Compose (recommended)

```bash
cp .env.example .env
# Edit .env with production values
docker compose up -d
```

PostgreSQL and Redis are bound to `127.0.0.1` by default and are not exposed to the public network.

### Manual

1. Build the release binary:
   ```bash
   cargo build --release
   ```

2. Ensure PostgreSQL and Redis are running and accessible.

3. Set environment variables or place a `config.yaml` next to the binary.

4. Run:
   ```bash
   ./target/release/finopsmind
   ```

## Background Jobs

The application runs five background jobs:

| Job | Default Interval | Description |
|-----|-----------------|-------------|
| Cost Sync | 6 hours | Pulls cost data from configured cloud providers |
| Anomaly Detection | 24 hours | Detects spending anomalies using z-score analysis; enriches HIGH/CRITICAL with LLM root cause analysis (max 5 per run) |
| Forecasting | 24 hours | Generates ETS cost forecasts per service |
| Budget Check | 1 hour | Evaluates budgets against current and forecasted spend |
| Recommendation Scan | 24 hours | Runs 8 cost optimization rules across all AWS providers |

## Recommendation Rules

| Rule | ID | Description |
|------|----|-------------|
| Idle EC2 | `idle-ec2` | Instances with avg CPU < 5% over 7 days |
| Oversized RDS | `oversized-rds` | RDS instances with avg CPU < 10% over 7 days |
| Unattached EBS | `unattached-ebs` | EBS volumes in `available` state |
| Old Snapshots | `old-snapshots` | EBS snapshots older than 90 days |
| Idle ELB | `idle-elb` | Load balancers with zero healthy targets or < 100 requests/day |
| Missing RI | `missing-ri` | On-demand EC2 instances running 30+ days without reserved instance coverage |
| S3 Lifecycle | `s3-lifecycle` | S3 buckets without lifecycle rules configured |
| Idle EIP | `idle-eip` | Elastic IPs not associated with a running instance ($3.65/mo each) |

Each rule generates Terraform code snippets for remediation.

## AWS IAM Permissions

The following IAM policy grants the minimum permissions required for all recommendation rules and cost syncing:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "CostExplorer",
      "Effect": "Allow",
      "Action": [
        "ce:GetCostAndUsage",
        "ce:GetCostForecast"
      ],
      "Resource": "*"
    },
    {
      "Sid": "EC2ReadOnly",
      "Effect": "Allow",
      "Action": [
        "ec2:DescribeInstances",
        "ec2:DescribeVolumes",
        "ec2:DescribeSnapshots",
        "ec2:DescribeAddresses",
        "ec2:DescribeReservedInstances",
        "ec2:DescribeImages"
      ],
      "Resource": "*"
    },
    {
      "Sid": "RDSReadOnly",
      "Effect": "Allow",
      "Action": [
        "rds:DescribeDBInstances"
      ],
      "Resource": "*"
    },
    {
      "Sid": "CloudWatchMetrics",
      "Effect": "Allow",
      "Action": [
        "cloudwatch:GetMetricStatistics",
        "cloudwatch:ListMetrics"
      ],
      "Resource": "*"
    },
    {
      "Sid": "ELBReadOnly",
      "Effect": "Allow",
      "Action": [
        "elasticloadbalancing:DescribeLoadBalancers",
        "elasticloadbalancing:DescribeTargetGroups",
        "elasticloadbalancing:DescribeTargetHealth"
      ],
      "Resource": "*"
    },
    {
      "Sid": "S3ReadOnly",
      "Effect": "Allow",
      "Action": [
        "s3:ListAllMyBuckets",
        "s3:GetLifecycleConfiguration"
      ],
      "Resource": "*"
    }
  ]
}
```

## License

Private — all rights reserved.
