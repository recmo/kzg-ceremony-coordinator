#![doc = include_str!("../Readme.md")]
#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::nursery)]

mod contribution;

use axum::{
    routing::{get, post},
    Router, Server,
};
use clap::Parser;
use cli_batteries::await_shutdown;
use eyre::{bail, ensure, Result as EyreResult, Result};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use url::{Host, Url};
use valico::json_schema;

#[derive(Clone, Debug, PartialEq, Parser)]
pub struct Options {
    /// API Server url
    #[clap(long, env, default_value = "http://127.0.0.1:8080/")]
    pub server: Url,
}

pub async fn main(options: Options) -> EyreResult<()> {
    let app = Router::new()
        .layer(TraceLayer::new_for_http())
        .route("/login", post(|| async { "Hello, World!" }))
        .route("/ceremony/status", get(|| async { "Hello, World!" }))
        .route("/queue/join", post(|| async { "Hello, World!" }))
        .route("/queue/checkin", post(|| async { "Hello, World!" }))
        .route("/queue/leave", post(|| async { "Hello, World!" }))
        .route("/contribution/start", post(contribution::start))
        .route("/contribution/complete", post(|| async { "Hello, World!" }))
        .route("/contribution/abort", post(|| async { "Hello, World!" }));

    // Load schema
    let schema = serde_json::from_str(include_str!("../specs/contributionSchema.json")).unwrap();
    let mut scope = json_schema::Scope::new();
    let schema = scope.compile_and_return(schema, false).unwrap();

    // Load initial contribution
    let initial = serde_json::from_str(include_str!("../specs/initialContribution.json")).unwrap();
    let validation = schema.validate(&initial);
    if !validation.is_strictly_valid() {
        for error in validation.errors {
            error!("{}", error);
        }
        for missing in validation.missing {
            error!("Missing {}", missing);
        }
        bail!("Initial contribution is not valid.");
    }
    info!("Initial contribution is valid.");

    // Run the server
    let (addr, prefix) = parse_url(&options.server)?;
    let app = Router::new().nest(prefix, app);
    let server = Server::try_bind(&addr)?.serve(app.into_make_service());
    info!("Listening on http://{}{}", server.local_addr(), prefix);
    server.with_graceful_shutdown(await_shutdown()).await?;
    Ok(())
}

fn parse_url(url: &Url) -> Result<(SocketAddr, &str)> {
    ensure!(
        url.scheme() == "http",
        "Only http:// is supported in {}",
        url
    );
    let prefix = url.path();
    let ip: IpAddr = match url.host() {
        Some(Host::Ipv4(ip)) => ip.into(),
        Some(Host::Ipv6(ip)) => ip.into(),
        Some(_) => bail!("Cannot bind {}", url),
        None => Ipv4Addr::LOCALHOST.into(),
    };
    let port = url.port().unwrap_or(8080);
    let addr = SocketAddr::new(ip, port);
    Ok((addr, prefix))
}

#[cfg(test)]
pub mod test {
    use super::*;
    use proptest::proptest;
    use tracing::{error, warn};
    use tracing_test::traced_test;

    #[test]
    #[allow(clippy::eq_op)]
    fn test_with_proptest() {
        proptest!(|(a in 0..5, b in 0..5)| {
            assert_eq!(a + b, b + a);
        });
    }

    #[test]
    #[allow(clippy::disallowed_methods)] // False positive from macro
    #[traced_test]
    fn test_with_log_output() {
        error!("logged on the error level");
        assert!(logs_contain("logged on the error level"));
    }

    #[tokio::test]
    #[allow(clippy::disallowed_methods)] // False positive from macro
    #[traced_test]
    #[allow(clippy::semicolon_if_nothing_returned)] // False positive
    async fn async_test_with_log() {
        // Local log
        info!("This is being logged on the info level");

        // Log from a spawned task (which runs in a separate thread)
        tokio::spawn(async {
            warn!("This is being logged on the warn level from a spawned task");
        })
        .await
        .unwrap();

        // Ensure that `logs_contain` works as intended
        assert!(logs_contain("logged on the info level"));
        assert!(logs_contain("logged on the warn level"));
        assert!(!logs_contain("logged on the error level"));
    }
}

#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench {
    use criterion::{black_box, BatchSize, Criterion};
    use proptest::{
        strategy::{Strategy, ValueTree},
        test_runner::TestRunner,
    };
    use std::time::Duration;
    use tokio::runtime;

    pub fn group(criterion: &mut Criterion) {
        bench_example_proptest(criterion);
        bench_example_async(criterion);
    }

    /// Constructs an executor for async tests
    pub(crate) fn runtime() -> runtime::Runtime {
        runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    /// Example proptest benchmark
    /// Uses proptest to randomize the benchmark input
    fn bench_example_proptest(criterion: &mut Criterion) {
        let input = (0..5, 0..5);
        let mut runner = TestRunner::deterministic();
        // Note: benchmarks need to have proper identifiers as names for
        // the CI to pick them up correctly.
        criterion.bench_function("example_proptest", move |bencher| {
            bencher.iter_batched(
                || input.new_tree(&mut runner).unwrap().current(),
                |(a, b)| {
                    // Benchmark number addition
                    black_box(a + b)
                },
                BatchSize::LargeInput,
            );
        });
    }

    /// Example async benchmark
    /// See <https://bheisler.github.io/criterion.rs/book/user_guide/benchmarking_async.html>
    fn bench_example_async(criterion: &mut Criterion) {
        let duration = Duration::from_micros(1);
        criterion.bench_function("example_async", move |bencher| {
            bencher.to_async(runtime()).iter(|| async {
                tokio::time::sleep(duration).await;
            });
        });
    }
}
