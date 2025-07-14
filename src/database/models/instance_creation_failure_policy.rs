use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type)]
#[sqlx(type_name = "TEXT")]
#[serde(rename_all = "snake_case")]
pub enum InstanceCreationFailurePolicy {
    Cancel,
    Migrate,
    OnDemand,
}

impl fmt::Display for InstanceCreationFailurePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            InstanceCreationFailurePolicy::Cancel => "cancel",
            InstanceCreationFailurePolicy::Migrate => "migrate",
            InstanceCreationFailurePolicy::OnDemand => "on-demand",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for InstanceCreationFailurePolicy {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cancel" => Ok(InstanceCreationFailurePolicy::Cancel),
            "migrate" => Ok(InstanceCreationFailurePolicy::Migrate),
            "on-demand" => Ok(InstanceCreationFailurePolicy::OnDemand),
            _ => Err(()),
        }
    }
}
