#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use egui::plot::{Line, PlotPoints};
use egui::{FontFamily, FontId, TextStyle};

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

static PUNTI: [&'static str; 3] = [
    "punto di lavoro#1",
    "punto di lavoro#2",
    "punto di lavoro#3",
];

fn main() -> Result<(), eframe::Error> {
    // Log to stdout (if you run with `RUST_LOG=debug`).

    let mut options = eframe::NativeOptions::default();

    //options.maximized = true;
    options.initial_window_size = Some(egui::Vec2 { x: 800.0, y: 640.0 });

    eframe::run_native(
        "Rifrattometro",
        options,
        Box::new(|cc| Box::new(RifrattometroApp::new(cc))),
    )
}

fn configure_text_styles(ctx: &egui::Context) {
    use FontFamily::{Monospace, Proportional};

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(25.0, Proportional)),
        (
            TextStyle::Name("Display".into()),
            FontId::new(40.0, FontFamily::Name("erbos".into())),
        ),
        (TextStyle::Body, FontId::new(16.0, Proportional)),
        (TextStyle::Monospace, FontId::new(14.0, Monospace)),
        (TextStyle::Button, FontId::new(16.0, Proportional)),
        (TextStyle::Small, FontId::new(12.0, Proportional)),
    ]
    .into();
    ctx.set_style(style);
}
fn setup_custom_fonts(ctx: &egui::Context) {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = egui::FontDefinitions::default();

    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    fonts.font_data.insert(
        "my_font".to_owned(),
        egui::FontData::from_static(include_bytes!("../Digital7Mono-Yz9J4.ttf")),
    );

    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Name("erbos".into()))
        .or_default()
        .insert(0, "my_font".to_owned());

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}
struct RifrattometroApp {
    serial_detected: Vec<String>,
    com_port: usize,
    info: String,
    measuring: bool,
    acquired: bool,
    punto: usize,
    readings: Vec<(f64, f64)>,
    frame: usize,
    start_time: Instant,
    rx: Option<mpsc::Receiver<f64>>,
    tx: Option<mpsc::Sender<bool>>,
}

impl RifrattometroApp {
    fn get_serial(&mut self) {
        self.serial_detected.truncate(0);
        self.serial_detected.push("Selezionare...".to_string());
        // DEBUG
        self.serial_detected.push("/dev/pts/10".to_string());
        let ports = serialport::available_ports().expect("No ports found!");
        //println!("found {} ports:", ports.len());
        self.info = format!("Trovate {} porte seriali", ports.len());
        for p in ports {
            //println!("{}", p.port_name);
            self.serial_detected.push(p.port_name);
        }
    }
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);
        configure_text_styles(&cc.egui_ctx);
        let mut state = Self {
            serial_detected: Vec::new(),
            com_port: 0,
            info: String::new(),
            measuring: false,
            acquired: false,
            punto: 0,
            readings: Vec::new(),
            frame: 0,
            rx: None,
            tx: None,
            start_time: Instant::now(),
        };
        state.get_serial();
        state
    }
}

