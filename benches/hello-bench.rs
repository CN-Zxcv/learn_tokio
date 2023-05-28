use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn benchmark(c: &mut Criterion) {
    c.bench_function("fib", |b| b.iter(|| fibonacci(black_box(20))));
}

// 不支持 vscode 直接调用，只能通过命令行？
// 感觉不太好用的样子；内置 bench 目前够用，先不用这个
criterion_group!(benchs, benchmark);
criterion_main!(benchs);
