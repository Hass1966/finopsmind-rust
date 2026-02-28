use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub auth: AuthConfig,
    pub encryption_key: String,
    pub llm: LlmConfig,
    pub jobs: JobsConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub name: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

fn default_max_connections() -> u32 {
    10
}

impl DatabaseConfig {
    pub fn url(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.name
        )
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    #[serde(default = "default_token_expiry")]
    pub token_expiry_hours: u64,
}

fn default_token_expiry() -> u64 {
    24
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JobsConfig {
    #[serde(default = "default_cost_sync")]
    pub cost_sync_interval_secs: u64,
    #[serde(default = "default_anomaly_detect")]
    pub anomaly_detect_interval_secs: u64,
    #[serde(default = "default_forecast")]
    pub forecast_interval_secs: u64,
    #[serde(default = "default_budget_check")]
    pub budget_check_interval_secs: u64,
    #[serde(default = "default_recommendation")]
    pub recommendation_interval_secs: u64,
}

fn default_cost_sync() -> u64 {
    21600
}
fn default_anomaly_detect() -> u64 {
    86400
}
fn default_forecast() -> u64 {
    86400
}
fn default_budget_check() -> u64 {
    3600
}
fn default_recommendation() -> u64 {
    86400
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("FINOPS").separator("__"))
            .build()?;

        let app_config: AppConfig = config.try_deserialize()?;
        Ok(app_config)
    }
}
