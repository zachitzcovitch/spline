use std::future::Future;
use std::task::{Context, Poll, Waker};

use serde::{Deserialize, Serialize};
use spline::{Direction, Schema, Shape};

#[derive(Serialize, Deserialize, Schema)]
pub struct Outer {
    pub value: String,
}

#[path = "fixtures/module_users.rs"]
pub mod users;

pub mod nested {
    pub mod accounts {
        #[spline::contract]
        pub trait Profiles {
            #[query]
            async fn health(&self) -> Result<(), ()>;
        }
    }
}

pub mod pinned {
    #[spline::contract(namespace = "v1.people")]
    pub trait People {
        #[query]
        async fn health(&self) -> Result<(), ()>;
    }
}

pub mod companions {
    #[spline::contract(module = "people")]
    pub trait People {
        #[query]
        async fn health(&self) -> Result<(), ()>;
    }

    #[spline::contract(module = "posts", namespace = "v1.articles")]
    pub trait Posts {
        #[query]
        async fn health(&self) -> Result<(), ()>;
    }
}

pub mod r#type {
    #[spline::contract]
    pub trait Keywords {
        #[query]
        async fn r#type(&self) -> Result<(), ()>;
    }
}

pub mod actions {
    #[spline::contract]
    pub trait Actions {
        #[query]
        async fn describe(&self) -> Result<(), ()>;
        #[mutation]
        async fn register(&self) -> Result<(), ()>;
    }
}

#[spline::contract]
pub trait Root {
    #[query]
    async fn health(&self) -> Result<(), ()>;
}

fn complete<F: Future + Send>(future: F) -> F::Output {
    match Box::pin(future)
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fixture must complete immediately"),
    }
}

#[test]
fn file_modules_expose_support_directly_and_preserve_argument_type_scope() {
    use users::UserApi;
    let input: users::lookup::Input =
        serde_json::from_str(r#"{"value":{"value":"inner"},"outer":{"value":"outer"}}"#).unwrap();
    assert!(input.note.is_none());
    assert_eq!(
        complete(users::Service.lookup(input.value, input.outer, input.note))
            .unwrap()
            .value,
        "inner"
    );
    assert!(serde_json::from_str::<users::lookup::Input>(
        r#"{"value":{"value":"inner"},"outer":{"value":"outer"},"extra":true}"#,
    )
    .is_err());
    let description = users::describe().unwrap();
    assert_eq!(description.namespace(), "users");
    assert_eq!(description.operations()["get"].id, "users.get");
    let Shape::Ref(id) = &description.operations()["get"].input else {
        panic!("expected named input")
    };
    assert_eq!(id.name.to_string(), "users.get.Args");
    let Shape::Object(input) = &description.schemas().definitions()[id].shape else {
        panic!("expected object")
    };
    assert_eq!(input.fields[0].documentation, ["The supplied value."]);
    assert_eq!(input.fields[2].presence, spline::Presence::Optional);
    assert!(matches!(input.fields[2].shape, Shape::Union(_)));

    let mut registry = spline::Registry::new();
    assert!(
        registry
            .register::<users::lookup::Input>(Direction::Serialize)
            .is_err(),
        "metadata-only arguments are deserialize-only"
    );
}

#[test]
fn inline_modules_namespace_overrides_companions_and_crate_roots_have_clear_defaults() {
    assert_eq!(
        nested::accounts::describe().unwrap().namespace(),
        "accounts"
    );
    assert_eq!(
        pinned::describe().unwrap().operations()["health"].id,
        "v1.people.health"
    );
    assert_eq!(
        companions::people::describe().unwrap().namespace(),
        "people"
    );
    assert_eq!(
        companions::posts::describe().unwrap().namespace(),
        "v1.articles"
    );
    assert_eq!(
        r#type::describe().unwrap().operations()["type"].id,
        "type.type"
    );
    assert_eq!(
        actions::describe().unwrap().operations()["register"].id,
        "actions.register"
    );
    let _: actions::describe::Input = serde_json::from_str("{}").unwrap();
    let _: actions::register::Input = serde_json::from_str("{}").unwrap();
    assert_eq!(describe().unwrap().namespace(), "contract_modules");
}

#[test]
fn same_module_and_companion_resources_register_in_one_application() {
    let mut builder = spline::contract::ApplicationBuilder::new("");
    users::register(&mut builder, "users").unwrap();
    nested::accounts::register(&mut builder, "accounts").unwrap();
    pinned::register(&mut builder, "pinned").unwrap();
    companions::people::register(&mut builder, "people").unwrap();
    companions::posts::register(&mut builder, "posts").unwrap();
    assert_eq!(builder.finish().unwrap().resources().len(), 5);
}
