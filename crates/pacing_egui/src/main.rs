// hide the console in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pacing_egui::MainWindow;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use tray_icon::TrayIconBuilder;

    let (icon, tray_icon) = {
        const DATA: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/icon.png"));
        let img = ::image::load_from_memory_with_format(DATA, ::image::ImageFormat::Png)
            .expect("valid icon");

        let (width, height) = (img.width(), img.height());
        let bytes = img.into_bytes();
        (
            eframe::IconData {
                width,
                height,
                rgba: bytes.clone(),
            },
            tray_icon::icon::Icon::from_rgba(bytes, width, width).unwrap(),
        )
    };

    let _tray_icon = TrayIconBuilder::new()
        .with_tooltip("Pacing")
        .with_icon(tray_icon)
        .with_tooltip("Toggle Pacing")
        .build()
        .unwrap();

    eframe::run_native(
        "Pacing",
        eframe::NativeOptions {
            icon_data: Some(icon),
            ..Default::default()
        },
        Box::new(|cc| Box::new(MainWindow::new(cc))),
    )
    .unwrap();
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();

    tracing_wasm::set_as_global_default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::start_web(
            "pacing_canvas",
            Default::default(),
            Box::new(|cc| Box::new(MainWindow::new(cc))),
        )
        .await
        .expect("failed to start eframe");
    });
}
