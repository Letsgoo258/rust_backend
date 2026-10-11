-- Fix literal curly brace typo in roles table code column where code was saved as '{SCHOOL_ADMIN}'
UPDATE roles
SET code = TRIM(BOTH '{}' FROM code)
WHERE code LIKE '{%}';
