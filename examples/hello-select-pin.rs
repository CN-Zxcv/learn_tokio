use std::println;


async fn action(input: Option<i32>) -> Option<String> {
    let i = match input {
        Some(input) => input,
        None => return None,
    };
    Some("hello".into())
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<i32>(128);

    let mut done = false;
    let operation = action(None);
    // 要 await 一个引用，必须 pin!
    // TODO pin! 是什么
   tokio::pin!(operation);

    tokio::spawn(async move {
        let _ = tx.send(1).await;
        let _ = tx.send(2).await;
        let _ = tx.send(3).await;
    });

    loop {
        tokio::select! {
            // 不加 if !done 会发生 '`async fn` resumed after completion'
            // 引用在操作完成后依然存在,所以要在操作完成后手动禁用分支
            res = &mut operation, if !done => {
                done = true;

                if let Some(v) = res {
                    println!("Got {}", v);
                    return;
                }
            }
            Some(v) = rx.recv() => {
                if v % 2 == 0 {
                    operation.set(action(Some(v)));
                    done = false;
                }
            }
        }
    }

}