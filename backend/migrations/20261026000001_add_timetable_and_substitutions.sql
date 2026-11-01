-- 20261026000001_add_timetable_and_substitutions.sql

-- 1. TIMETABLE SLOTS (Master Recurring Schedule)
CREATE TABLE IF NOT EXISTS timetable_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    day_of_week INT NOT NULL CHECK (day_of_week BETWEEN 1 AND 7), -- 1 = Monday, 6 = Saturday, 7 = Sunday
    period_number INT NOT NULL CHECK (period_number BETWEEN 1 AND 12),
    start_time TIME NOT NULL,
    end_time TIME NOT NULL,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE RESTRICT,
    teacher_staff_id UUID NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
    room_id UUID REFERENCES rooms(id) ON DELETE SET NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_section_day_period UNIQUE(class_section_id, day_of_week, period_number)
);

CREATE INDEX IF NOT EXISTS idx_timetable_teacher_slot ON timetable_slots (school_id, teacher_staff_id, day_of_week, period_number);
CREATE INDEX IF NOT EXISTS idx_timetable_room_slot ON timetable_slots (school_id, room_id, day_of_week, period_number) WHERE room_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_timetable_section_day ON timetable_slots (school_id, class_section_id, day_of_week);

-- 2. SUBSTITUTIONS (Daily Teacher Replacement Records)
CREATE TABLE IF NOT EXISTS substitutions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    timetable_slot_id UUID NOT NULL REFERENCES timetable_slots(id) ON DELETE CASCADE,
    original_teacher_id UUID NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
    substitute_teacher_id UUID NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'COMPLETED', 'CANCELLED')),
    reason TEXT,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_slot_date_substitution UNIQUE(timetable_slot_id, date)
);

CREATE INDEX IF NOT EXISTS idx_substitutions_date_school ON substitutions (school_id, date, status);
CREATE INDEX IF NOT EXISTS idx_substitutions_sub_teacher ON substitutions (school_id, substitute_teacher_id, date);
CREATE INDEX IF NOT EXISTS idx_substitutions_orig_teacher ON substitutions (school_id, original_teacher_id, date);

-- 3. CURRENT DAY TIMETABLE (Materialized Live Daily Projection)
CREATE TABLE IF NOT EXISTS current_day_timetable (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    timetable_slot_id UUID NOT NULL REFERENCES timetable_slots(id) ON DELETE CASCADE,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    day_of_week INT NOT NULL,
    period_number INT NOT NULL,
    start_time TIME NOT NULL,
    end_time TIME NOT NULL,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    effective_teacher_id UUID NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
    original_teacher_id UUID NOT NULL REFERENCES staff(id) ON DELETE RESTRICT,
    substitution_id UUID REFERENCES substitutions(id) ON DELETE SET NULL,
    is_substituted BOOLEAN NOT NULL DEFAULT FALSE,
    room_id UUID REFERENCES rooms(id) ON DELETE SET NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'SCHEDULED',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_daily_slot_date UNIQUE(timetable_slot_id, date)
);

CREATE INDEX IF NOT EXISTS idx_current_day_section ON current_day_timetable (school_id, class_section_id, date);
CREATE INDEX IF NOT EXISTS idx_current_day_effective_teacher ON current_day_timetable (school_id, effective_teacher_id, date);
CREATE INDEX IF NOT EXISTS idx_current_day_orig_teacher ON current_day_timetable (school_id, original_teacher_id, date);

-- 4. Apply updated_at triggers
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'set_timestamp_timetable_slots') THEN
        CREATE TRIGGER set_timestamp_timetable_slots BEFORE UPDATE ON timetable_slots FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'set_timestamp_substitutions') THEN
        CREATE TRIGGER set_timestamp_substitutions BEFORE UPDATE ON substitutions FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'set_timestamp_current_day_timetable') THEN
        CREATE TRIGGER set_timestamp_current_day_timetable BEFORE UPDATE ON current_day_timetable FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
    END IF;
END $$;

-- 5. TRIGGER ON TIMETABLE_SLOTS CHANGE
-- If slot changed or deleted, delete affected substitutions & re-sync today's projection
CREATE OR REPLACE FUNCTION trg_on_timetable_slot_change()
RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'DELETE') THEN
        DELETE FROM substitutions WHERE timetable_slot_id = OLD.id AND date >= CURRENT_DATE;
        DELETE FROM current_day_timetable WHERE timetable_slot_id = OLD.id AND date = CURRENT_DATE;
        RETURN OLD;
    ELSIF (TG_OP = 'UPDATE') THEN
        IF (OLD.teacher_staff_id != NEW.teacher_staff_id 
            OR OLD.period_number != NEW.period_number 
            OR OLD.day_of_week != NEW.day_of_week
            OR OLD.subject_id != NEW.subject_id) THEN
            
            -- Delete affected substitutions as requested
            DELETE FROM substitutions WHERE timetable_slot_id = OLD.id AND date >= CURRENT_DATE;
            
            -- Re-sync today's projection if today matches
            IF (NEW.day_of_week = EXTRACT(ISODOW FROM CURRENT_DATE)) THEN
                UPDATE current_day_timetable
                SET period_number = NEW.period_number,
                    start_time = NEW.start_time,
                    end_time = NEW.end_time,
                    subject_id = NEW.subject_id,
                    effective_teacher_id = NEW.teacher_staff_id,
                    original_teacher_id = NEW.teacher_staff_id,
                    substitution_id = NULL,
                    is_substituted = FALSE,
                    room_id = NEW.room_id,
                    updated_at = NOW()
                WHERE timetable_slot_id = NEW.id AND date = CURRENT_DATE;
            ELSE
                DELETE FROM current_day_timetable WHERE timetable_slot_id = NEW.id AND date = CURRENT_DATE;
            END IF;
        END IF;
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS tr_timetable_slot_change ON timetable_slots;
CREATE TRIGGER tr_timetable_slot_change
AFTER UPDATE OR DELETE ON timetable_slots
FOR EACH ROW EXECUTE PROCEDURE trg_on_timetable_slot_change();

