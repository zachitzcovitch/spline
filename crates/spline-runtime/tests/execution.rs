use std::convert::Infallible;
use std::future::{ready, Ready};
use std::io;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::task::{Context, Poll};
use std::time::Duration;

use spline_runtime::{
    invoke,
    tower::{self, Service},
    CallError,
};

struct ReadyOnce {
    polled: bool,
    ready: bool,
    calls: Arc<AtomicUsize>,
}
impl Clone for ReadyOnce {
    fn clone(&self) -> Self {
        Self {
            polled: false,
            ready: false,
            calls: self.calls.clone(),
        }
    }
}
impl Service<u32> for ReadyOnce {
    type Response = Result<u32, &'static str>;
    type Error = Infallible;
    type Future = Ready<Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if !self.polled {
            self.polled = true;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        self.ready = true;
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, value: u32) -> Self::Future {
        assert!(
            self.ready,
            "call must use the instance that acquired readiness"
        );
        self.ready = false;
        self.calls.fetch_add(1, Ordering::SeqCst);
        ready(Ok(Ok(value)))
    }
}

#[tokio::test]
async fn readiness_is_awaited_and_the_reserved_instance_is_called_once() {
    let calls = Arc::new(AtomicUsize::new(0));
    let service = ReadyOnce {
        polled: false,
        ready: false,
        calls: calls.clone(),
    };
    assert_eq!(invoke(service, 7).await.unwrap(), 7);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

struct NotReady;
impl Service<()> for NotReady {
    type Response = Result<(), &'static str>;
    type Error = io::Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Err(io::Error::other("readiness failed")))
    }
    fn call(&mut self, _: ()) -> Self::Future {
        panic!("a readiness failure must prevent dispatch")
    }
}

#[tokio::test]
async fn application_failures_remain_distinct_from_readiness_and_call_failures() {
    let application =
        tower::service_fn(|()| async { Ok::<_, Infallible>(Err::<(), _>("missing")) });
    assert!(matches!(
        invoke(application, ()).await,
        Err(CallError::Application("missing"))
    ));
    assert!(
        matches!(invoke(NotReady, ()).await, Err(CallError::Service(error)) if error.to_string() == "readiness failed")
    );
    let failed = tower::service_fn(|()| async {
        Err::<Result<(), ()>, _>(io::Error::other("middleware failed"))
    });
    assert!(
        matches!(invoke(failed, ()).await, Err(CallError::Service(error)) if error.to_string() == "middleware failed")
    );
}

#[derive(Clone)]
struct Observed<S> {
    inner: S,
    blocked: Arc<tokio::sync::Notify>,
}
impl<S: Service<u32>> Service<u32> for Observed<S> {
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let result = self.inner.poll_ready(cx);
        if result.is_pending() {
            self.blocked.notify_one();
        }
        result
    }
    fn call(&mut self, input: u32) -> Self::Future {
        self.inner.call(input)
    }
}

#[tokio::test]
async fn shared_concurrency_limits_backpressure_and_cancellation_releases_capacity() {
    let started = Arc::new(tokio::sync::Notify::new());
    let blocked = Arc::new(tokio::sync::Notify::new());
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let service = {
        let (started, gate, calls) = (started.clone(), gate.clone(), calls.clone());
        tower::service_fn(move |value| {
            let (started, gate, calls) = (started.clone(), gate.clone(), calls.clone());
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                started.notify_one();
                let _permit = gate.acquire().await.unwrap();
                Ok::<_, Infallible>(Ok::<_, ()>(value))
            }
        })
    };
    let service = Observed {
        inner: tower::limit::ConcurrencyLimit::new(service, 1),
        blocked: blocked.clone(),
    };
    let first = tokio::spawn(invoke(service.clone(), 1));
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    let second = tokio::spawn(invoke(service, 2));
    tokio::time::timeout(Duration::from_secs(2), blocked.notified())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    gate.add_permits(1);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), second)
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        2
    );
}
