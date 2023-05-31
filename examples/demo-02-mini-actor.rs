#![allow(unused)]
#![feature(box_patterns)]
#![feature(test)]
extern crate test;

use futures::channel::oneshot;
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

// Actor 具有的特征
pub trait Actor: 'static + Send + Sync {}

// Actor 对外的引用
// 线程内共享 Arc，线程间共享 clone
pub struct Address<A: Actor> {
    inner: Arc<AddressInner<A>>,
}

struct AddressInner<A: Actor> {
    tx: UnboundedSender<MessageHandler<A>>,
}

impl<A: Actor> Address<A> {
    fn new(tx: UnboundedSender<MessageHandler<A>>) -> Self {
        Self {
            inner: Arc::new(AddressInner { tx: tx }),
        }
    }

    // 请求并等待回复
    async fn call<M>(&self, msg: M) -> Result<M::Result, AddressErr>
    where
        A: Handler<M>,
        M: Message + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let msg = Box::new(ActorMessage::new(msg, Some(tx)));
        let res = self.inner.tx.send(msg);
        match res {
            Ok(_) => match rx.await {
                Ok(res) => Ok(res),
                Err(e) => {
                    println!("rx err, {:?}", e);
                    Err(AddressErr::ReceiverError)
                }
            },
            Err(e) => {
                println!("tx err, {}", e);
                Err(AddressErr::SenderInvalid)
            }
        }
    }

    // 只发送
    fn send<M>(&self, msg: M) -> Result<(), AddressErr>
    where
        A: Handler<M>,
        M: Message + 'static,
    {
        let msg = Box::new(ActorMessage::new(msg, None));
        let res = self.inner.tx.send(msg);
        match res {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("tx err, {}", e);
                Err(AddressErr::SenderInvalid)
            }
        }
    }

    async fn call_exec<F, R>(&self, f: F) -> Result<R, AddressErr>
    where
        F: 'static + (FnMut(&mut A) -> R) + Send + Sync,
        R: 'static + Send + Sync,
    {
        self.call(Exec::new(f)).await
    }

    fn send_exec<F>(&self, f: F) -> Result<(), AddressErr>
    where
        F: 'static + (FnMut(&mut A) -> ()) + Send + Sync,
    {
        self.send(Exec::new(f))
    }

    // // 回调
    // fn callback<M>(&self, msg: M) {}

    // // 函数发送到 actor 中执行，并返回执行结果
    // async fn exec_sync<M>(&self, msg: M) {}
    // // 函数发送到 actor 中执行，只管发送成功，不期待结果
    // fn exec_async<M>(&self, msg: M) {}
}

impl<A: Actor> Clone for Address<A> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

type MessageHandler<A> = Box<dyn ActorMessageHandler<A> + Sync + Send>;

// 通过 channel 实际传递的消息需要的特征
trait ActorMessageHandler<A>: Sync + Send
where
    A: Actor,
{
    fn handle(&mut self, actor: &mut A, ctx: &mut ActorContext<A>);
}

// channel 传递的send/notify消息结构
struct ActorMessage<A, M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    msg: Option<M>,
    tx: Option<oneshot::Sender<M::Result>>,
    create_at: Instant,
    _phan: PhantomData<A>,
}

impl<A, M> ActorMessage<A, M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn new(msg: M, tx: Option<oneshot::Sender<M::Result>>) -> ActorMessage<A, M> {
        ActorMessage {
            msg: Some(msg),
            tx: tx,
            create_at: Instant::now(),
            _phan: PhantomData,
        }
    }

    fn handle(&mut self, actor: &mut A, ctx: &mut ActorContext<A>) {
        // println!("ActorMessage::handle");

        if let Some(msg) = self.msg.take() {
            let result = actor.handle(msg, ctx);
            self.response(result);
        } else {
            println!("handle err, error message");
            // self.response(Err(AddressErr::MessageError));
        }
    }

    fn response(&mut self, result: M::Result) {
        if let Some(tx) = self.tx.take() {
            match tx.send(result) {
                Ok(_) => {}
                Err(e) => {
                    println!("response err");
                }
            }
        }
    }
}

// 让 Actormessge 可以被传递
impl<A, M> ActorMessageHandler<A> for ActorMessage<A, M>
where
    A: Actor + Handler<M>,
    M: Message,
{
    fn handle(&mut self, actor: &mut A, ctx: &mut ActorContext<A>) {
        // TODO 这里的 handle dispatch 是个什么规则 ?
        // ActorMessageHandler::handle 和 ActorMessage::handle 签名一样
        // 不写 ActorMessage::handle 会死循环，写了能正确 dispatch
        self.handle(actor, ctx)
        //
        // TODO 看看文档
        // 这里其实可以直接写逻辑的, 不过看起来不能添加方法，是为什么
    }
}

