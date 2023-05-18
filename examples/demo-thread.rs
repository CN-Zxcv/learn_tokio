use futures::executor::block_on;
use tokio;
use tokio::time;
use tokio::time::Duration;
use std::thread;
 
// task 默认会在多个线程中随机分配

async fn infinte(id: i32) {
    let mut tid = thread::current().id();
    loop {
        time::sleep(Duration::from_secs(1)).await;
        let new_id = thread::current().id();
        if tid != new_id {
            tid = new_id;
            println!("task={}, change thread={:?}", id, new_id);
        }
    }
}
 
fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_time()
        .build()
        .unwrap();

    for id in 1 .. 10 {
        runtime.spawn(async move { infinte(id).await });
    }

    runtime.block_on(async { infinte(0).await });
    // let rt = tokio::runtime::Runtime::new().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(30));

}