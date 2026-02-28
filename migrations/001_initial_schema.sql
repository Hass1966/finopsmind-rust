-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Organizations table
CREATE TABLE IF NOT EXISTS organizations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    settings JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Users table
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    first_name VARCHAR(100) NOT NULL DEFAULT '',
    last_name VARCHAR(100) NOT NULL DEFAULT '',
    role VARCHAR(20) NOT NULL DEFAULT 'viewer' CHECK (role IN ('admin', 'editor', 'viewer')),
    api_key_hash VARCHAR(255),
    last_login_at TIMESTAMPTZ,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(organization_id, email)
);

CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_org ON users(organization_id);
CREATE INDEX IF NOT EXISTS idx_users_api_key ON users(api_key_hash);

-- Costs table
CREATE TABLE IF NOT EXISTS costs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    amount DECIMAL(15, 2) NOT NULL,
    currency VARCHAR(10) NOT NULL DEFAULT 'USD',
    provider VARCHAR(20) NOT NULL,
    service VARCHAR(255) NOT NULL,
    account_id VARCHAR(255) NOT NULL DEFAULT '',
    region VARCHAR(100) NOT NULL DEFAULT '',
    resource_id VARCHAR(500) NOT NULL DEFAULT '',
    tags JSONB DEFAULT '{}',
    estimated BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(organization_id, date, provider, service, account_id, region, resource_id)
);

CREATE INDEX IF NOT EXISTS idx_costs_org_date ON costs(organization_id, date);
CREATE INDEX IF NOT EXISTS idx_costs_provider ON costs(provider);
CREATE INDEX IF NOT EXISTS idx_costs_service ON costs(service);

-- Budgets table
CREATE TABLE IF NOT EXISTS budgets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    amount DECIMAL(15, 2) NOT NULL,
    currency VARCHAR(10) NOT NULL DEFAULT 'USD',
    period VARCHAR(20) NOT NULL DEFAULT 'monthly',
    filters JSONB DEFAULT '{}',
    thresholds JSONB DEFAULT '[]',
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    current_spend DECIMAL(15, 2) NOT NULL DEFAULT 0,
    forecasted_spend DECIMAL(15, 2) NOT NULL DEFAULT 0,
    start_date DATE,
    end_date DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_budgets_org ON budgets(organization_id);

-- Anomalies table
CREATE TABLE IF NOT EXISTS anomalies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    actual_amount DECIMAL(15, 2) NOT NULL,
    expected_amount DECIMAL(15, 2) NOT NULL,
    deviation DECIMAL(15, 2) NOT NULL DEFAULT 0,
    deviation_pct DECIMAL(10, 2) NOT NULL DEFAULT 0,
    score DECIMAL(5, 4) NOT NULL DEFAULT 0,
    severity VARCHAR(20) NOT NULL DEFAULT 'low',
    status VARCHAR(20) NOT NULL DEFAULT 'open',
    provider VARCHAR(20) NOT NULL DEFAULT '',
    service VARCHAR(255) NOT NULL DEFAULT '',
    account_id VARCHAR(255) NOT NULL DEFAULT '',
    region VARCHAR(100) NOT NULL DEFAULT '',
    root_cause TEXT,
    notes TEXT,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_at TIMESTAMPTZ,
    acknowledged_by UUID,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_anomalies_org ON anomalies(organization_id);
CREATE INDEX IF NOT EXISTS idx_anomalies_severity ON anomalies(severity);
CREATE INDEX IF NOT EXISTS idx_anomalies_status ON anomalies(status);

