#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use pacing::MainWindow;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eframe::run_native(
        "Pacing",
        Default::default(),
        Box::new(|cc| Box::new(MainWindow::new(cc))),
    )
    .unwrap()
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