impl eframe::App for RifrattometroApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        //println!("updating app... {}",self.frame);
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(received_message) => {
                    let t = self.start_time.elapsed().as_secs_f64();
                    println!("{}: {}", t, received_message);
                    self.readings.push((t, received_message));
                }
                Err(_) => (),
            }
        }

        self.frame += 1;
        egui::TopBottomPanel::top("top_panel")
            .min_height(48.0)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.label("Porta seriale:");
                    egui::ComboBox::new("com", "")
                        .selected_text(&self.serial_detected[self.com_port])
                        .show_ui(ui, |ui| {
                            ui.style_mut().wrap = Some(false);
                            ui.set_min_width(60.0);
                            for com in self.serial_detected.iter().enumerate() {
                                ui.selectable_value(&mut self.com_port, com.0, com.1);
                            }
                        });
                    if ui.add(egui::Button::new("⟳")).clicked() {
                        self.get_serial();
                    }
                    ui.add_space(32.0);
                    ui.label("Punto di lavoro:");
                    egui::ComboBox::new("punto", "")
                        .selected_text(PUNTI[self.punto])
                        .show_ui(ui, |ui| {
                            ui.style_mut().wrap = Some(false);
                            ui.set_min_width(60.0);
                            for punto in PUNTI.iter().enumerate() {
                                ui.selectable_value(&mut self.punto, punto.0, *punto.1);
                            }
                        });
                });
            });

        egui::TopBottomPanel::bottom("info_panel")
            .min_height(32.0)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.label(&self.info);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.set_max_width(160.0);
                    ui.vertical_centered(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 12.0);
                        if ui
                            .add_enabled(
                                self.com_port > 0 && !self.measuring,
                                egui::Button::new("AVVIA")
                                    .min_size(egui::Vec2 { x: 120.0, y: 40.0 }),
                            )
                            .clicked()
                        {
                            let mut port = serialport::new(&self.serial_detected[self.com_port], 9_600)
                                .timeout(Duration::from_millis(10))
                                .open()
                                .expect("Failed to open port");

                            self.acquired = false;
                            self.measuring = true;

                            let (tx, rx) = mpsc::channel();
                            let (tx2, rx2) = mpsc::channel();
                            self.rx = Some(rx);
                            self.tx = Some(tx2);
                            let tctx = ctx.clone();
                            self.start_time = Instant::now();

                            thread::spawn(move || {
                                loop {
                                    // Send an update message every second
                                    println!("reading");
                                    let mut serial_buf: Vec<u8> = vec![0; 32];
                                    port.read_exact(serial_buf.as_mut_slice())
                                        .expect("Found no data!");
                                    println!("sending message");
                                    let message = 11.4;
                                    tx.send(message).unwrap();
                                    tctx.request_repaint();
                                    match rx2.try_recv() {
                                        Ok(_received_message) => break,
                                        Err(_) => (),
                                    }
                                }
                            });
                        }
                        if ui
                            .add_enabled(
                                self.measuring,
                                egui::Button::new("FERMA")
                                    .min_size(egui::Vec2 { x: 120.0, y: 40.0 }),
                            )
                            .clicked()
                        {
                            self.acquired = true;
                            self.measuring = false;
                            if let Some(tx) = self.tx.take() {
                                tx.send(true).unwrap();
                            }
                            // self.tx.as_ref().unwrap().send(true);
                            //tx.send(true);
                        }
                        if ui
                            .add_enabled(
                                self.acquired,
                                egui::Button::new("SALVA")
                                    .min_size(egui::Vec2 { x: 120.0, y: 40.0 }),
                            )
                            .clicked()
                        {}
                    });
                    ui.horizontal_top(|ui| {
                        let r = egui::Rounding {
                            ne: 8.0,
                            se: 8.0,
                            sw: 8.0,
                            nw: 8.0,
                        };
                        egui::Frame::none()
                            .rounding(r)
                            .fill(egui::Color32::BLACK)
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.add_sized([130.0, 22.0], egui::Label::new("tempo"));
                                    ui.visuals_mut().override_text_color =
                                        Some(egui::Color32::WHITE);
                                    ui.style_mut().override_text_style =
                                        Some(egui::TextStyle::Name("Display".into()));
                                    ui.add_sized([130.0, 100.0], egui::Label::new("03:24"));
                                });
                            });
                        egui::Frame::none()
                            .rounding(r)
                            .fill(egui::Color32::BLACK)
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.add_sized([130.0, 22.0], egui::Label::new("bric"));
                                    ui.visuals_mut().override_text_color =
                                        Some(egui::Color32::YELLOW);
                                    ui.style_mut().override_text_style =
                                        Some(egui::TextStyle::Name("Display".into()));
                                    ui.add_sized([130.0, 100.0], egui::Label::new("24.3"));
                                });
                            });
                        egui::Frame::none()
                            .rounding(r)
                            .fill(egui::Color32::BLACK)
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.add_sized([130.0, 22.0], egui::Label::new("ior"));
                                    ui.visuals_mut().override_text_color =
                                        Some(egui::Color32::LIGHT_GREEN);
                                    ui.style_mut().override_text_style =
                                        Some(egui::TextStyle::Name("Display".into()));
                                    ui.add_sized([130.0, 100.0], egui::Label::new("1.4"));
                                });
                            });
                        egui::Frame::none()
                            .rounding(r)
                            .fill(egui::Color32::BLACK)
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.add_sized([130.0, 22.0], egui::Label::new("tensione"));
                                    ui.visuals_mut().override_text_color =
                                        Some(egui::Color32::LIGHT_GREEN);
                                    ui.style_mut().override_text_style =
                                        Some(egui::TextStyle::Name("Display".into()));
                                    ui.add_sized([130.0, 100.0], egui::Label::new("4.4"));
                                });
                            });
                    });
                });

                //ui.button(ui.available_height().to_string());
                let n = 128;
                let line_points: PlotPoints = (0..=n)
                    .map(|i| {
                        use std::f64::consts::TAU;
                        let x = egui::remap(i as f64, 0.0..=n as f64, -TAU..=TAU);
                        [x, x.sin()]
                    })
                    .collect();
                let line = Line::new(line_points);
                egui::plot::Plot::new("example_plot")
                    .data_aspect(1.0)
                    .show(ui, |plot_ui| plot_ui.line(line))
                    .response
            });
        });
    }
}

/*
ui.vertical(|ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    ui.with_layout(
                        egui::Layout {
                            main_dir: egui::Direction::TopDown,
                            main_wrap: false,
                            main_align: egui::Align::TOP,
                            main_justify: false,
                            cross_align: egui::Align::Center,
                            cross_justify: true,
                        },
                        //egui::Layout::top_down_justified(egui::Align::TOP),
                        |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(10.0, 12.0);
                            ui.set_max_width(100.0);
                            if self.com_port > 0 && !self.measuring {
                                if ui.button("Avvia").clicked() {
                                    self.acquired = false;
                                    self.measuring = true;
                                }
                            } else {
                                ui.add_enabled(false, egui::Button::new("Avvia"));
                            }
                            if self.measuring {
                                if ui.button("Ferma").clicked() {
                                    self.acquired = true;
                                    self.measuring = false;
                                }
                            } else {
                                ui.add_enabled(false, egui::Button::new("Ferma"));
                            }
                            if self.acquired {
                                if ui.button("Salva").clicked() {}
                            } else {
                                ui.add_enabled(false, egui::Button::new("Salva"));
                            }
                        },
                    );
                    ui.label("tempo");
                    ui.label("bric");
                    ui.label("indice");
                    ui.label("volt/volt");
                });
                ui.label("grafico...");
            });
             */
