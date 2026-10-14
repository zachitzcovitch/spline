use std::future::Future;
use std::task::{Context, Poll, Waker};

use serde::{Deserialize, Serialize};
use spline::{Direction, Schema, Shape};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[spline(name = "test.User")]
pub struct User {
    pub name: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[spline(name = "test.Error")]
pub enum ApiError {
    Missing,
}

/// A small user API.
#[spline::contract(namespace = "api.users", module = "users_contract")]
pub trait Users {
    /// Look up a user.
    #[query(name = "get")]
    async fn lookup(&self, id: String, note: Option<String>) -> Result<User, ApiError>;

    #[mutation]
    async fn create(&self, name: String) -> std::result::Result<User, ApiError>;

    #[query]
    async fn health(&self) -> Result<(), ApiError>;
}

struct Server;
impl Users for Server {
    async fn lookup(&self, id: String, _: Option<String>) -> Result<User, ApiError> {
        if id == "missing" {
            Err(ApiError::Missing)
        } else {
            Ok(User { name: id })
        }
    }
    async fn create(&self, name: String) -> Result<User, ApiError> {
        Ok(User { name })
    }
    async fn health(&self) -> Result<(), ApiError> {
        Ok(())
    }
}

fn complete<F: Future + Send>(future: F) -> F::Output {
    match Box::pin(future)
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fixture must be immediately ready"),
    }
}

#[test]
fn server_trait_and_typed_inputs_work_together() {
    let input: users_contract::lookup::Input = serde_json::from_str(r#"{"id":"Zach"}"#).unwrap();
    assert_eq!(input.note, None);
    assert_eq!(
        complete(Server.lookup(input.id, input.note)).unwrap().name,
        "Zach"
    );
    assert_eq!(
        complete(Server.lookup("missing".into(), None)),
        Err(ApiError::Missing)
    );
    let input: users_contract::create::Input = serde_json::from_str(r#"{"name":"New"}"#).unwrap();
    assert_eq!(complete(Server.create(input.name)).unwrap().name, "New");
    assert!(
        serde_json::from_str::<users_contract::lookup::Input>(r#"{"id":"x","extra":true}"#)
            .is_err()
    );
    assert!(serde_json::from_str::<users_contract::health::Input>("{}").is_ok());
    assert!(complete(Server.health()).is_ok());
}

#[test]
fn metadata_keeps_wire_identity_kind_docs_and_schema_direction() {
    let contract = users_contract::describe().unwrap();
    assert_eq!(contract.namespace(), "api.users");
    assert_eq!(contract.documentation(), "A small user API.");
    assert_eq!(
        contract
            .operations()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["create", "get", "health"]
    );
    let get = &contract.operations()["get"];
    assert_eq!(get.id, "api.users.get");
    assert_eq!(get.rust_name, "lookup");
    assert_eq!(get.kind, spline::contract::OperationKind::Query);
    assert_eq!(get.documentation, "Look up a user.");
    assert_eq!(
        get.arguments
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "note"]
    );
    for (shape, direction) in [
        (&get.input, Direction::Deserialize),
        (&get.output, Direction::Serialize),
        (&get.error, Direction::Serialize),
    ] {
        let Shape::Ref(id) = shape else {
            panic!("expected named reference")
        };
        assert_eq!(id.direction, direction);
        assert!(contract.schemas().definitions().contains_key(id));
    }
    let Shape::Ref(id) = &get.input else {
        unreachable!()
    };
    assert_eq!(id.name.to_string(), "api.users.get.Args");
    let Shape::Object(input) = &contract.schemas().definitions()[id].shape else {
        unreachable!()
    };
    assert_eq!(input.fields[1].presence, spline::Presence::Optional);
    assert!(contract
        .schemas()
        .get("test.User", Direction::Deserialize)
        .is_none());
    assert_eq!(
        contract.operations()["create"].kind,
        spline::contract::OperationKind::Mutation
    );
}

mod original {
    use serde::{Deserialize, Serialize};
    use spline as renamed;

    // A user DTO named Input must not be captured by the generated Input struct.
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize, renamed::Schema)]
    #[spline(crate = "renamed", name = "test.NamedInput")]
    pub struct Input {
        pub value: String,
    }

    #[renamed::contract(namespace = "paths", module = "requests", crate = "renamed")]
    pub trait Paths {
        #[query]
        async fn r#type(
            &self,
            value: self::Input,
            outer: super::User,
        ) -> Result<self::Input, super::ApiError>;
    }

    pub struct Server;
    impl Paths for Server {
        async fn r#type(
            &self,
            value: self::Input,
            _: super::User,
        ) -> Result<self::Input, super::ApiError> {
            Ok(value)
        }
    }
}

#[test]
fn original_type_scope_and_runtime_aliases_survive_generated_modules() {
    let input: original::requests::r#type::Input =
        serde_json::from_str(r#"{"value":{"value":"inner"},"outer":{"name":"outer"}}"#).unwrap();
    assert_eq!(input.value.value, "inner");
    assert_eq!(input.outer.name, "outer");
    assert_eq!(
        complete(original::Paths::r#type(
            &original::Server,
            input.value,
            input.outer
        ))
        .unwrap()
        .value,
        "inner"
    );
    let description = original::requests::describe().unwrap();
    assert_eq!(description.operations()["type"].id, "paths.type");
}
