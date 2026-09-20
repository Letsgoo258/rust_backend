export type StaffRole = 
  | 'VICE_PRINCIPAL' | 'HEADMASTER' | 'CLASS_TEACHER' | 'SUBJECT_COORDINATOR' 
  | 'EXAM_COORDINATOR' | 'LIBRARIAN' | 'LAB_ASSISTANT' | 'COUNSELOR' | 'PT_TEACHER' 
  | 'RECEPTIONIST' | 'HR_MANAGER' | 'ACCOUNTANT' | 'CLERK' | 'TRANSPORT_MANAGER' 
  | 'DRIVER' | 'BUS_ATTENDANT' | 'CANTEEN_MANAGER' | 'CANTEEN_STAFF' | 'SECURITY_HEAD' 
  | 'SECURITY_GUARD' | 'PEON' | 'HOUSEKEEPING' | 'SUPPORT_STAFF' | 'TEACHER' | 'STAFF' | 'SCHOOL_ADMIN';

export interface CsvColumn {
  header: string;
  required: boolean;
  type: 'String' | 'Email' | 'Phone' | 'Date';
}

export interface RoleConfig {
  value: StaffRole;
  label: string;
  description: string;
  category: 'ACADEMIC' | 'ADMINISTRATION' | 'OPERATIONS' | 'SUPPORT';
  schema: CsvColumn[];
}

const COMMON_STAFF_SCHEMA: CsvColumn[] = [
  { header: 'First Name', required: true, type: 'String' },
  { header: 'Last Name', required: true, type: 'String' },
  { header: 'Email', required: true, type: 'Email' },
  { header: 'Phone', required: true, type: 'Phone' },
  { header: 'Employee ID', required: true, type: 'String' },
  { header: 'Joining Date (YYYY-MM-DD)', required: true, type: 'Date' },
];

const ACADEMIC_SCHEMA: CsvColumn[] = [
  ...COMMON_STAFF_SCHEMA,
  { header: 'Department', required: false, type: 'String' },
];

export const STAFF_ROLES: RoleConfig[] = [
  {
    value: 'TEACHER',
    label: 'Teacher',
    description: 'General teaching staff and educators.',
    category: 'ACADEMIC',
    schema: ACADEMIC_SCHEMA
  },
  {
    value: 'CLASS_TEACHER',
    label: 'Class Teacher',
    description: 'Teachers designated with class coordination responsibilities.',
    category: 'ACADEMIC',
    schema: ACADEMIC_SCHEMA
  },
  {
    value: 'VICE_PRINCIPAL',
    label: 'Vice Principal',
    description: 'Second-in-command of school administration.',
    category: 'ADMINISTRATION',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'HEADMASTER',
    label: 'Headmaster / Headmistress',
    description: 'Head of a specific academic section (e.g. Primary, Secondary).',
    category: 'ADMINISTRATION',
    schema: ACADEMIC_SCHEMA
  },
  {
    value: 'SUBJECT_COORDINATOR',
    label: 'Subject Coordinator',
    description: 'Head of a specific academic department.',
    category: 'ACADEMIC',
    schema: ACADEMIC_SCHEMA
  },
  {
    value: 'EXAM_COORDINATOR',
    label: 'Examination Coordinator',
    description: 'Manages school examinations and grading systems.',
    category: 'ACADEMIC',
    schema: ACADEMIC_SCHEMA
  },
  {
    value: 'LIBRARIAN',
    label: 'Librarian',
    description: 'Manages the school library, books, and issuing.',
    category: 'ACADEMIC',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'LAB_ASSISTANT',
    label: 'Laboratory Assistant',
    description: 'Manages science/computer labs and equipment.',
    category: 'ACADEMIC',
    schema: ACADEMIC_SCHEMA
  },
  {
    value: 'COUNSELOR',
    label: 'Student Counselor',
    description: 'Provides academic and psychological guidance to students.',
    category: 'ACADEMIC',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'PT_TEACHER',
    label: 'Physical Training (PT) Teacher',
    description: 'Manages sports, physical education, and athletics.',
    category: 'ACADEMIC',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'HR_MANAGER',
    label: 'HR Manager',
    description: 'Manages staff recruitment, payroll, and HR operations.',
    category: 'ADMINISTRATION',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'ACCOUNTANT',
    label: 'Accountant',
    description: 'Manages school finances, fees collection, and ledgers.',
    category: 'ADMINISTRATION',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'RECEPTIONIST',
    label: 'Receptionist / Front Desk',
    description: 'Manages front office, visitors, and general inquiries.',
    category: 'ADMINISTRATION',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'CLERK',
    label: 'Clerk / Admin Assistant',
    description: 'General administrative support staff.',
    category: 'ADMINISTRATION',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'TRANSPORT_MANAGER',
    label: 'Transport & Fleet Manager',
    description: 'Manages school buses, routes, and transport logistics.',
    category: 'OPERATIONS',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'DRIVER',
    label: 'Bus Driver',
    description: 'Drives school transport vehicles.',
    category: 'OPERATIONS',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'BUS_ATTENDANT',
    label: 'Bus Attendant / Conductor',
    description: 'Assists students on school transport.',
    category: 'OPERATIONS',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'CANTEEN_MANAGER',
    label: 'Canteen Manager',
    description: 'Manages cafeteria operations and inventory.',
    category: 'OPERATIONS',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'SECURITY_HEAD',
    label: 'Chief Security Officer',
    description: 'Heads campus security and visitor management.',
    category: 'OPERATIONS',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'SECURITY_GUARD',
    label: 'Security Guard',
    description: 'Campus security personnel.',
    category: 'OPERATIONS',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'PEON',
    label: 'Peon / Office Boy',
    description: 'General office support staff.',
    category: 'SUPPORT',
    schema: COMMON_STAFF_SCHEMA
  },
  {
    value: 'HOUSEKEEPING',
    label: 'Housekeeping / Janitor',
    description: 'Maintains cleanliness and campus hygiene.',
    category: 'SUPPORT',
    schema: COMMON_STAFF_SCHEMA
  }
];
