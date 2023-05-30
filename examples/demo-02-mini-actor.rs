#![allow(unused)]

use futures::channel::oneshot;
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

// Actor 具有的特征
trait Actor: 'static + Send + Sync {}

// Actor 对外的引用
// 线程内共享 Arc，线程间共享 clone
struct Address<A: Actor> {
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

fn main() {}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time, vec};

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
        // 请求回复
        // 通知
        // 异步回调

        use super::*;

        #[derive(Debug)]
        struct Set(String, i32);

        impl Message for Set {
            type Result = bool;
        }

        #[derive(Debug)]
        struct Get(String);

        impl Message for Get {
            type Result = Option<i32>;
        }

        struct MyActor {
            store: HashMap<String, i32>,
        };

        impl Actor for MyActor {}

        impl MyActor {
            fn new() -> Self {
                MyActor {
                    store: HashMap::new(),
                }
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

        addr.send(Set("hello".into(), 1));

        let sys = ActorSystem::new();
        let addr = sys.add_actor(MyActor::new());
        addr.call(Set("hello".into(), 1)).await;
        assert_eq!(addr.call(Get("hello".into())).await, Ok(Some(1)));
    }
}