-- Recommendations table
CREATE TABLE IF NOT EXISTS recommendations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    type VARCHAR(50) NOT NULL,
    provider VARCHAR(20) NOT NULL DEFAULT '',
    account_id VARCHAR(255) NOT NULL DEFAULT '',
    region VARCHAR(100) NOT NULL DEFAULT '',
    resource_id VARCHAR(500) NOT NULL DEFAULT '',
    resource_type VARCHAR(100) NOT NULL DEFAULT '',
    current_config JSONB DEFAULT '{}',
    recommended_config JSONB DEFAULT '{}',
    estimated_savings DECIMAL(15, 2) NOT NULL DEFAULT 0,
    estimated_savings_pct DECIMAL(10, 2) NOT NULL DEFAULT 0,
    currency VARCHAR(10) NOT NULL DEFAULT 'USD',
    impact VARCHAR(20) NOT NULL DEFAULT 'medium',
    effort VARCHAR(20) NOT NULL DEFAULT 'medium',
    risk VARCHAR(20) NOT NULL DEFAULT 'low',
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    details JSONB DEFAULT '{}',
    notes TEXT,
    implemented_by UUID,
    implemented_at TIMESTAMPTZ,
    rule_id VARCHAR(100),
    confidence VARCHAR(20) DEFAULT 'medium',
    terraform_code TEXT,
    resource_metadata JSONB DEFAULT '{}',
    resource_arn VARCHAR(500),
    expires_at TIMESTAMPTZ,
    severity VARCHAR(20) DEFAULT 'medium',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_recommendations_org ON recommendations(organization_id);
CREATE INDEX IF NOT EXISTS idx_recommendations_status ON recommendations(status);
CREATE INDEX IF NOT EXISTS idx_recommendations_type ON recommendations(type);

-- Forecasts table
CREATE TABLE IF NOT EXISTS forecasts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    model_version VARCHAR(50) NOT NULL DEFAULT '1.0',
    granularity VARCHAR(20) NOT NULL DEFAULT 'daily',
    predictions JSONB NOT NULL DEFAULT '[]',
    total_forecasted DECIMAL(15, 2) NOT NULL DEFAULT 0,
    confidence_level DECIMAL(5, 4) NOT NULL DEFAULT 0.8,
    currency VARCHAR(10) NOT NULL DEFAULT 'USD',
    service_filter VARCHAR(255),
    account_filter VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_forecasts_org ON forecasts(organization_id);

-- Alerts table
CREATE TABLE IF NOT EXISTS alerts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL DEFAULT 'low',
    status VARCHAR(20) NOT NULL DEFAULT 'open',
    title VARCHAR(500) NOT NULL,
    message TEXT,
    resource_type VARCHAR(100),
    resource_id VARCHAR(500),
    metadata JSONB DEFAULT '{}',
    triggered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_at TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Cloud providers table
CREATE TABLE IF NOT EXISTS cloud_providers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    provider_type VARCHAR(20) NOT NULL,
    name VARCHAR(255) NOT NULL,
    credentials BYTEA,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    status_message TEXT,
    last_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(organization_id, provider_type)
);

CREATE INDEX IF NOT EXISTS idx_cloud_providers_org ON cloud_providers(organization_id);

-- Remediation actions table
CREATE TABLE IF NOT EXISTS remediation_actions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    recommendation_id UUID REFERENCES recommendations(id),
    type VARCHAR(50) NOT NULL,
    status VARCHAR(30) NOT NULL DEFAULT 'pending_approval',
    provider VARCHAR(20) NOT NULL DEFAULT '',
    account_id VARCHAR(255) NOT NULL DEFAULT '',
    region VARCHAR(100) NOT NULL DEFAULT '',
    resource_id VARCHAR(500) NOT NULL DEFAULT '',
    resource_type VARCHAR(100) NOT NULL DEFAULT '',
    description TEXT,
    current_state JSONB DEFAULT '{}',
    desired_state JSONB DEFAULT '{}',
    estimated_savings DECIMAL(15, 2) NOT NULL DEFAULT 0,
    currency VARCHAR(10) NOT NULL DEFAULT 'USD',
    risk VARCHAR(20) NOT NULL DEFAULT 'low',
    auto_approved BOOLEAN NOT NULL DEFAULT FALSE,
    approval_rule VARCHAR(255),
    requested_by UUID,
    approved_by UUID,
    approved_at TIMESTAMPTZ,
    executed_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    rolled_back_at TIMESTAMPTZ,
    failure_reason TEXT,
    rollback_data JSONB DEFAULT '{}',
    audit_log JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_remediation_actions_org ON remediation_actions(organization_id);