// Actor 的运行环境
struct ActorContext<A: Actor> {
    addr: Address<A>,
}

// Actor 的运行环境
impl<A> ActorContext<A>
where
    A: Actor,
{
    fn new(addr: Address<A>) -> Self {
        ActorContext { addr }
    }

    fn addr(&self) -> Address<A> {
        todo!("");
        // self.addr.0.as_any().downcast_ref::<Address<A>>().expect("address").clone()
    }

    async fn run(&mut self, mut actor: A, mut rx: UnboundedReceiver<MessageHandler<A>>) {
        println!("run");
        while let Some(mut msg) = rx.recv().await {
            // println!("recv, {:?}", msg)
            msg.handle(&mut actor, self);
        }
    }
}

// ActorRef 特征接口
trait AddressInterface {
    fn as_any(&self) -> &dyn Any {
        todo!();
        // self
    }
}

impl<A: Actor> AddressInterface for Address<A> {}

// Address 调用错误
#[derive(Debug, PartialEq)]
enum AddressErr {
    SenderInvalid,
    ReceiverError,
    MessageError,
}

// Actor 消息处理特征
trait Message: Sync + Send {
    type Result: Sync + Send;
}

// Actor 消息处理特征
// #[async_trait]
trait Handler<M>
where
    Self: Actor + Sized,
    M: Message,
{
    fn handle(&mut self, msg: M, ctx: &mut ActorContext<Self>) -> M::Result;
}

struct ActorSystem {}

impl ActorSystem {
    fn new() -> Self {
        ActorSystem {}
    }

    fn add_actor<A>(&self, actor: A) -> Address<A>
    where
        A: Actor,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        let addr = Address::new(tx);

        let cloned_addr = addr.clone();
        tokio::spawn(async move {
            ActorContext::new(cloned_addr).run(actor, rx).await;
        });

        addr
    }
}


struct Exec<F, A, R> 
where
    F: (FnMut(&mut A) -> R)
{
    f: F,
    _a: PhantomData<A>,
}

impl <F, A, R> Exec<F, A, R>
where
    F: (FnMut(&mut A) -> R),
    R: Send + Sync,
{
    fn new(f: F) -> Exec<F, A, R> {
        Exec { f: f, _a: PhantomData }
    }
}

impl <F, A, R> Message for Exec<F, A, R>
where
    F: (FnMut(&mut A) -> R) + Send + Sync,
    A: Actor,
    R: Send + Sync,
{
    type Result = R;
}

impl <F, A, R> Handler<Exec<F, A, R>> for A
where
    A: Actor,
    F: (FnMut(&mut A) -> R) + Send + Sync,
    R: Send + Sync,
{
    fn handle(&mut self, msg: Exec<F, A, R>, ctx: &mut ActorContext<Self>) -> <Exec<F, A, R> as Message>::Result {
        let mut f = msg.f;
        f(self)
    }
}


fn main() {}

#[cfg(test)]
mod tests {

    use std::{collections::HashMap, time, vec};
    use test::Bencher;

    use super::*;

    mod test_actor {
        use super::*;

        #[derive(Debug)]
        pub struct Set(pub String, pub i32);

        impl Message for Set {
            type Result = bool;
        }

        #[derive(Debug)]
        pub struct Get(pub String);

        impl Message for Get {
            type Result = Option<i32>;
        }

        pub struct MyActor {
            store: HashMap<String, i32>,
        }

        impl Actor for MyActor {}

        impl MyActor {
            pub fn new() -> Self {
                MyActor {
                    store: HashMap::new(),
                }
            }

            pub fn get(&self, k: String) -> Option<i32> {
                if let Some(r) = self.store.get("hello".into()) {
                    (*r).into()
                } else {
                    None
                }
            }

            pub fn set(&mut self, k: String, v: i32) {
                self.store.insert(k, v);
            }
        }

        impl Handler<Set> for MyActor {
            fn handle(
                &mut self,
                msg: Set,
                ctx: &mut ActorContext<Self>,
            ) -> <Set as Message>::Result {
                self.store.insert(msg.0, msg.1);
                return true;
            }
        }

        impl Handler<Get> for MyActor {
            fn handle(
                &mut self,
                msg: Get,
                ctx: &mut ActorContext<Self>,
            ) -> <Get as Message>::Result {
                if let Some(v) = self.store.get(&msg.0) {
                    Some(v.clone())
                } else {
                    None
                }
            }
        }

    }

