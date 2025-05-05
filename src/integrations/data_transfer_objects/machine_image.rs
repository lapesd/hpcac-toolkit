use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MachineImageDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner: String,
    pub creation_date: String,
}
