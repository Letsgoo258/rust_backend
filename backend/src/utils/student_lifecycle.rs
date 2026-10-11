use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Complete Student Academic Lifecycle Status States
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StudentLifecycleStatus {
    /// Provisional admission pending final verification
    Provisional,
    /// Actively enrolled and attending classes
    Active,
    /// Successfully completed academic program
    Graduated,
    /// Formally graduated alumnus of the school
    Alumni,
    /// Temporarily inactive / not actively participating
    Inactive,
    /// Disciplinary or administrative temporary suspension
    Suspended,
    /// Permanently expelled from the institution
    Expelled,
    /// Officially on approved temporary leave of absence
    OnLeave,
    /// Discontinued studies / dropped out
    Dropout,
    /// Re-admitted after dropout or leave
    Readmitted,
    /// Detained to repeat the academic year (active next year)
    Detained,
    /// Transfer Certificate (TC) application in process
    TcProcess,
    /// Transfer Certificate officially issued
    TcIssued,
    /// Formally transferred out to another institution
    Transferred,
    /// Transfer Certificate application cancelled / revoked
    TcCancelled,
}

#[allow(dead_code)]
impl StudentLifecycleStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Provisional => "PROVISIONAL",
            Self::Active => "ACTIVE",
            Self::Graduated => "GRADUATED",
            Self::Alumni => "ALUMNI",
            Self::Inactive => "INACTIVE",
            Self::Suspended => "SUSPENDED",
            Self::Expelled => "EXPELLED",
            Self::OnLeave => "ON_LEAVE",
            Self::Dropout => "DROPOUT",
            Self::Readmitted => "READMITTED",
            Self::Detained => "DETAINED",
            Self::TcProcess => "TC_PROCESS",
            Self::TcIssued => "TC_ISSUED",
            Self::Transferred => "TRANSFERRED",
            Self::TcCancelled => "TC_CANCELLED",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Provisional => "Provisional",
            Self::Active => "Active",
            Self::Graduated => "Graduated",
            Self::Alumni => "Alumni",
            Self::Inactive => "Inactive",
            Self::Suspended => "Suspended",
            Self::Expelled => "Expelled",
            Self::OnLeave => "On Leave",
            Self::Dropout => "Dropout",
            Self::Readmitted => "Readmitted",
            Self::Detained => "Detained",
            Self::TcProcess => "TC in Process",
            Self::TcIssued => "TC Issued",
            Self::Transferred => "Transferred",
            Self::TcCancelled => "TC Cancelled",
        }
    }

    /// Returns list of valid next status transitions per institutional lifecycle rules
    pub fn valid_transitions(&self) -> &'static [StudentLifecycleStatus] {
        match self {
            // provisional → active
            Self::Provisional => &[Self::Active, Self::Inactive, Self::Dropout, Self::TcProcess],
            // active → graduated | inActive | onLeave | dropout | detained | tcProcess
            Self::Active => &[
                Self::Graduated,
                Self::Inactive,
                Self::OnLeave,
                Self::Dropout,
                Self::Detained,
                Self::TcProcess,
            ],
            // graduated → alumni
            Self::Graduated => &[Self::Alumni],
            // alumni is a terminal state
            Self::Alumni => &[],
            // inActive → suspended | active | dropout
            Self::Inactive => &[
                Self::Active,
                Self::Suspended,
                Self::Dropout,
                Self::TcProcess,
            ],
            // suspended → expelled | active | inActive
            Self::Suspended => &[Self::Expelled, Self::Active, Self::Inactive],
            // expelled is a terminal state
            Self::Expelled => &[],
            // onLeave → active | dropout
            Self::OnLeave => &[Self::Active, Self::Dropout],
            // dropout → readmitted
            Self::Dropout => &[Self::Readmitted, Self::TcProcess],
            // readmitted → active
            Self::Readmitted => &[Self::Active],
            // detained → active (next year)
            Self::Detained => &[Self::Active, Self::Dropout, Self::TcProcess],
            // tcProcess → tcIssued | tcCancelled
            Self::TcProcess => &[Self::TcIssued, Self::TcCancelled, Self::Active],
            // tcIssued → transfered | tcCancelled
            Self::TcIssued => &[Self::Transferred, Self::TcCancelled],
            // transferred is a terminal state
            Self::Transferred => &[],
            // tcCancelled → active
            Self::TcCancelled => &[Self::Active],
        }
    }

    /// Validates whether a state transition from `self` to `target` is legally permitted
    pub fn can_transition_to(&self, target: StudentLifecycleStatus) -> bool {
        if *self == target {
            return true;
        }
        self.valid_transitions().contains(&target)
    }

    /// Whether the student is currently in active academic status
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::Provisional | Self::Readmitted)
    }

    /// Whether the status represents a final terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Alumni | Self::Expelled | Self::Transferred)
    }
}

