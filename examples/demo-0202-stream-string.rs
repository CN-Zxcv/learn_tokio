use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, TcpListener};
use tokio::sync::mpsc;
use tokio_util::codec::{Framed, LinesCodec};

type LineFramedStream = SplitStream<Framed<TcpStream, LinesCodec>>;
type LineFramedSink = SplitSink<Framed<TcpStream, LinesCodec>, String>;

#[tokio::main]
async fn main() {
    let server = TcpListener::bind("localhost:8888").await.unwrap();
    
    let t1 = tokio::spawn(async move {
        while let Ok((client_stream, _)) = server.accept().await {
            tokio::spawn(async move {
                process_client_string(client_stream).await;
            });
        }
    });

    let t2 = tokio::spawn(async move {
        simulate_client_string().await;
    });

    let _ = tokio::join!(t1, t2);
}

async fn process_client_string(client_stream: TcpStream) {
    // TcpStream 转换为 Framed
    let framed = Framed::new(client_stream, LinesCodec::new());
    // 读写分离
    let (frame_writer, frame_reader) = framed.split::<String>();

    let (msg_tx, msg_rx) = mpsc::channel::<String>(100);

    let mut read_task = tokio::spawn(async move {
        read_from_client_string(frame_reader, msg_tx).await;
    });

    let mut write_task = tokio::spawn(async move {
        write_to_client_string(frame_writer, msg_rx).await;
    });

    if tokio::try_join!(&mut read_task, &mut write_task).is_err() {
        eprintln!("terminated");
        read_task.abort();
        write_task.abort();
    }
}

async fn read_from_client_string(mut reader: LineFramedStream, msg_tx: mpsc::Sender<String>) {
    loop {
        match reader.next().await {
            None => {
                println!("client closed");
                break;
            }
            Some(Err(e)) => {
                eprintln!("read from client error: {}", e);
                break;
            }
            Some(Ok(str)) => {
                println!("read from client, content: {}", str);
                if msg_tx.send(str).await.is_err() {
                    eprintln!("rx closed");
                }
            }
        }
    }
}

async fn write_to_client_string(mut writer: LineFramedSink, mut msg_rx: mpsc::Receiver<String>) {
    while let Some(str) = msg_rx.recv().await {
        if writer.send(str).await.is_err() {
            eprintln!("write to client failed");
            break;
        }
    }
}

async fn simulate_client_string() {
    let mut stream = TcpStream::connect("localhost:8888").await.unwrap();

    println!("connected");

    if stream.write("hello\nworld".as_bytes()).await.is_err() {
        eprintln!("write failed");
    }
}
