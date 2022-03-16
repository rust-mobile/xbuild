use nativeshell::{
    codec::Value,
    shell::{exec_bundle, register_observatory_listener, Context, ContextOptions},
};

fn main() {
    exec_bundle();
    register_observatory_listener("app_template".into());

    env_logger::builder().format_timestamp(None).init();

    let context = Context::new(ContextOptions {
        app_namespace: "AppTemplate".into(),
        ..Default::default()
    });

    let context = context.unwrap();

    context
        .window_manager
        .borrow_mut()
        .create_window(Value::Null, None)
        .unwrap();

    context.run_loop.borrow().run();
}
