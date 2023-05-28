#![allow(unused)]
#![feature(test)]
extern crate test;

use lazy_static::*;
use std::collections::VecDeque;
use std::sync::Mutex;

use tokio::sync::mpsc;
use tokio::sync::oneshot::{self, Sender};

fn run_channel() {
    let (tx, mut rx) = oneshot::channel();
    tx.send(1);
    let _ = rx.try_recv();
}

fn run_empty() {}

struct MpscChannel {
    tx: mpsc::UnboundedSender<i32>,
    rx: mpsc::UnboundedReceiver<i32>,
}

impl MpscChannel {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        MpscChannel { tx, rx }
    }

    fn send(&self, i: i32) {
        self.tx.send(i);
    }

    async fn recv(&mut self) -> Option<i32> {
        self.rx.recv().await
    }
}

fn run_mpsc(channel: &mut MpscChannel) {
    channel.send(1);
    channel.recv();
}

lazy_static! {
    static ref QUE: Mutex<VecDeque<i32>> = Mutex::new(VecDeque::new());
}

fn run_push(que: &Mutex<VecDeque<i32>>) {
    {
        que.lock().unwrap().push_back(1);
    }
    {
        que.lock().unwrap().pop_front();
    }
}

fn run_push_parking_lot(que: &parking_lot::Mutex<VecDeque<i32>>) {
    {
        que.lock().push_back(1);
    }
    {
        que.lock().pop_front();
    }
}

fn run_push_raw(que: &mut VecDeque<i32>) {
    {
        que.push_back(1);
    }
    {
        que.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    // 60ns
    #[bench]
    fn bench_channel(b: &mut Bencher) {
        b.iter(|| test::black_box(run_channel()));
    }

    #[bench]
    fn bench_empty(b: &mut Bencher) {
        b.iter(|| test::black_box(run_empty()));
    }

    #[test]
    fn test_mpsc() {
        let mut channel = MpscChannel::new();
        run_mpsc(&mut channel);
    }

    // 45ns
    #[bench]
    fn bench_mpsc(b: &mut Bencher) {
        let mut channel = MpscChannel::new();
        b.iter(|| test::black_box(run_mpsc(&mut channel)));
    }

    // 36 ns
    #[bench]
    fn bench_push(b: &mut Bencher) {
        let que = Mutex::new(VecDeque::new());
        b.iter(|| test::black_box(run_push(&que)));
    }

    // 33 ns
    #[bench]
    fn bench_push_parking_lot(b: &mut Bencher) {
        let que = parking_lot::Mutex::new(VecDeque::new());
        b.iter(|| test::black_box(run_push_parking_lot(&que)));
    }

    // 4 ns
    #[bench]
    fn bench_push_raw(b: &mut Bencher) {
        let mut que = VecDeque::new();
        b.iter(|| test::black_box(run_push_raw(&mut que)));
    }
}

// 结论
// * oneshut channel 的消耗大约是 mutex 消息队列的两倍，不是极端情况下，使用 oneshot
//   完成线程间 await 是可以接受的；再看看其他方案，目前可以选这个办法
// * 线程发消息时，通过批量发送来减少临界区访问，可能会有较大的性能提升；
//   可以每x毫秒内的消息打一个包，以适当的延迟换取性能

fn main() {}
