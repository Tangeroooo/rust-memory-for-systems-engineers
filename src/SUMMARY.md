# 목차

- [들어가며](introduction.md)
- [출처와 권위 읽는 법](source-policy.md)

# 1부. 하나의 메모리 문제가 아니다

- [일곱 계층 mental model](01-mental-model/01-layers.md)
- [Rust가 막는 문제와 막지 않는 문제](01-mental-model/02-safety-boundary.md)
- [C/C++에서 Rust로 옮겨오는 mental model](01-mental-model/03-from-cpp.md)

# 2부. Language — 값과 접근 권한

- [Ownership과 move](02-language/04-ownership-move.md)
- [Borrowing과 lifetime](02-language/05-borrowing-lifetime.md)
- [Concurrency와 data race](02-language/06-concurrency.md)

# 3부. Object lifetime — 파괴 시점

- [Drop, RAII, destructor scope](03-object-lifetime/07-drop.md)
- [Leak과 reference cycle](03-object-lifetime/08-leaks-cycles.md)

# 4부. Collections & heap — 크기가 변하는 값

- [Box, Vec, String의 표현](04-collections-heap/09-box-vec-string.md)
- [HashMap과 Arc의 비용](04-collections-heap/10-hashmap-arc.md)
- [Capacity, reserve, shrink](04-collections-heap/11-capacity.md)

# 5부. Allocator — 반납과 재사용

- [Allocation과 deallocation](05-allocator/12-allocation-deallocation.md)
- [Fragmentation과 allocator retention](05-allocator/13-fragmentation-retention.md)

# 6부. OS — virtual memory와 관측값

- [Virtual memory, page, RSS](06-os/14-virtual-memory-rss.md)
- [Overcommit, page fault, OOM](06-os/15-overcommit-oom.md)

# 7부. Resource control — 실패와 한도

- [RFC 2116과 allocation failure](07-resource-control/16-rfc-2116.md)
- [cgroup v2와 OOM kill](07-resource-control/17-cgroup.md)

# 8부. Server/DB governance — 예산과 제어

- [Memory budget과 admission](08-governance/18-budget-admission.md)
- [Spill, eviction, backpressure](08-governance/19-spill-eviction.md)
- [관측과 검증 전략](08-governance/20-observability-testing.md)

# 부록

- [용어집](glossary.md)
- [참고 문헌](references.md)