impl fmt::Display for StudentLifecycleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for StudentLifecycleStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let clean = s.trim().to_uppercase().replace(['-', ' '], "_");
        match clean.as_str() {
            "PROVISIONAL" => Ok(Self::Provisional),
            "ACTIVE" => Ok(Self::Active),
            "GRADUATED" => Ok(Self::Graduated),
            "ALUMNI" => Ok(Self::Alumni),
            "INACTIVE" | "IN_ACTIVE" => Ok(Self::Inactive),
            "SUSPENDED" => Ok(Self::Suspended),
            "EXPELLED" => Ok(Self::Expelled),
            "ON_LEAVE" | "ONLEAVE" => Ok(Self::OnLeave),
            "DROPOUT" | "DROP_OUT" => Ok(Self::Dropout),
            "READMITTED" | "RE_ADMITTED" => Ok(Self::Readmitted),
            "DETAINED" => Ok(Self::Detained),
            "TC_PROCESS" | "TCPROCESS" => Ok(Self::TcProcess),
            "TC_ISSUED" | "TCISSUED" => Ok(Self::TcIssued),
            "TRANSFERRED" | "TRANSFERED" => Ok(Self::Transferred),
            "TC_CANCELLED" | "TCCANCELLED" | "TC_CANCELED" => Ok(Self::TcCancelled),
            _ => Err(format!("Unknown student lifecycle status: '{}'", s)),
        }
    }
}

impl Serialize for StudentLifecycleStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for StudentLifecycleStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        StudentLifecycleStatus::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions_chain() {
        // provisional → active → graduated → alumni
        assert!(
            StudentLifecycleStatus::Provisional.can_transition_to(StudentLifecycleStatus::Active)
        );
        assert!(StudentLifecycleStatus::Active.can_transition_to(StudentLifecycleStatus::Graduated));
        assert!(StudentLifecycleStatus::Graduated.can_transition_to(StudentLifecycleStatus::Alumni));

        // active → inActive → suspended → expelled
        assert!(StudentLifecycleStatus::Active.can_transition_to(StudentLifecycleStatus::Inactive));
        assert!(
            StudentLifecycleStatus::Inactive.can_transition_to(StudentLifecycleStatus::Suspended)
        );
        assert!(
            StudentLifecycleStatus::Suspended.can_transition_to(StudentLifecycleStatus::Expelled)
        );

        // onLeave → active
        assert!(StudentLifecycleStatus::Active.can_transition_to(StudentLifecycleStatus::OnLeave));
        assert!(StudentLifecycleStatus::OnLeave.can_transition_to(StudentLifecycleStatus::Active));

        // dropout → readmitted → active
        assert!(StudentLifecycleStatus::Active.can_transition_to(StudentLifecycleStatus::Dropout));
        assert!(
            StudentLifecycleStatus::Dropout.can_transition_to(StudentLifecycleStatus::Readmitted)
        );
        assert!(
            StudentLifecycleStatus::Readmitted.can_transition_to(StudentLifecycleStatus::Active)
        );

        // detained → active
        assert!(StudentLifecycleStatus::Active.can_transition_to(StudentLifecycleStatus::Detained));
        assert!(StudentLifecycleStatus::Detained.can_transition_to(StudentLifecycleStatus::Active));

        // tcProcess → tcIssued → transfered
        assert!(StudentLifecycleStatus::Active.can_transition_to(StudentLifecycleStatus::TcProcess));
        assert!(
            StudentLifecycleStatus::TcProcess.can_transition_to(StudentLifecycleStatus::TcIssued)
        );
        assert!(
            StudentLifecycleStatus::TcIssued.can_transition_to(StudentLifecycleStatus::Transferred)
        );

        // tcCancelled → active
        assert!(StudentLifecycleStatus::TcProcess
            .can_transition_to(StudentLifecycleStatus::TcCancelled));
        assert!(
            StudentLifecycleStatus::TcCancelled.can_transition_to(StudentLifecycleStatus::Active)
        );
    }
}
