//! Types for interacting with Hercules CI.
//!
//! This is reverse engineered from looking at the SPA's network requests. We
//! could have used the client's source, but this feels "clean room".

use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JobSource {
    pub r#ref: String,
}

impl JobSource {
    pub fn branch(&self) -> Option<&str> {
        self.r#ref.strip_prefix("refs/heads/")
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub index: i64,
    pub derivation_status: String,

    pub id: String,
    pub source: JobSource,

    // One of "OnPush" or "Config".
    pub job_type: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JobList {
    pub items: Vec<Job>,
}

impl IntoIterator for JobList {
    type Item = Job;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

#[derive(Deserialize, Debug)]
pub enum AttributeValue {
    #[serde(rename_all = "camelCase")]
    Ok {
        derivation_path: String,
        #[allow(dead_code)]
        status: String,
    }, // What else?
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Attribute {
    pub path: Vec<String>,
    pub value: AttributeValue,
}

impl Attribute {
    /// Returns the name of the NixOS configuration if this attribute is a NixOS configuration.
    pub fn nixos_configuration_name(&self) -> Option<String> {
        (self.path.len() >= 2 && self.path[0] == "nixosConfigurations")
            .then(|| self.path[1].clone())
    }

    pub fn derivation_path(&self) -> Option<String> {
        match &self.value {
            AttributeValue::Ok {
                derivation_path, ..
            } => Some(derivation_path.clone()),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    pub attributes: Vec<Attribute>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub output_name: String,
    pub output_path: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub enum Event {
    Queued(serde_json::Value),
    Started(serde_json::Value),
    #[serde(rename_all = "camelCase")]
    Built {
        outputs: Vec<Output>,
    },
    Succeeded(serde_json::Value),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Derivation {
    pub events: Vec<Vec<Event>>,
}

impl Derivation {
    /// List all outputs of the derivation by scanning for `Built` events.
    pub fn outputs(&self) -> impl Iterator<Item = &Output> {
        self.events
            .iter()
            .flat_map(|events| events.iter())
            .filter_map(|event| {
                if let Event::Built { outputs } = event {
                    Some(outputs.iter())
                } else {
                    None
                }
            })
            .flatten()
    }
}
