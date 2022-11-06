use dioxus::prelude::*;

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn start_app() {
    use wry::android_binding;

    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::Level::Trace)
            .with_tag("template"),
    );

    android_binding!(com_example, template, _start_app);
}

#[cfg(target_os = "android")]
fn _start_app() {
    if let Err(err) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(main)) {
        eprintln!("attempt to unwind out of `rust` with err: {:?}", err);
        std::process::abort();
    }
}

pub fn main() {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    std::env::set_var("RUST_BACKTRACE", "1");

    dioxus_desktop::launch(app);
}

fn app(cx: Scope) -> Element {
    cx.render(rsx! {
        div {
            "hello world"
        }
    })
}
