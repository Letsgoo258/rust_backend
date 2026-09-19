-- Audit Logs for critical actions (Login, Add Staff, Remove Staff, etc.)
CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID REFERENCES schools(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(255) NOT NULL,
    resource_type VARCHAR(100),
    resource_id UUID,
    details JSONB,
    ip_address VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_audit_logs_school_id ON audit_logs(school_id);
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);

-- Frontend Telemetry for analytics (App Opens, Errors, Session Duration, Clicks)
CREATE TABLE frontend_telemetry (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID REFERENCES schools(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    session_id VARCHAR(255) NOT NULL,
    event_type VARCHAR(100) NOT NULL, -- e.g., 'APP_OPEN', 'ERROR', 'CLICK', 'SESSION_DURATION'
    url TEXT,
    component VARCHAR(255),
    error_message TEXT,
    error_stack TEXT,
    duration_ms INT,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_telemetry_school_id ON frontend_telemetry(school_id);
CREATE INDEX idx_telemetry_session ON frontend_telemetry(session_id);
CREATE INDEX idx_telemetry_event_type ON frontend_telemetry(event_type);
