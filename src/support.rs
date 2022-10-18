use std::time::Instant;

#[allow(unused)]
pub fn print_time_of<R, F: FnOnce() -> R>(f: F, tag: &str) -> R {
    let start = Instant::now();
    let res = f();
    println!(
        "{tag}: {time} ms.",
        tag = tag,
        time = start.elapsed().as_millis()
    );
    res
}