    #[tokio::test(flavor = "multi_thread")]
    async fn hello_actor() {
        use super::*;

        struct MyActor {};
        impl Actor for MyActor {};

        #[derive(Debug)]
        struct Set {};
        impl Message for Set {
            type Result = bool;
        };

        impl Handler<Set> for MyActor {
            fn handle(
                &mut self,
                msg: Set,
                ctx: &mut ActorContext<Self>,
            ) -> <Set as Message>::Result {
                println!("handle {:?} on tid {:?}", msg, thread::current().id());
                true
            }
        }

        let sys = ActorSystem::new();

        let mut addrs = vec![];
        for i in 0..3 {
            let addr = sys.add_actor(MyActor {});
            addrs.push(addr);
        }

        for i in 0..10 {
            if let Some(addr) = addrs.get(i % 3) {
                let addr = addr.clone();
                tokio::spawn(async move {
                    let ret = addr.call(Set {}).await;
                    println!("ret {:?}", ret);
                });
            }
        }
        thread::sleep(time::Duration::from_secs(5));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn transfer() {
        // 请求回复测试
        use test_actor::*;

        let sys = ActorSystem::new();
        let addr = sys.add_actor(MyActor::new());

        addr.send(Set("hello".into(), 1));

        addr.call(Set("hello".into(), 1)).await;
        assert_eq!(addr.call(Get("hello".into())).await, Ok(Some(1)));

        let res = addr.send_exec(|mut actor| actor.set("hello".into(), 2));

        let res = addr.call_exec(|mut actor| {
            actor.get("hello".into())
        }).await;
        assert_eq!(res, Ok(Some(2)));
    }

    // 对比原生线程和actor模式下消息的吞吐能力差异
    mod through {

        pub mod thread_channel {
            use std::thread;
            use std::sync::mpsc;
            
            #[derive(Debug)]
            pub enum Message {
                Msg,
                Done(mpsc::Sender<Message>),
                Closed,
            }

            pub struct Handle(pub thread::JoinHandle<Box<Message>>, pub mpsc::Sender<Box<Message>>);

            pub fn spawn() -> Handle {
                let (tx, rx) = mpsc::channel::<Box<Message>>();
                
                let handle = thread::spawn(move || loop {
                    match rx.recv() {
                        Ok(box Message::Done(tx)) => {tx.send(Message::Closed);},
                        Err(_) => {},
                        data => {
                            // println!("recv {:?}", data);
                        },
                    }
                });
                Handle(handle, tx)
            }

            pub fn send_n(addr: &mpsc::Sender<Box<Message>>, n: i32) {
                for _ in 0..n {
                    // println!("send");
                    let _ = addr.send(Box::new(Message::Msg)).unwrap();
                }
                let (tx, rx) = mpsc::channel();
                addr.send(Box::new(Message::Done(tx))).unwrap();
                rx.recv().unwrap();
                // println!("done");
            }

        }

        pub mod actor {
            use crate::*;

            #[derive(Debug)]
            pub struct Msg;

            impl Message for Msg {
                type Result = ();
            }

            pub struct MyActor;

            impl Actor for MyActor {}

            impl Handler<Msg> for MyActor {
                fn handle(&mut self, msg: Msg, ctx: &mut crate::ActorContext<Self>) -> <Msg as crate::Message>::Result {
                    // println!("handle {:?}", msg);
                }
            }

            pub async fn send_n(addr: &Address<MyActor>, n: i32) {
                for _ in 0..n {
                    let _ = addr.send(Msg);
                }
                let _ = addr.call(Msg).await;
            }

        }
    }

    #[test]
    fn test_actor_send() {
        use through::actor;

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let sys = ActorSystem::new();
        runtime.block_on(async {
            let addr = sys.add_actor(actor::MyActor);
            actor::send_n(&addr, 1).await;
        });
    }

    //  2,259,051 ns/iter
    // 换算下来每秒 4000k 消息，对 c100k 应该够了
    // 先用这个，不行后面另外实现一套简化版的运行时
    #[bench]
    fn bench_actor_send_10000(b: &mut Bencher) {
        use through::actor;

        let runtime = tokio::runtime::Builder::new_multi_thread().build().unwrap();

        // let runtime = tokio::runtime::Runtime::new().unwrap();
        let sys = ActorSystem::new();
        let addr = runtime.block_on(async {sys.add_actor(actor::MyActor)});
        b.iter(|| runtime.block_on(test::black_box(actor::send_n(&addr, 10000))));
    }

    #[test]
    fn test_thread_send() {
        use through::thread_channel;
        let handle = thread_channel::spawn();
        thread_channel::send_n(&handle.1, 1);
    }

    // 266,211 ns/iter
    // tokio actor 性能差了 10 倍？
    // Box 后性能影响较大，就和 tokio 差不多了
    // 1,233,553 ns/iter
    //
    // TODO channel 的实现细节；如何传递对象的
    #[bench]
    fn bench_thread_send_10000(b: &mut Bencher) {
        use through::thread_channel;
        let handle = thread_channel::spawn();
        b.iter(|| test::black_box(thread_channel::send_n(&handle.1, 10000)))
    }

}
