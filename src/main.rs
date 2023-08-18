use bthome::Object;
use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use futures::StreamExt;
use tokio::pin;

mod app2d;
mod bthome;

#[derive(Debug, PartialEq)]
pub struct Update {
    name: String,
    object: Object,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = Manager::new().await?;

    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().next().expect("no adapters found");

    let events = central.events().await?;

    central.start_scan(ScanFilter::default()).await?;

    let (tx, rx) = std::sync::mpsc::channel();

    tokio::spawn(async move {
        let update_stream = events.map(|event| async {
            match event {
                CentralEvent::ServiceDataAdvertisement { id, service_data } => {
                    if let Some(data) = service_data.get(&uuid_from_u16(0x181c)) {
                        let peripherals = central.peripherals().await.unwrap();

                        let Some(peripheral) = peripherals.iter().find(|p| p.id() == id) else {
                            eprintln!("got ad from unknown peripheral");
                            return vec![];
                        };

                        let Some(properties) = peripheral.properties().await.unwrap() else {
                            eprintln!("got ad from peripheral with no properties");
                            return vec![];
                        };

                        let Some(name) = properties.local_name else {
                            eprintln!("got ad from peripheral with no name");
                            return vec![];
                        };

                        let mut objects = bthome::decode(data.as_slice())
                            .await
                            .into_iter()
                            .filter(|obj| match obj {
                                Object::Temperature(_) | Object::Humidity(_) => true,
                                Object::Battery(_) | Object::Voltage(_) | Object::Power(_) => false,
                                Object::Rssi(_) => unreachable!(),
                            })
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
                _ => {}
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

    app2d::run(rx)
}