-- 6. TRIGGER ON SUBSTITUTION CHANGE
-- If substitution inserted, cancelled, updated, or removed, immediately update current_day_timetable
CREATE OR REPLACE FUNCTION trg_on_substitution_change()
RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'INSERT' OR TG_OP = 'UPDATE') THEN
        IF (NEW.status = 'ACTIVE' AND NEW.date = CURRENT_DATE) THEN
            UPDATE current_day_timetable
            SET effective_teacher_id = NEW.substitute_teacher_id,
                substitution_id = NEW.id,
                is_substituted = TRUE,
                updated_at = NOW()
            WHERE timetable_slot_id = NEW.timetable_slot_id AND date = NEW.date;
        ELSIF (NEW.status != 'ACTIVE' AND NEW.date = CURRENT_DATE) THEN
            UPDATE current_day_timetable
            SET effective_teacher_id = original_teacher_id,
                substitution_id = NULL,
                is_substituted = FALSE,
                updated_at = NOW()
            WHERE timetable_slot_id = NEW.timetable_slot_id AND date = NEW.date;
        END IF;
        RETURN NEW;
    END IF;

    IF (TG_OP = 'DELETE') THEN
        IF (OLD.date = CURRENT_DATE) THEN
            UPDATE current_day_timetable
            SET effective_teacher_id = original_teacher_id,
                substitution_id = NULL,
                is_substituted = FALSE,
                updated_at = NOW()
            WHERE timetable_slot_id = OLD.timetable_slot_id AND date = OLD.date;
        END IF;
        RETURN OLD;
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS tr_substitution_change ON substitutions;
CREATE TRIGGER tr_substitution_change
AFTER INSERT OR UPDATE OR DELETE ON substitutions
FOR EACH ROW EXECUTE PROCEDURE trg_on_substitution_change();

-- 7. STORED PROCEDURE TO SYNC DAILY TIMETABLE (Can be called by cron or backend at 01:30 AM)
CREATE OR REPLACE FUNCTION sync_daily_timetable_for_date(p_date DATE)
RETURNS INTEGER AS $$
DECLARE
    v_dow INT;
    v_inserted INT := 0;
BEGIN
    v_dow := EXTRACT(ISODOW FROM p_date); -- 1 = Monday, 7 = Sunday

    -- Populate / Upsert slots for today's day of week
    INSERT INTO current_day_timetable (
        school_id, date, timetable_slot_id, class_section_id,
        day_of_week, period_number, start_time, end_time,
        subject_id, effective_teacher_id, original_teacher_id,
        substitution_id, is_substituted, room_id, status, created_at, updated_at
    )
    SELECT
        ts.school_id,
        p_date,
        ts.id,
        ts.class_section_id,
        ts.day_of_week,
        ts.period_number,
        ts.start_time,
        ts.end_time,
        ts.subject_id,
        COALESCE(sub.substitute_teacher_id, ts.teacher_staff_id) as effective_teacher_id,
        ts.teacher_staff_id as original_teacher_id,
        sub.id as substitution_id,
        (sub.id IS NOT NULL) as is_substituted,
        ts.room_id,
        'SCHEDULED',
        NOW(),
        NOW()
    FROM timetable_slots ts
    LEFT JOIN substitutions sub ON sub.timetable_slot_id = ts.id AND sub.date = p_date AND sub.status = 'ACTIVE'
    WHERE ts.day_of_week = v_dow AND ts.status = 'ACTIVE'
    ON CONFLICT (timetable_slot_id, date) DO UPDATE SET
        class_section_id = EXCLUDED.class_section_id,
        day_of_week = EXCLUDED.day_of_week,
        period_number = EXCLUDED.period_number,
        start_time = EXCLUDED.start_time,
        end_time = EXCLUDED.end_time,
        subject_id = EXCLUDED.subject_id,
        effective_teacher_id = EXCLUDED.effective_teacher_id,
        original_teacher_id = EXCLUDED.original_teacher_id,
        substitution_id = EXCLUDED.substitution_id,
        is_substituted = EXCLUDED.is_substituted,
        room_id = EXCLUDED.room_id,
        updated_at = NOW();

    GET DIAGNOSTICS v_inserted = ROW_COUNT;

    -- Prune historic rows older than 30 days
    DELETE FROM current_day_timetable WHERE date < (p_date - INTERVAL '30 days');

    RETURN v_inserted;
END;
$$ LANGUAGE plpgsql;
