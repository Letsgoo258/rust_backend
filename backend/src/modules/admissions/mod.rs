pub mod dto;
pub mod handler;
pub mod repository;
pub mod seq;

#[allow(unused_imports)]
pub use repository::AdmissionRepository;
#[allow(unused_imports)]
pub use seq::{
    generate_next_admission_number, get_admission_sequence_preview, get_school_admission_config,
};
