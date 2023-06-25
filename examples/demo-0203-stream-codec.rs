use bincode;
use bytes::BufMut;
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::codec::{self, Framed};

type LineFramedStream = SplitStream<Framed<TcpStream, RstRespCodec>>;
// type LineFramedSink = SplitSink<Framed<TcpStream, RstRespCodec>, RstResp>;

#[tokio::main]
async fn main() {
    let server = TcpListener::bind("localhost:8888").await.unwrap();

    let t1 = tokio::spawn(async move {
        while let Ok((client_stream, _)) = server.accept().await {
            tokio::spawn(async move {
                process_client_codec(client_stream).await;
            });
        }
    });

    let t2 = tokio::spawn(async move {
        simulate_client_codec().await;
    });

    let _ = tokio::join!(t1, t2);
}

async fn simulate_client_codec() {
    let stream = TcpStream::connect("localhost:8888").await.unwrap();

    let framed = Framed::new(stream, RstRespCodec);
    let (mut frame_writer, _) = framed.split::<RstResp>();

    let resp = RstResp::Response(Response(Some("hello".into())));

    if frame_writer.send(resp).await.is_err() {
        eprintln!("write failed")
    }
}

async fn process_client_codec(client_stream: TcpStream) {
    // TcpStream 转换为 Framed
    let framed = Framed::new(client_stream, RstRespCodec);
    // 读写分离
    let (_, frame_reader) = framed.split::<RstResp>();

    let (msg_tx, _) = mpsc::channel::<RstResp>(100);

    let mut read_task = tokio::spawn(async move {
        read_from_client_codec(frame_reader, msg_tx).await;
    });

    // let mut write_task = tokio::spawn(async move {
    //     write_to_client_string(frame_writer, msg_rx).await;
    // });

    if tokio::try_join!(&mut read_task).is_err() {
        eprintln!("terminated");
        read_task.abort();
    }
}

async fn read_from_client_codec(mut reader: LineFramedStream, _msg_tx: mpsc::Sender<RstResp>) {
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
            Some(Ok(msg)) => {
                println!("read from client, content: {:?}", msg);
            }
        }
    }
}

// 定义协议，需要序列化特征
#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub sym: String,
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response(pub Option<String>);

// 通过enum聚合类型，客户端服务器将基于该类型进行通信
#[derive(Debug, Serialize, Deserialize)]
pub enum RstResp {
    Request(Request),
    Response(Response),
}

// 实现协议类型的 codec
pub struct RstRespCodec;
impl RstRespCodec {
    // 1G
    const MAX_SIZE: usize = 1024 * 1024 * 1024 * 8;
}

impl codec::Encoder<RstResp> for RstRespCodec {
    type Error = bincode::Error;
    fn encode(&mut self, item: RstResp, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        let data = bincode::serialize(&item)?;
        let data = data.as_slice();

        let data_len = data.len();
        if data_len > Self::MAX_SIZE {
            return Err(bincode::Error::new(bincode::ErrorKind::Custom(
                "frame is too large".into(),
            )));
        }

        // | data_len; 4 | payload; n |
        // 准备buf
        dst.reserve(data_len + 4);
        // 写入头，默认大端模式
        dst.put_u32(data_len as u32);
        // 写入数据
        dst.extend_from_slice(data);

        Ok(())
    }
}

impl codec::Decoder for RstRespCodec {
    type Item = RstResp;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let buf_len = src.len();

        // 长度不够
        if buf_len < 4 {
            return Ok(None);
        }

        // 读包头
        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&src[..4]);

        let data_len = u32::from_be_bytes(length_bytes) as usize;
        if data_len > Self::MAX_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("frame len {} too large", data_len),
            ));
        }

        // 数据长度不够，重新申请空闲空间，后续数据到来后重新解析
        let frame_len = data_len + 4;
        if buf_len < frame_len {
            src.reserve(frame_len - buf_len);
            return Ok(None);
        }

        // 取出包体，解析
        let frame_bytes = src.split_to(frame_len);
        match bincode::deserialize::<RstResp>(&frame_bytes[4..]) {
            Ok(frame) => Ok(Some(frame)),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;
    use tokio_util::codec::{Decoder, Encoder};

    use super::*;

    #[test]
    fn test_encode_decode() {
        let item = RstResp::Response(Response(Some("hello".into())));
        let mut codec = RstRespCodec {};
        let mut dst = BytesMut::new();
        codec.encode(item, &mut dst);

        if let Ok(d) = codec.decode(&mut dst) {
            println!("decode {:?}", d);
        }
        // let bin = msg.serialize(serializer)
    }
}
