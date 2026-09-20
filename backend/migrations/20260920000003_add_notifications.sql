CREATE TYPE notification_type AS ENUM ('EMAIL', 'SMS', 'PUSH');
CREATE TYPE notification_status AS ENUM ('PENDING', 'SENT', 'FAILED');

CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    notification_type notification_type NOT NULL,
    status notification_status NOT NULL DEFAULT 'PENDING',
    priority INT NOT NULL DEFAULT 2, -- 1 = Critical, 2 = Normal, 3 = Low
    retry_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_notifications_status_priority ON notifications(status, priority, created_at);
