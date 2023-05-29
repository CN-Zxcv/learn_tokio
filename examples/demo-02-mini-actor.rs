#![allow(unused)]

use futures::channel::oneshot;
use tokio::sync::mpsc::{UnboundedSender, self};
use std::marker::PhantomData;
use std::sync::Arc;
use std::any::Any;
use std::time::Instant;

// Actor 具有的特征
trait Actor: 'static + Send + Sync {
    type Context: Any;
}

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
            inner: Arc::new(AddressInner { tx: tx })
        }
    }

    async fn send<M>(&self, msg: M) -> Result<M::Result, AddressErr> where
        A: Handler<M>,
        M: Message + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let msg = Box::new(ActorMessage::new(msg, Some(tx)));
        if let Ok(_) = self.inner.tx.send(msg) {
            match rx.await {
                Ok(res) => Ok(res),
                Err(_) => Err(AddressErr::ReceiverError),
            }

        } else {
            Err(AddressErr::SenderInvalid)
        }

    }
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
trait ActorMessageHandler<A>: Sync + Send where
    A: Actor
{
    fn handle(&mut self, actor: &mut A, ctx: &mut ActorContext<A>);
}

// channel 传递的send/notify消息结构
struct ActorMessage<A, M> where
    A: Actor + Handler<M>,
    M: Message
{
    msg: Option<M>,
    tx: Option<oneshot::Sender<M::Result>>,
    create_at: Instant,
    _phan: PhantomData<A>,
}

impl<A, M> ActorMessage<A, M> where 
    A: Actor + Handler<M>,
    M: Message,
{
    fn new(msg: M, tx: Option<oneshot::Sender<M::Result>>) -> ActorMessage<A, M> {
        ActorMessage { msg: Some(msg), tx: tx, create_at: Instant::now(), _phan: PhantomData }
    }

}

// 让 Actormessge 可以被传递
impl<A, M> ActorMessageHandler<A> for ActorMessage<A, M> where
    A: Actor + Handler<M>,
    M: Message,
{
    fn handle(&mut self, actor: &mut A, ctx: &mut ActorContext<A>) {
        self.handle(actor, ctx)
    }
}

// Actor 的运行环境
struct ActorContext<A: Actor> {
    addr: Address<A>
}

// Actor 的运行环境
impl<A: Actor> ActorContext<A> {
    fn new(addr: Address<A>) -> Self {
        ActorContext {
            addr,
        }
    }

    fn addr(&self) -> Address<A> {
        todo!("");
        // self.addr.0.as_any().downcast_ref::<Address<A>>().expect("address").clone()
    }

    fn run(&self) {
        println!("run");
    }
}


struct BoxedAddress(Arc<dyn AddressInterface>);

// ActorRef 特征接口
trait AddressInterface {
    fn as_any(&self) -> &dyn Any {
        todo!();
        // self
    }
}

impl<A: Actor> AddressInterface for Address<A> {}


// Address 调用错误
#[derive(Debug)]
enum AddressErr {
    SenderInvalid,
    ReceiverError,
}

// Actor 消息处理特征
trait Message: Sync + Send {
    type Result: Sync + Send;
}

// Actor 消息处理特征
// #[async_trait]
trait Handler<M> where
    Self: Actor,
    M: Message,
{
    fn handle(&mut self, msg: M, ctx: &mut Self::Context) -> M::Result;
}

struct ActorSystem {
    inner: Arc<ActorSystemInner>,
}

struct ActorSystemInner {}

impl ActorSystem {
    fn new() -> Self {
        ActorSystem { 
            inner:  Arc::new(ActorSystemInner {  })
        }
    }

    fn add_actor<A: Actor>(&self, actor: A) -> Address<A> {
        let (tx, rx) = mpsc::unbounded_channel();
        let addr = Address::new(tx);

        let cloned_addr = addr.clone();
        tokio::spawn(async move {
            ActorContext::new(cloned_addr).run();
        });

        addr
    }
}

fn main() {}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn hello_actor() {
        use super::*;

        struct MyActor {};
        impl Actor for MyActor {
            type Context = ActorContext<Self>;
        };

        #[derive(Debug)]
        struct Set {};
        impl Message for Set {
            type Result = bool;
        };

        impl Handler<Set> for MyActor {
            fn handle(&mut self, msg: Set, ctx: &mut ActorContext<MyActor>) -> <Set as Message>::Result {
                println!("handle {:?}", msg);
                true
            }
        }
        
        let sys = ActorSystem::new();
        let addr = sys.add_actor(MyActor {});
        match addr.send(Set {}).await {
            Ok(done) => println!("done"),
            Err(e) => println!("err {:?}", e),
        }
        
    }
}