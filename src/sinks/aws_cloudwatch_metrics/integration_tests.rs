#![cfg(feature = "aws-cloudwatch-metrics-integration-tests")]
#![cfg(test)]

use chrono::{offset::TimeZone, Utc};
use rand::seq::SliceRandom;
use vector_core::metric_tags;

use super::*;
use crate::{
    event::{metric::StatisticKind, Event, MetricKind},
    test_util::{
        components::{run_and_assert_sink_compliance, AWS_SINK_TAGS},
        random_string,
    },
};

fn cloudwatch_address() -> String {
    std::env::var("CLOUDWATCH_ADDRESS").unwrap_or_else(|_| "http://localhost:4566".into())
}

fn config() -> CloudWatchMetricsSinkConfig {
    CloudWatchMetricsSinkConfig {
        default_namespace: "vector".into(),
        region: RegionOrEndpoint::with_both("local", cloudwatch_address().as_str()),
        ..Default::default()
    }
}

#[tokio::test]
async fn cloudwatch_metrics_healthcheck() {
    let config = config();
    let client = config
        .create_client(&ProxyConfig::from_env())
        .await
        .unwrap();
    config.healthcheck(client).await.unwrap();
}

#[tokio::test]
async fn cloudwatch_metrics_put_data() {
    let cx = SinkContext::new_test();
    let config = config();
    let client = config.create_client(&cx.globals.proxy).await.unwrap();
    let sink = CloudWatchMetricsSvc::new(config, client).unwrap();

    let mut events = Vec::new();

    for i in 0..100 {
        let event = Event::Metric(
            Metric::new(
                format!("counter-{}", 0),
                MetricKind::Incremental,
                MetricValue::Counter { value: i as f64 },
            )
            .with_tags(Some(metric_tags!(
                "region" => "us-west-1",
                "production" => "true",
                "e" => "",
            ))),
        );
        events.push(event);
    }

    let gauge_name = random_string(10);
    for i in 0..10 {
        let event = Event::Metric(Metric::new(
            format!("gauge-{}", gauge_name),
            MetricKind::Absolute,
            MetricValue::Gauge { value: i as f64 },
        ));
        events.push(event);
    }

    let distribution_name = random_string(10);
    for i in 0..10 {
        let event = Event::Metric(
            Metric::new(
                format!("distribution-{}", distribution_name),
                MetricKind::Incremental,
                MetricValue::Distribution {
                    samples: vector_core::samples![i as f64 => 100],
                    statistic: StatisticKind::Histogram,
                },
            )
            .with_timestamp(Some(
                Utc.ymd(2018, 11, 14).and_hms_nano(8, 9, 10, 123456789),
            )),
        );
        events.push(event);
    }

    run_and_assert_sink_compliance(sink, stream::iter(events), &AWS_SINK_TAGS).await;
}

#[tokio::test]
async fn cloudwatch_metrics_namespace_partitioning() {
    let cx = SinkContext::new_test();
    let config = config();
    let client = config.create_client(&cx.globals.proxy).await.unwrap();
    let sink = CloudWatchMetricsSvc::new(config, client).unwrap();

    let mut events = Vec::new();

    for namespace in ["ns1", "ns2", "ns3", "ns4"].iter() {
        for _ in 0..100 {
            let event = Event::Metric(
                Metric::new(
                    "counter",
                    MetricKind::Incremental,
                    MetricValue::Counter { value: 1.0 },
                )
                .with_namespace(Some(*namespace)),
            );
            events.push(event);
        }
    }

    events.shuffle(&mut rand::thread_rng());

    run_and_assert_sink_compliance(sink, stream::iter(events), &AWS_SINK_TAGS).await;
}
