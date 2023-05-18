use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        let _ = tx1.send("one");
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    // 选择执行一个分支，分支执行完成后结束，未完成的分支被丢弃
    tokio::select! {
        val = rx1 => {
            println!("rx1 got {:?}", val);
        }
        val = rx2 => {
            println!("rx2 got {:?}", val);
        }
    }
}