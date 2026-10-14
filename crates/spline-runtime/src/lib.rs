//! Typed operation execution, independent of transport and codecs.

#![forbid(unsafe_code)]

use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

pub use tower;
pub use tower::BoxError;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// A generated resource/application definition, independent of its adapters.
pub trait Definition {
    type Description;
    fn describe() -> spline_contract::Result<Self::Description>;
}

/// A shared implementation tagged with its neutral definition. Constructing the
/// bundle selects no protocol and performs no binding validation.
pub struct Services<D, S> {
    inner: Arc<S>,
    definition: PhantomData<fn() -> D>,
}
impl<D, S> Clone for Services<D, S> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            definition: PhantomData,
        }
    }
}
impl<D, S> Services<D, S> {
    pub fn new(implementation: S) -> Self {
        Self::from_shared(Arc::new(implementation))
    }
    pub fn from_shared(inner: Arc<S>) -> Self {
        Self {
            inner,
            definition: PhantomData,
        }
    }
    pub fn into_shared(self) -> Arc<S> {
        self.inner
    }
}

/// Generated adapter bridge retaining each definition's concrete client type.
pub trait InProcessBinding<S> {
    type Client;
    fn client_shared(implementation: Arc<S>) -> spline_contract::Result<Self::Client>;
}

/// Select direct typed calls on a neutral service bundle.
pub trait InProcessExt: Sized {
    type Client;
    fn in_process(self) -> spline_contract::Result<Self::Client>;
}
impl<D, S> InProcessExt for Services<D, S>
where
    D: InProcessBinding<S>,
{
    type Client = D::Client;
    fn in_process(self) -> spline_contract::Result<Self::Client> {
        D::client_shared(self.into_shared())
    }
}

/// An operation's typed execution signature and logical identity.
pub trait Operation {
    type Input: Send + 'static;
    type Output: Send + 'static;
    type Error: Send + 'static;
    fn id() -> String;
    fn kind() -> spline_contract::OperationKind;
}

#[derive(Debug)]
pub enum CallError<E> {
    Application(E),
    Service(BoxError),
}
impl<E: fmt::Debug> fmt::Display for CallError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application(error) => write!(f, "application error: {error:?}"),
            Self::Service(error) => write!(f, "operation service failed: {error}"),
        }
    }
}
impl<E: fmt::Debug> std::error::Error for CallError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Service(error) => Some(&**error),
            _ => None,
        }
    }
}

/// Poll readiness and invoke that same service instance. Application failures are
/// typed responses; Tower middleware/readiness failures are service errors.
pub async fn invoke<S, I, O, E>(service: S, input: I) -> Result<O, CallError<E>>
where
    S: tower::Service<I, Response = Result<O, E>>,
    S::Error: Into<BoxError>,
{
    use tower::ServiceExt;
    service
        .oneshot(input)
        .await
        .map_err(|error| CallError::Service(error.into()))?
        .map_err(CallError::Application)
}
