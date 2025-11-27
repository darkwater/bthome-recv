use core::{fmt::Display, mem::discriminant, time::Duration};
use std::{collections::HashMap, sync::Arc, time::Instant};

use axum::{Router, extract::State, routing::get};
use tokio::sync::{Mutex, mpsc::UnboundedReceiver};

use crate::{Update, bthome::Object};

const PREFIX: &str = "bthome_";
const TIMEOUT: Duration = Duration::from_secs(300); // when to prune old devices

pub async fn run(mut rx: UnboundedReceiver<Update>) -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(Metrics::default()));

    tokio::spawn({
        let state = state.clone();
        async move {
            while let Some(update) = rx.recv().await {
                let mut state = state.lock().await;
                state.get(update.name).put(update.object);
            }

            panic!("update channel closed");
        }
    });

    tokio::spawn({
        let state = state.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let mut state = state.lock().await;
                let now = Instant::now();
                state
                    .devices
                    .retain(|_, dev| now.duration_since(dev.last_update) < TIMEOUT);
            }
        }
    });

    let app = Router::new()
        .route("/metrics", get(metrics))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9556").await.unwrap();

    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!(e))
}

async fn metrics(state: State<Arc<Mutex<Metrics>>>) -> String {
    state.lock().await.to_string()
}

#[derive(Default)]
struct Metrics {
    devices: HashMap<String, Device>,
}

impl Metrics {
    fn get(&mut self, name: String) -> &mut Device {
        self.devices.entry(name).or_insert_with(|| Device {
            last_update: Instant::now(),
            objects: vec![],
        })
    }
}

struct Device {
    last_update: Instant,
    objects: Vec<Object>,
}

impl Device {
    fn put(&mut self, object: Object) {
        self.objects
            .retain(|obj| discriminant(obj) != discriminant(&object));

        self.objects.push(object);
    }
}

impl Display for Metrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, dev) in &self.devices {
            for object in &dev.objects {
                let metric_name = format!("{}{}", PREFIX, object.prometheus_name());
                let value = match object {
                    Object::Battery(v) => *v as f64,
                    Object::Temperature(v) => *v as f64,
                    Object::Humidity(v) => *v as f64,
                    Object::Voltage(v) => *v as f64,
                    Object::Power(v) => {
                        if *v {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    Object::Rssi(v) => *v as f64,
                };
                writeln!(f, "{}{{name=\"{}\"}} {}", metric_name, name, value)?;
            }
        }
        Ok(())
    }
}
