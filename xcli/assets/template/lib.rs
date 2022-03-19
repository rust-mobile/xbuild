use futures::{Stream, StreamExt};
use futures::channel::mpsc;

ffi_gen_macro::ffi_gen!("api.rsh");

pub fn create_counter_state() -> CounterState {
    Default::default()
}

#[derive(Default)]
pub struct CounterState {
    counter: u32,
    subscribers: Vec<mpsc::Sender<()>>,
}

impl CounterState {
    pub fn increment(&mut self) {
        self.counter += 1;
        self.subscribers.retain(|tx| match tx.clone().try_send(()) {
            Ok(()) => true,
            Err(err) if err.is_full() => true,
            Err(_) => false,
        });
    }

    pub fn counter(&self) -> u32 {
        self.counter
    }

    pub fn subscribe(&mut self) -> impl Stream<Item = i32> {
        let (tx, rx) = mpsc::channel(1);
        self.subscribers.push(tx);
        rx.map(|_| 0)
    }
}
