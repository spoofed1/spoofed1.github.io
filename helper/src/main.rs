#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod pairing;
mod protocol;

use eframe::egui;
use pairing::{Outcome, Phone};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

enum Event {
    Phones(Result<Vec<Phone>, String>),
    Stage(u64, String),
    Finished(u64, Result<Outcome, String>),
}

struct Helper {
    runtime: tokio::runtime::Runtime,
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
    phones: Vec<Phone>,
    selected: Option<String>,
    scanning: bool,
    next_scan: Instant,
    scan_error: Option<String>,
    task: Option<tokio::task::JoinHandle<()>>,
    generation: u64,
    message: String,
    success: bool,
    error: bool,
}

impl Helper {
    fn new(cc: &eframe::CreationContext<'_>, runtime: tokio::runtime::Runtime) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = egui::Color32::from_rgb(242, 246, 240);
        style.visuals.override_text_color = Some(egui::Color32::from_rgb(28, 51, 42));
        style.spacing.item_spacing = egui::vec2(12.0, 14.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
        cc.egui_ctx.set_style(style);
        let (sender, receiver) = mpsc::channel();
        Self {
            runtime,
            sender,
            receiver,
            phones: vec![],
            selected: None,
            scanning: false,
            next_scan: Instant::now(),
            scan_error: None,
            task: None,
            generation: 0,
            message: "Keep Receive from Computer open in Spoofed.".into(),
            success: false,
            error: false,
        }
    }

    fn scan(&mut self, ctx: &egui::Context) {
        self.scanning = true;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        self.runtime.spawn(async move {
            let result = tokio::time::timeout(Duration::from_secs(8), pairing::discover())
                .await
                .unwrap_or_else(|_| {
                    Err("Checking your iPhone took too long. Reconnect the USB cable.".into())
                });
            let _ = sender.send(Event::Phones(result));
            ctx.request_repaint();
        });
    }

    fn start(&mut self, phone: Phone, ctx: &egui::Context) {
        self.generation += 1;
        let generation = self.generation;
        self.success = false;
        self.error = false;
        self.message = "Connecting to your iPhone…".into();
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        self.task = Some(self.runtime.spawn(async move {
            let progress_sender = sender.clone();
            let progress_ctx = ctx.clone();
            let result = tokio::time::timeout(Duration::from_secs(180), pairing::setup(phone, move |message| {
                let _ = progress_sender.send(Event::Stage(generation, message.into()));
                progress_ctx.request_repaint();
            })).await.unwrap_or_else(|_| Err("Setup timed out. Unlock your iPhone, approve Trust, and restart Receive from Computer before trying again.".into()));
            let _ = sender.send(Event::Finished(generation, result));
            ctx.request_repaint();
        }));
    }
}

impl eframe::App for Helper {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                Event::Phones(result) => {
                    self.scanning = false;
                    self.next_scan = Instant::now() + Duration::from_secs(2);
                    match result {
                        Ok(phones) => {
                            self.scan_error = None;
                            if !phones
                                .iter()
                                .any(|p| Some(&p.device.udid) == self.selected.as_ref())
                            {
                                self.selected = if phones.len() == 1 {
                                    Some(phones[0].device.udid.clone())
                                } else {
                                    None
                                };
                            }
                            self.phones = phones;
                        }
                        Err(e) => {
                            self.scan_error = Some(e);
                            self.phones.clear();
                            self.selected = None;
                        }
                    }
                }
                Event::Stage(generation, message) if generation == self.generation => {
                    self.message = message
                }
                Event::Finished(generation, result) if generation == self.generation => {
                    self.task = None;
                    self.next_scan = Instant::now();
                    match result {
                        Ok(Outcome::Accepted) => {
                            self.success = true;
                            self.message = "Pairing received by Spoofed. You can unplug your iPhone, connect LocalDevVPN, and return to the map. Your first connection checks developer access.".into();
                        }
                        Ok(Outcome::Delivered) => {
                            self.message = "Pairing sent. Check Spoofed for “Pairing received” before unplugging. If it isn't there, restart Receive from Computer and try again.".into();
                        }
                        Err(e) => {
                            self.error = true;
                            self.message = e;
                        }
                    }
                }
                _ => {}
            }
        }
        if self.task.is_none() && !self.scanning && Instant::now() >= self.next_scan {
            self.scan(ctx);
        }
        ctx.request_repaint_after(Duration::from_millis(250));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(egui::Color32::from_rgb(242,246,240)).inner_margin(32)).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(egui::RichText::new("SPOOFED HELPER").size(12.0).strong().color(egui::Color32::from_rgb(82,111,90)));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(if self.success { "You're connected." } else { "Let's connect your iPhone." }).size(29.0).strong());
                ui.label("One cable. No pairing files to move.");
                ui.add_space(12.0);
                egui::Frame::new().fill(egui::Color32::WHITE).corner_radius(16).inner_margin(22).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(egui::RichText::new("1   Open Spoofed on your iPhone").strong());
                    ui.label("Go to Setup > Receive from Computer. Keep that screen open.");
                    ui.add_space(3.0);
                    ui.label(egui::RichText::new("2   Plug in and unlock your iPhone").strong());
                    ui.label("Use a USB data cable. Tap Trust and enter your passcode if asked.");
                    ui.add_space(3.0);
                    ui.label(egui::RichText::new("3   Set up your iPhone below").strong());
                    ui.label("We'll create the pairing and send it straight into Spoofed.");
                });
                ui.add_space(8.0);
                ui.add_enabled_ui(self.task.is_none(), |ui| {
                    if self.phones.len() > 1 {
                        egui::ComboBox::from_id_salt("phone").width(ui.available_width()-20.0)
                            .selected_text(self.phones.iter().find(|p| Some(&p.device.udid) == self.selected.as_ref()).map(|p| p.name.as_str()).unwrap_or("Choose your iPhone"))
                            .show_ui(ui, |ui| {
                                for p in &self.phones {
                                    let suffix: String = p.device.udid.chars().rev().take(4).collect::<String>().chars().rev().collect();
                                    ui.selectable_value(&mut self.selected, Some(p.device.udid.clone()), format!("{} · {}", p.name, suffix));
                                }
                            });
                    } else if let Some(phone) = self.phones.first() { ui.label(format!("Connected: {}", phone.name)); }
                    else { ui.label(self.scan_error.as_deref().unwrap_or("Waiting for your iPhone…")); }
                });
                let selected = self.phones.iter().find(|p| Some(&p.device.udid) == self.selected.as_ref()).cloned();
                let busy = self.task.is_some();
                let label = if busy { "Setting up…" } else if self.error { "Try again" } else if self.success { "Set up again" } else { "Set up iPhone" };
                let button = egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE).strong())
                    .fill(egui::Color32::from_rgb(35,71,58)).corner_radius(12).min_size(egui::vec2(ui.available_width(), 48.0));
                if ui.add_enabled(!busy && selected.is_some(), button).clicked() {
                    if let Some(phone) = selected { self.start(phone, ctx); }
                }
                if busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        if ui.button("Cancel").clicked() {
                            if let Some(task) = self.task.take() { task.abort(); }
                            self.generation += 1;
                            self.message = "Setup stopped. If pairing was already sent, check Spoofed. To retry, restart Receive from Computer.".into();
                        }
                    });
                }
                ui.label(egui::RichText::new(&self.message).color(if self.error { egui::Color32::from_rgb(155,48,35) } else { egui::Color32::from_rgb(61,91,67) }));
                ui.add_space(6.0);
                ui.separator();
                ui.small("Pairing transfers over USB. No account, uploads, or background helper.");
                ui.horizontal_wrapped(|ui| {
                    ui.hyperlink_to("Setup help", "https://spoofed1.github.io/#help");
                    if cfg!(windows) { ui.hyperlink_to("Get Apple Devices", "https://apps.microsoft.com/detail/9np83lwlpz9k"); }
                    ui.label(egui::RichText::new(concat!("Preview ", env!("CARGO_PKG_VERSION"))).small().weak());
                });
            });
        });
    }
}

fn main() -> eframe::Result {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Couldn't start the helper");
    // A device-discovery smoke check that never pairs or logs identifiers.
    if std::env::args().any(|arg| arg == "--probe") {
        match runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(10), pairing::discover()).await
        }) {
            Ok(Ok(phones)) => println!("USB discovery succeeded: {} device(s)", phones.len()),
            Ok(Err(e)) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
            Err(_) => {
                eprintln!("USB discovery timed out");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 680.0])
            .with_min_inner_size([420.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Spoofed Helper",
        options,
        Box::new(move |cc| Ok(Box::new(Helper::new(cc, runtime)))),
    )
}
