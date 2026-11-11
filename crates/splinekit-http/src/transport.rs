//! How the generated client sends a request.
//!
//! The client builds an [`HttpRequest`] and reads an [`HttpResponse`]; a
//! [`Transport`] moves bytes between them. The default is
//! [`ReqwestTransport`]. Any `tower::Service` with these types works, so an
//! application adds authentication, token refresh, retries or timeouts as
//! Tower layers around it.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use tower::{BoxError, Service, ServiceExt};

/// A request as the client sends it. The body is [`Bytes`], so a layer can
/// re-send it (after refreshing a token, say) without copying.
pub type HttpRequest = http::Request<Bytes>;
/// A response with its whole body read.
pub type HttpResponse = http::Response<Bytes>;

/// `Send` on native targets, nothing on wasm32, where browser futures and
/// clients aren't `Send`.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + ?Sized> MaybeSend for T {}
#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSend for T {}

/// `Sync` on native targets, nothing on wasm32.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSync: Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Sync + ?Sized> MaybeSync for T {}
#[cfg(target_arch = "wasm32")]
pub trait MaybeSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSync for T {}

/// A Tower service that sends one request. Implemented for every suitable
/// service; name it in bounds rather than implementing it.
pub trait Transport:
    Service<
        HttpRequest,
        Response = HttpResponse,
        Error: Into<BoxError>,
        Future: MaybeSend + 'static,
    > + Clone
    + MaybeSend
    + MaybeSync
    + 'static
{
}
impl<T> Transport for T where
    T: Service<
            HttpRequest,
            Response = HttpResponse,
            Error: Into<BoxError>,
            Future: MaybeSend + 'static,
        > + Clone
        + MaybeSend
        + MaybeSync
        + 'static
{
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type SendFuture =
    Pin<Box<dyn Future<Output = Result<HttpResponse, BoxError>> + Send + 'static>>;
#[cfg(target_arch = "wasm32")]
pub(crate) type SendFuture =
    Pin<Box<dyn Future<Output = Result<HttpResponse, BoxError>> + 'static>>;

/// The client's type-erased transport, cheap to clone.
pub(crate) trait Erased: MaybeSend + MaybeSync {
    fn send(&self, request: HttpRequest) -> SendFuture;
}

struct Erase<T>(T);

impl<T: Transport> Erased for Erase<T> {
    fn send(&self, request: HttpRequest) -> SendFuture {
        // Tower services are called on a clone that was driven to readiness.
        let service = self.0.clone();
        Box::pin(async move {
            let mut service = service;
            let ready = service.ready().await.map_err(Into::into)?;
            ready.call(request).await.map_err(Into::into)
        })
    }
}

pub(crate) fn erase<T: Transport>(transport: T) -> Arc<dyn Erased> {
    Arc::new(Erase(transport))
}

/// The default transport: a `reqwest::Client`. On wasm32 it uses the
/// browser's `fetch`.
#[derive(Clone, Debug)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Service<HttpRequest> for ReqwestTransport {
    type Response = HttpResponse;
    type Error = reqwest::Error;
    type Future = ReqwestFuture;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: HttpRequest) -> Self::Future {
        let client = self.client.clone();
        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let response = client
                .request(parts.method, parts.uri.to_string())
                .headers(parts.headers)
                .body(body)
                .send()
                .await?;
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.bytes().await?;
            let mut out = http::Response::new(body);
            *out.status_mut() = status;
            *out.headers_mut() = headers;
            Ok(out)
        })
    }
}

/// [`ReqwestTransport`]'s future: `Send` except on wasm32.
#[cfg(not(target_arch = "wasm32"))]
pub type ReqwestFuture =
    Pin<Box<dyn Future<Output = Result<HttpResponse, reqwest::Error>> + Send + 'static>>;
/// [`ReqwestTransport`]'s future: `Send` except on wasm32.
#[cfg(target_arch = "wasm32")]
pub type ReqwestFuture =
    Pin<Box<dyn Future<Output = Result<HttpResponse, reqwest::Error>> + 'static>>;
