use bthome::Object;
use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use clap::Parser;
use futures::StreamExt;
use tokio::pin;

#[cfg(feature = "egui")]
mod app2d;
mod bthome;
mod prometheus;

#[derive(Debug, PartialEq)]
pub struct Update {
    name: String,
    object: Object,
}

#[derive(clap::Parser)]
#[command(version, about)]
struct Opts {
    #[clap(subcommand)]
    frontend: Frontend,
}

#[derive(clap::Subcommand)]
enum Frontend {
    #[cfg(feature = "egui")]
    Egui,
    Prometheus,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    pretty_env_logger::init();

    let manager = Manager::new().await?;

    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().next().expect("no adapters found");

    let events = central.events().await?;

    central.start_scan(ScanFilter::default()).await?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        let update_stream = events.map(|event| async {
            if let CentralEvent::ServiceDataAdvertisement { id, service_data } = event {
                if let Some(data) = service_data.get(&uuid_from_u16(0x181c)) {
                    let peripherals = central.peripherals().await.unwrap();

                    let Some(peripheral) = peripherals.iter().find(|p| p.id() == id) else {
                        log::warn!("got ad from unknown peripheral");
                        return vec![];
                    };

                    let Some(properties) = peripheral.properties().await.unwrap() else {
                        log::warn!("got ad from peripheral with no properties");
                        return vec![];
                    };

                    let Some(name) = properties.local_name else {
                        log::warn!("got ad from peripheral with no name");
                        return vec![];
                    };

                    let mut objects = bthome::decode(data.as_slice())
                        .await
                        .into_iter()
                        .map(|object| Update {
                            name: name.clone(),
                            object,
                        })
                        .collect::<Vec<_>>();

                    if let Some(rssi) = properties.rssi {
                        objects.push(Update {
                            name: name.clone(),
                            object: Object::Rssi(rssi),
                        });
                    }

                    return objects;
                }
            }

            vec![]
        });

        pin!(update_stream);

        while let Some(events) = update_stream.next().await {
            for event in events.await {
                tx.send(event).unwrap();
            }
        }
    });

    match opts.frontend {
        #[cfg(feature = "egui")]
        Frontend::Egui => {
            // gui must happen on the main thread on macOS
            tokio::task::block_in_place(|| app2d::run(rx))
        }
        Frontend::Prometheus => prometheus::run(rx).await,
    }
}
