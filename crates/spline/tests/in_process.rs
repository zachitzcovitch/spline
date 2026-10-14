use std::sync::Arc;

use serde::{Deserialize, Serialize};
use spline::{
    runtime::{self, tower::ServiceExt, CallError, InProcessExt, Operation},
    Schema,
};

pub mod profiles {
    use super::*;

    #[derive(Deserialize, Schema)]
    pub struct Lookup {
        pub name: String,
    }
    #[derive(Debug, Serialize, Schema)]
    pub struct Profile {
        pub name: String,
    }
    #[derive(Debug, Serialize, Schema)]
    pub enum ProfileError {
        Missing,
    }

    #[spline::contract]
    pub trait Profiles {
        #[query]
        async fn get(&self, lookup: Lookup) -> Result<Profile, ProfileError>;
    }

    pub struct Store;
    impl Profiles for Store {
        async fn get(&self, lookup: Lookup) -> Result<Profile, ProfileError> {
            if lookup.name.is_empty() {
                Err(ProfileError::Missing)
            } else {
                Ok(Profile { name: lookup.name })
            }
        }
    }
}

spline::api! { pub mod api { profiles: self::profiles } }

pub mod partial {
    #[spline::contract]
    pub trait Partial {
        #[query]
        #[http(get = "/partial")]
        async fn remote(&self) -> Result<(), ()>;
        #[query]
        async fn local(&self) -> Result<(), ()>;
    }
    pub struct Store;
    impl Partial for Store {
        async fn remote(&self) -> Result<(), ()> {
            Ok(())
        }
        async fn local(&self) -> Result<(), ()> {
            Ok(())
        }
    }
}

#[tokio::test]
async fn unbound_contracts_compose_and_execute_without_bidirectional_codecs() {
    let bundle = api::shared(profiles::Store);
    let client = bundle.clone().in_process().unwrap();
    assert_eq!(
        client
            .profiles()
            .get(profiles::Lookup {
                name: "Alice".into()
            })
            .await
            .unwrap()
            .name,
        "Alice"
    );
    assert!(matches!(
        client
            .profiles()
            .get(profiles::Lookup {
                name: String::new()
            })
            .await,
        Err(CallError::Application(profiles::ProfileError::Missing))
    ));
    assert_eq!(profiles::get::Operation::id(), "profiles.get");
    assert_eq!(
        profiles::get::Operation::kind(),
        spline::contract::OperationKind::Query
    );
    assert_eq!(partial::describe().unwrap().operations().len(), 2);
    partial::shared(partial::Store)
        .in_process()
        .unwrap()
        .local()
        .await
        .unwrap();
    #[cfg(feature = "http")]
    assert!(partial::http::describe()
        .unwrap_err()
        .to_string()
        .contains("partial.local has no HTTP binding"));
}

#[tokio::test]
async fn resource_bundles_preserve_concrete_clients_and_definition_metadata() {
    let bundle: profiles::Services<profiles::Store> = profiles::shared(profiles::Store);
    let client: profiles::in_process::Client<profiles::Store> = bundle.in_process().unwrap();
    assert_eq!(
        client
            .get(profiles::Lookup {
                name: "Typed".into()
            })
            .await
            .unwrap()
            .name,
        "Typed"
    );
    let description = <profiles::Definition as runtime::Definition>::describe().unwrap();
    assert_eq!(description.namespace(), "profiles");
    let _ = profiles::contract();
    let _ = api::contract();
}

#[tokio::test]
async fn typed_operation_middleware_works_without_a_protocol_or_serialization() {
    let service = profiles::get::Service::new(Arc::new(profiles::Store)).map_request(
        |mut input: profiles::get::Input| {
            input.lookup.name = "Middleware".into();
            input
        },
    );
    let profile = runtime::invoke(
        service,
        profiles::get::Input {
            lookup: profiles::Lookup {
                name: String::new(),
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(profile.name, "Middleware");
}
