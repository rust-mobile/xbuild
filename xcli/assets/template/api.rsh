fn create_counter_state() -> CounterState;

object CounterState {
    fn increment();
    fn counter() -> u32;
    fn subscribe() -> Stream<i32>;
}
