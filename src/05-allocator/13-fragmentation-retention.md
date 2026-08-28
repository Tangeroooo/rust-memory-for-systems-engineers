# Fragmentation과 allocator retention

Live object가 줄었는데 allocator가 관리하는 address space나 RSS가 충분히 줄지 않는다면 leak만 의심해서는 안 된다. Fragmentation과 retention은 올바르게 deallocate된 workload에서도 나타날 수 있다.

## 두 종류의 fragmentation

- **internal fragmentation:** 요청보다 큰 size class/block을 받아 block 내부가 남는다.
- **external fragmentation:** free space 총량은 충분하지만 필요한 큰 contiguous block으로 사용하기 어렵다.

Allocator는 throughput과 concurrency를 위해 arena, per-thread cache, size class를 사용할 수 있다. 이 구조는 lock contention을 줄이는 대신 free block을 process 안에 더 오래 보관할 수 있다.

## 네 가지 숫자를 분리한다

```text
logical live bytes
  application이 현재 필요로 하는 값

requested/active allocator bytes
  allocator가 live allocation으로 보는 양

retained/mapped allocator bytes
  allocator가 재사용하려고 보유한 mapping

RSS
  현재 resident한 process page
```

Metric 이름과 정확한 정의는 allocator/tool마다 다르다. 서로 다른 도구의 “allocated”를 같은 값으로 간주하지 않는다.

## 흔한 패턴

1. 큰 request가 여러 thread에서 동시에 allocation한다.
2. request가 끝나 object와 buffer는 정상적으로 drop된다.
3. arena마다 free block이 흩어지고 일부 page에 작은 live block이 남는다.
4. allocator가 mapping 전체를 OS에 반환하기 어렵거나 재사용을 위해 보관한다.
5. 다음 request에서는 빨리 재사용되지만 idle RSS는 높게 보인다.

## 현실적인 판단

- RSS가 높지만 반복 workload에서 더 이상 증가하지 않고 allocation latency가 안정적이면 retention일 수 있다.
- live object와 allocator active bytes가 계속 증가하면 application growth/leak 가능성이 높다.
- thread 수에 따라 baseline이 크게 달라지면 per-thread cache/arena를 확인한다.
- allocator 교체 전 allocation size/lifetime 분포와 tail latency를 측정한다.

Allocator를 바꾸는 것은 application architecture를 대체하지 않는다. Unbounded cache는 더 좋은 allocator에서도 unbounded다.

## 보장 경계

### 이 장이 보장하는 설명

- fragmentation, retention, leak을 구분하는 진단 모델
- allocator throughput 최적화가 RSS trade-off를 가질 수 있다는 점

### 이 장이 보장하지 않는 것

- glibc, jemalloc, mimalloc의 모든 version/configuration이 같은 방식으로 동작한다는 주장
- 높은 RSS가 언제나 harmless retention이라는 주장
- allocator 교체만으로 workload memory upper bound를 보장한다는 주장

### 출처와 권위

- **구현 확인:** [`std::alloc`](https://doc.rust-lang.org/std/alloc/index.html)
- **보조 학습:** [Rust Performance Book — Heap Allocations](https://nnethercote.github.io/perf-book/heap-allocations.html)
- **운영 참고:** 실제 배포 allocator의 공식 문서와 metrics를 별도로 확인해야 한다.
