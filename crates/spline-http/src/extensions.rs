use std::future::Future;
use std::sync::Arc;

use spline_runtime::{Definition, Services};

use crate::{axum::Router, BuildError, Fault, RequestContext};

/// Generated bridge from a neutral definition's concrete implementation and
/// application-error policy to its HTTP router.
pub trait HttpServiceBinding<S, P> {
    fn router_shared(implementation: Arc<S>, errors: P) -> Result<Router, BuildError>;
}

/// Select HTTP on a neutral service bundle. Definition-specific bounds live on
/// the adapter bridge, preserving concrete trait and error-policy checking.
pub trait HttpExt: Sized {
    type Definition;
    type Implementation;

    fn http<P>(self, errors: P) -> Result<Router, BuildError>
    where
        Self::Definition: HttpServiceBinding<Self::Implementation, P>;
}
impl<D, S> HttpExt for Services<D, S> {
    type Definition = D;
    type Implementation = S;
    fn http<P>(self, errors: P) -> Result<Router, BuildError>
    where
        D: HttpServiceBinding<S, P>,
    {
        D::router_shared(self.into_shared(), errors)
    }
}

/// Generated HTTP factory bridge. Context and factory failures belong to this
/// binding, while the neutral definition remains transport-independent.
pub trait HttpFactoryBinding<S, P> {
    fn router_with<F, Fut>(factory: F, errors: P) -> Result<Router, BuildError>
    where
        F: Fn(RequestContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<S>, Fault>> + Send + 'static;
}

/// Select request-scoped HTTP execution from a neutral definition value. The
/// direct Fn bound gives closure parameters their HTTP context type at the call.
pub trait HttpFactoryExt: Definition + Sized {
    fn http_with<S, P, F, Fut>(self, factory: F, errors: P) -> Result<Router, BuildError>
    where
        Self: HttpFactoryBinding<S, P>,
        F: Fn(RequestContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<S>, Fault>> + Send + 'static,
    {
        Self::router_with(factory, errors)
    }
}
impl<D: Definition> HttpFactoryExt for D {}