CREATE INDEX IF NOT EXISTS idx_remediation_actions_status ON remediation_actions(status);

-- Auto-approval rules table
CREATE TABLE IF NOT EXISTS auto_approval_rules (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    conditions JSONB NOT NULL DEFAULT '{}',
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Policies table
CREATE TABLE IF NOT EXISTS policies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    type VARCHAR(50) NOT NULL,
    enforcement_mode VARCHAR(20) NOT NULL DEFAULT 'alert_only',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    conditions JSONB NOT NULL DEFAULT '{}',
    providers JSONB DEFAULT '[]',
    environments JSONB DEFAULT '[]',
    created_by UUID,
    last_evaluated_at TIMESTAMPTZ,
    violation_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_policies_org ON policies(organization_id);

-- Policy violations table
CREATE TABLE IF NOT EXISTS policy_violations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    policy_id UUID NOT NULL REFERENCES policies(id) ON DELETE CASCADE,
    policy_name VARCHAR(255) NOT NULL DEFAULT '',
    status VARCHAR(20) NOT NULL DEFAULT 'open',
    provider VARCHAR(20) NOT NULL DEFAULT '',
    account_id VARCHAR(255) NOT NULL DEFAULT '',
    region VARCHAR(100) NOT NULL DEFAULT '',
    resource_id VARCHAR(500) NOT NULL DEFAULT '',
    resource_type VARCHAR(100) NOT NULL DEFAULT '',
    description TEXT,
    severity VARCHAR(20) NOT NULL DEFAULT 'medium',
    details JSONB DEFAULT '{}',
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    remediated_at TIMESTAMPTZ,
    exempted_by UUID,
    exempt_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_policy_violations_org ON policy_violations(organization_id);
CREATE INDEX IF NOT EXISTS idx_policy_violations_policy ON policy_violations(policy_id);

-- Recommendation history table
CREATE TABLE IF NOT EXISTS recommendation_history (
    id BIGSERIAL PRIMARY KEY,
    recommendation_id UUID NOT NULL REFERENCES recommendations(id) ON DELETE CASCADE,
    action VARCHAR(50) NOT NULL,
    old_status VARCHAR(20),
    new_status VARCHAR(20),
    user_id UUID,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Auto-update updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply triggers
DO $$
DECLARE
    t text;
BEGIN
    FOR t IN SELECT unnest(ARRAY[
        'organizations', 'users', 'costs', 'budgets', 'anomalies',
        'recommendations', 'forecasts', 'alerts', 'cloud_providers',
        'remediation_actions', 'auto_approval_rules', 'policies', 'policy_violations'
    ]) LOOP
        EXECUTE format('
            DROP TRIGGER IF EXISTS update_%s_updated_at ON %s;
            CREATE TRIGGER update_%s_updated_at
                BEFORE UPDATE ON %s
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();
        ', t, t, t, t);
    END LOOP;
END $$;

-- Seed default organization
INSERT INTO organizations (id, name, settings) VALUES (
    '00000000-0000-0000-0000-000000000001',
    'Default Organization',
    '{"default_currency": "USD", "timezone": "UTC", "fiscal_year_start": 1, "alerts_enabled": true}'
) ON CONFLICT (id) DO NOTHING;

-- Seed default admin user (password: changeme123)
INSERT INTO users (id, organization_id, email, password_hash, first_name, last_name, role) VALUES (
    '00000000-0000-0000-0000-000000000002',
    '00000000-0000-0000-0000-000000000001',
    'admin@finopsmind.io',
    '$2a$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy',
    'Admin',
    'User',
    'admin'
) ON CONFLICT (organization_id, email) DO NOTHING;
