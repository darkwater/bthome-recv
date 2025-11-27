use std::{
    collections::{BTreeMap, HashMap},
    sync::mpsc::Receiver,
    time::Duration,
};

use chrono::{DateTime, Local};
use eframe::{CreationContext, egui};
use egui::{Align, CentralPanel, TopBottomPanel};
use egui_extras::StripBuilder;
use egui_plot::{Legend, Line, Plot, Points};
use serde::{Deserialize, Serialize};

use crate::{Update, bthome::Object};

pub fn run(rx: Receiver<Update>) -> anyhow::Result<()> {
    let native_options = eframe::NativeOptions::default();

    eframe::run_native(
        "BTHome",
        native_options,
        Box::new(|cc| Ok(Box::new(BtHomeApp::new(rx, cc)))),
    )
    .unwrap();

    Ok(())
}

// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// struct Series {
//     name: String,
//     objtype: &'static str,
// }

// impl Display for Series {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let len = self.name.len();
//         write!(f, "{} {}", &self.name[(len - 2)..len], self.objtype)
//     }
// }

#[derive(Serialize, Deserialize)]
struct TimedValue {
    time: Duration,
    value: f64,
}

#[derive(Serialize, Deserialize)]
struct State {
    start_time: DateTime<Local>,
    objects: HashMap<String, BTreeMap<String, Vec<TimedValue>>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            start_time: Local::now(),
            objects: HashMap::new(),
        }
    }
}

struct BtHomeApp {
    state: State,
    rx: Receiver<Update>,
}

impl BtHomeApp {
    fn new(rx: Receiver<Update>, cc: &CreationContext<'_>) -> Self {
        BtHomeApp {
            state: cc
                .storage
                .and_then(|storage| storage.get_string("state"))
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            rx,
        }
    }
}

impl eframe::App for BtHomeApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("state", serde_json::to_string(&self.state).unwrap());
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.rx.try_recv() {
            let time = Local::now()
                .signed_duration_since(self.state.start_time)
                .to_std()
                .unwrap();

            let (objtype, value) = match msg.object {
                Object::Battery(v) => ("Battery", v),
                Object::Temperature(v) => ("Temperature", v),
                Object::Humidity(v) => ("Humidity", v),
                Object::Voltage(v) => ("Voltage", v),
                Object::Rssi(v) => ("RSSI", v as f32),
                Object::Power(_) => continue,
            };

            self.state
                .objects
                .entry(objtype.into())
                .or_default()
                .entry(msg.name)
                .or_default()
                .push(TimedValue {
                    time,
                    value: value as f64,
                });
        }

        TopBottomPanel::top("bar").show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Clear data").clicked() {
                    self.state = Default::default();
                }
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            StripBuilder::new(ui)
                .sizes(egui_extras::Size::remainder(), self.state.objects.len())
                .horizontal(|mut ui| {
                    for (objtype, series) in &self.state.objects {
                        ui.cell(|ui| {
                            ui.label(objtype);

                            let start_time = self.state.start_time;

                            Plot::new(objtype)
                                .legend(Legend::default())
                                .link_axis("plots", [true, false])
                                .link_cursor("plots", [true, false])
                                .include_x(
                                    Local::now()
                                        .signed_duration_since(self.state.start_time)
                                        .to_std()
                                        .unwrap()
                                        .as_secs_f64(),
                                )
                                .x_axis_formatter(move |val, _range| {
                                    (start_time
                                        + chrono::Duration::from_std(Duration::from_secs_f64(
                                            val.value.abs(),
                                        ))
                                        .unwrap()
                                            * if val.value.is_sign_positive() { 1 } else { -1 })
                                    .format("%H:%M")
                                    .to_string()
                                })
                                .label_formatter(move |name, point| {
                                    let datetime = (start_time
                                        + chrono::Duration::from_std(Duration::from_secs_f64(
                                            point.x.abs(),
                                        ))
                                        .unwrap()
                                            * if point.x.is_sign_positive() { 1 } else { -1 })
                                    .format("%H:%M")
                                    .to_string();

                                    format!("{name}\n{datetime}\n{:.1}", point.y)
                                })
                                .show(ui, |plot| {
                                    for (series, objects) in series {
                                        let points = objects
                                            .iter()
                                            .map(|val| [val.time.as_secs_f64(), val.value])
                                            .collect::<Vec<_>>();

                                        plot.line(Line::new(series, points.clone()));
                                        plot.points(Points::new(series, points));
                                    }
                                });
                        });
                    }
                });
        });

        if Local::now()
            .signed_duration_since(self.state.start_time)
            .to_std()
            .unwrap()
            < Duration::from_secs(300)
        {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
