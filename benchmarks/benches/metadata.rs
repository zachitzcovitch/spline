use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use spline::http::contract::{HttpBinding, HttpMethod};
use spline::{
    contract::{ApplicationBuilder, ApplicationDescription, OperationKind},
    Direction, Registry, Schema,
};
use spline_example_blog_contracts::{api, users, Post, PostError};
use spline_typescript::Typescript;

#[allow(dead_code)]
#[derive(Schema)]
#[spline(name = "bench.Tree")]
struct Tree {
    label: String,
    children: Vec<Tree>,
}

// Synthetic resource groups share DTOs, isolating operation-count scaling from
// schema-count scaling. This is deliberately separate from the real blog API.
fn application(resources: usize) -> ApplicationDescription {
    let mut builder = ApplicationBuilder::new("Benchmark application");
    for index in 0..resources {
        let group = format!("resource_{index}");
        builder
            .resource(&group, &format!("bench.{group}"), "", |resource| {
                resource.operation::<users::get::Input, Post, PostError>(
                    "get",
                    "get",
                    OperationKind::Query,
                    "",
                )?;
                Ok(())
            })
            .unwrap();
    }
    builder.finish().unwrap()
}

fn http_application(resources: usize) -> spline::http::contract::ApplicationDescription {
    let mut builder = spline::http::contract::ApplicationBuilder::new(application(resources));
    for index in 0..resources {
        builder
            .bind(
                &format!("bench.resource_{index}.get"),
                HttpBinding {
                    method: HttpMethod::Get,
                    path: format!("/resources/{index}/{{id}}"),
                    body: None,
                    success_status: 200,
                },
            )
            .unwrap();
    }
    builder.finish().unwrap()
}

fn metadata(c: &mut Criterion) {
    c.bench_function("schema/cold_post", |b| {
        b.iter(|| {
            let mut registry = Registry::new();
            black_box(registry.register::<Post>(Direction::Serialize).unwrap());
            black_box(registry.finish().unwrap());
        })
    });

    c.bench_function("schema/cold_recursive_tree", |b| {
        b.iter(|| {
            let mut registry = Registry::new();
            black_box(registry.register::<Tree>(Direction::Serialize).unwrap());
            black_box(registry.finish().unwrap());
        })
    });

    let mut registry = Registry::new();
    registry.register::<Post>(Direction::Serialize).unwrap();
    c.bench_function("schema/cached_post", |b| {
        b.iter(|| {
            black_box(registry.register::<Post>(Direction::Serialize).unwrap());
        })
    });

    let blog = api::describe().unwrap();
    assert_eq!(blog.operations().count(), 4);
    c.bench_function("metadata/neutral_blog", |b| {
        b.iter(|| black_box(api::describe().unwrap()))
    });

    c.bench_function("metadata/http_blog", |b| {
        b.iter(|| black_box(api::http::describe().unwrap()))
    });
    let mut group = c.benchmark_group("metadata/neutral_shared_dtos");
    for size in [1, 10, 100] {
        assert_eq!(application(size).resources().len(), size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, size| {
            b.iter(|| black_box(application(black_box(*size))));
        });
    }
    group.finish();

    // Descriptions and exporter configuration are prepared outside the timed loop.
    let exporter = Typescript::new();
    let blog_http = api::http::describe().unwrap();
    assert_eq!(
        exporter
            .export_http_application(&blog_http, "Blog")
            .unwrap()
            .source,
        include_str!("../../examples/blog/typescript/generated.ts"),
    );
    let large = http_application(100);
    let mut group = c.benchmark_group("typescript/export_http");
    for (name, description) in [("blog", &blog_http), ("100_resources", &large)] {
        group.bench_function(name, |b| {
            b.iter(|| {
                black_box(
                    exporter
                        .export_http_application(black_box(description), "Blog")
                        .unwrap(),
                );
            })
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(40)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = metadata
}
criterion_main!(benches);
