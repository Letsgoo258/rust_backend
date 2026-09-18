# TechDoc Schema Audit

## Overview
This document outlines discrepancies and required corrections discovered during the audit of the provided schema in `techDoc/Gcpe360_SRD.pdf` against production-ready database design principles.

## Corrections

### 1. Missing Data Types and Constraints
**Problem:** The provided documentation often omits explicit column types and constraints (e.g., length for `VARCHAR`, precise types for financial data).
**Decision:** All schemas will use explicit PostgreSQL native types. Identifiers will use `UUID`, financial data will use `NUMERIC`, and status fields will use PostgreSQL `ENUM`s. Timestamps will use `TIMESTAMPTZ`.
**Reason:** To ensure data integrity at the database level and prevent invalid states.

### 2. students.class_id (Example from Prompt)
**Problem:** A naive implementation might place `class_id` on the `students` table, which is incorrect for multi-year tracking.
**Decision:** Ensure `class_id` (or `class_section_id`) is strictly managed through the `enrollments` table.
**Reason:** Class membership is academic-year-specific and belongs to enrollment.

### 3. Missing `school_id` in Certain Contexts
**Problem:** Some entities might lack direct tenancy (`school_id`) or rely on deep joins for authorization.
**Decision:** Include `school_id` directly in most core entities (e.g., `students`, `staff`, `classes`, `attendance_sessions`) for fast, secure tenant isolation. Ensure composite unique constraints (e.g., `UNIQUE(school_id, code)`).
**Reason:** Multi-tenancy is the most critical security boundary. Direct tenant indexing prevents data leakage and improves query performance.

### 4. Financial Field Types
**Problem:** `amount` in `fee_structure_items`, `fee_invoices`, and `fee_payments` is not typed in the documentation.
**Decision:** Use `NUMERIC(10, 2)` (or appropriate precision) for all monetary fields.
**Reason:** Floating-point numbers lose precision and are inappropriate for financial calculations.

### 5. Attendance Session Identity
**Problem:** `attendance_sessions` could theoretically be duplicated for the same class/period.
**Decision:** Add a composite unique constraint on `(school_id, academic_year_id, class_section_id, date, period_id)`.
**Reason:** A class can only have one authoritative attendance session per period per day.

### 6. Audit Trail Fields
**Problem:** `created_by` and `updated_by` are missing from several tables in the PDF.
**Decision:** Add `created_by` and `updated_by` columns to entities that require strict audit trails. All critical tables will also have `created_at` and `updated_at` (defaulting to `now()`).
**Reason:** Essential for an enterprise system to track who made changes.
