use std::println;

use tokio_stream::StreamExt;
use mini_redis::client;

async fn publish() -> mini_redis::Result<()> {
    let mut client = client::connect("localhost:6379").await?;

    client.publish("numbers", "1".into()).await?;
    client.publish("numbers", "two".into()).await?;
    client.publish("numbers", "3".into()).await?;
    client.publish("numbers", "4".into()).await?;
    client.publish("numbers", "5".into()).await?;

    Ok(())
}

async fn subscribe() -> mini_redis::Result<()> {
    let client = client::connect("localhost:6379").await?;
    // 订阅
    let subscriber = client.subscribe(vec!["numbers".to_string()]).await?;
    let messages = subscriber.into_stream();

    tokio::pin!(messages);

    // 接收数据
    while let Some(msg) = messages.next().await {
        println!("got: {:?}", msg);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> mini_redis::Result<()> {
    tokio::spawn(async {
        publish().await
    });

    subscribe().await?;

    Ok(())
}