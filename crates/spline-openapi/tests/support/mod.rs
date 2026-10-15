#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use spline::http::StatusCode;
use spline::{Context, Schema, Shape, TypeInfo};

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: u32,
    #[serde(default)]
    pub labels: Vec<String>,
    pub note: Option<String>,
    #[serde(default)]
    pub counters: BTreeMap<String, u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub value: String,
    pub next: Option<Box<Node>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
pub struct Page<T> {
    pub items: Vec<T>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pair(pub (String, Node));
impl Schema for Pair {
    fn info() -> TypeInfo {
        TypeInfo::named("fixture.Pair")
    }
    fn describe(cx: &mut Context<'_>) -> spline::Result<Shape> {
        Ok(Shape::Tuple(vec![
            cx.schema::<String>()?,
            cx.schema::<Node>()?,
        ]))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Schema)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    First,
    Second,
}

#[derive(Default, Debug, Serialize, Deserialize, Schema)]
#[serde(deny_unknown_fields)]
pub struct Delta {
    #[serde(default, skip_serializing_if = "spline::Patch::is_missing")]
    pub note: spline::Patch<Option<String>>,
}

#[derive(Debug, Serialize, Deserialize, Schema, spline::http::Error)]
#[serde(
    tag = "kind",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Failure {
    #[http(status = StatusCode::NOT_FOUND)]
    Missing,
    #[http(status = StatusCode::BAD_REQUEST)]
    #[serde(rename = "invalid_value")]
    Invalid { message: String },
    #[http(status = StatusCode::BAD_REQUEST)]
    Rejected(String),
    #[http(status = StatusCode::INTERNAL_SERVER_ERROR)]
    Internal { trace: String },
}

pub mod sample {
    use super::*;
    /// Profiles and schema conformance fixtures.
    #[spline::contract]
    pub trait Sample {
        /// Read one profile.
        #[query]
        #[http(get = "/profiles/{id}")]
        async fn get(
            &self,
            id: u32,
            #[doc = "Select the profile kind."] choice: Option<Choice>,
            count: Option<u32>,
        ) -> Result<Profile, Failure>;
        #[mutation]
        #[http(post = "/profiles", body = "input", status = 201)]
        async fn create(&self, input: Profile) -> Result<Profile, Failure>;
        #[mutation]
        #[http(post = "/nullable", body = "input")]
        async fn nullable(&self, input: Option<String>) -> Result<Option<String>, Failure>;
        #[mutation]
        #[http(patch = "/profiles/{id}", body = "input")]
        async fn update(&self, id: u32, input: Delta) -> Result<Profile, Failure>;
        #[mutation]
        #[http(delete = "/profiles/{id}", status = 204)]
        async fn delete(&self, id: u32) -> Result<(), Failure>;
        #[query]
        #[http(get = "/unit")]
        async fn unit(&self) -> Result<(), Failure>;
        #[query]
        #[http(get = "/nodes")]
        async fn nodes(&self) -> Result<Page<Node>, Failure>;
        #[query]
        #[http(get = "/pair")]
        async fn pair(&self) -> Result<Pair, Failure>;
    }
}

pub fn profile() -> Profile {
    Profile {
        id: 7,
        labels: vec![],
        note: None,
        counters: BTreeMap::new(),
    }
}

pub struct Backend;
impl sample::Sample for Backend {
    async fn get(&self, id: u32, _: Option<Choice>, _: Option<u32>) -> Result<Profile, Failure> {
        match id {
            0 => Err(Failure::Missing),
            400 => Err(Failure::Invalid {
                message: "invalid".into(),
            }),
            500 => Err(Failure::Internal {
                trace: "trace".into(),
            }),
            _ => Ok(profile()),
        }
    }
    async fn create(&self, input: Profile) -> Result<Profile, Failure> {
        Ok(input)
    }
    async fn nullable(&self, input: Option<String>) -> Result<Option<String>, Failure> {
        Ok(input)
    }
    async fn update(&self, _: u32, input: Delta) -> Result<Profile, Failure> {
        let mut value = profile();
        if let spline::Patch::Value(note) = input.note {
            value.note = note;
        }
        Ok(value)
    }
    async fn delete(&self, _: u32) -> Result<(), Failure> {
        Ok(())
    }
    async fn unit(&self) -> Result<(), Failure> {
        Ok(())
    }
    async fn nodes(&self) -> Result<Page<Node>, Failure> {
        Ok(Page {
            items: vec![Node {
                value: "root".into(),
                next: None,
            }],
        })
    }
    async fn pair(&self) -> Result<Pair, Failure> {
        Ok(Pair((
            "label".into(),
            Node {
                value: "root".into(),
                next: None,
            },
        )))
    }
}

pub fn independent(document: &spline_openapi::Document, cases: serde_json::Value) {
    verify(document, cases, serde_json::json!([]));
}

pub fn independent_responses(document: &spline_openapi::Document, responses: serde_json::Value) {
    verify(document, serde_json::json!([]), responses);
}

fn verify(
    document: &spline_openapi::Document,
    cases: serde_json::Value,
    responses: serde_json::Value,
) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("node")
        .args(["scripts/check-openapi.mjs", "--stdin"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&serde_json::json!({"document":document.as_json(),"cases":cases,"responses":responses})).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
