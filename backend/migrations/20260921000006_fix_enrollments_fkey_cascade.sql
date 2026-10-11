-- Fix foreign key constraints on enrollments to allow CASCADE on DELETE and UPDATE
ALTER TABLE enrollments
    DROP CONSTRAINT IF EXISTS enrollments_academic_year_id_fkey,
    ADD CONSTRAINT enrollments_academic_year_id_fkey
    FOREIGN KEY (academic_year_id) REFERENCES academic_years(id) ON DELETE CASCADE ON UPDATE CASCADE;

ALTER TABLE enrollments
    DROP CONSTRAINT IF EXISTS enrollments_class_section_id_fkey,
    ADD CONSTRAINT enrollments_class_section_id_fkey
    FOREIGN KEY (class_section_id) REFERENCES class_sections(id) ON DELETE CASCADE ON UPDATE CASCADE;
