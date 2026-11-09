-- Migration: 20261109000003_add_cancellation_details_to_issued_reports.sql
-- Adds explicit audit fields for cancelled reports: cancelled_by, cancelled_by_name, cancelled_at, cancellation_reason

ALTER TABLE issued_reports ADD COLUMN IF NOT EXISTS cancelled_by UUID REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE issued_reports ADD COLUMN IF NOT EXISTS cancelled_by_name VARCHAR(255);
ALTER TABLE issued_reports ADD COLUMN IF NOT EXISTS cancelled_at TIMESTAMPTZ;
ALTER TABLE issued_reports ADD COLUMN IF NOT EXISTS cancellation_reason TEXT;

-- Backfill existing CANCELLED reports
UPDATE issued_reports
SET cancelled_at = COALESCE(cancelled_at, updated_at),
    cancelled_by_name = COALESCE(cancelled_by_name, issuer_name, 'System Administrator'),
    cancellation_reason = COALESCE(cancellation_reason, remarks, 'Cancelled redundant duplicate issuance')
WHERE status = 'CANCELLED' AND (cancelled_at IS NULL OR cancellation_reason IS NULL);
