//! 교재의 핵심 개념을 검증하는 dependency-free 실습 코드다.

use std::collections::TryReserveError;
use std::fmt;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// 필요한 capacity를 먼저 fallible하게 확보한 뒤 복사한다.
pub fn copy_checked(input: &[u8]) -> Result<Vec<u8>, TryReserveError> {
    let mut output = Vec::new();
    output.try_reserve(input.len())?;
    output.extend_from_slice(input);
    Ok(output)
}

/// Process-local application budget의 작은 예시다.
#[derive(Clone, Debug)]
pub struct MemoryPool {
    inner: Arc<PoolInner>,
}

#[derive(Debug)]
struct PoolInner {
    limit: usize,
    reserved: AtomicUsize,
}

#[derive(Debug, Eq, PartialEq)]
pub enum MemoryError {
    Overflow,
    Exhausted { requested: usize, available: usize },
    ReleaseExceedsReservation { requested: usize, reserved: usize },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => write!(formatter, "reservation arithmetic overflow"),
            Self::Exhausted {
                requested,
                available,
            } => write!(
                formatter,
                "memory budget exhausted: requested {requested}, available {available}"
            ),
            Self::ReleaseExceedsReservation {
                requested,
                reserved,
            } => write!(
                formatter,
                "cannot release {requested} bytes from a {reserved}-byte reservation"
            ),
        }
    }
}

impl std::error::Error for MemoryError {}

#[derive(Debug)]
pub enum Admission<T> {
    Admitted { task: T, reservation: Reservation },
    Backpressured { task: T, error: MemoryError },
}

/// Admission 실패 시 task의 ownership을 caller에게 돌려줘 queue/reject를 선택하게 한다.
pub fn admit<T>(pool: &MemoryPool, task: T, estimate: usize) -> Admission<T> {
    match pool.try_reserve(estimate) {
        Ok(reservation) => Admission::Admitted { task, reservation },
        Err(error) => Admission::Backpressured { task, error },
    }
}

#[derive(Debug)]
pub enum BudgetedBufferError {
    Budget(MemoryError),
    Allocation(TryReserveError),
}

impl fmt::Display for BudgetedBufferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(error) => write!(formatter, "memory budget rejected growth: {error}"),
            Self::Allocation(error) => write!(formatter, "buffer allocation failed: {error}"),
        }
    }
}

impl std::error::Error for BudgetedBufferError {}

impl From<MemoryError> for BudgetedBufferError {
    fn from(error: MemoryError) -> Self {
        Self::Budget(error)
    }
}

impl From<TryReserveError> for BudgetedBufferError {
    fn from(error: TryReserveError) -> Self {
        Self::Allocation(error)
    }
}

impl MemoryPool {
    pub fn new(limit: usize) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                limit,
                reserved: AtomicUsize::new(0),
            }),
        }
    }

    pub fn limit(&self) -> usize {
        self.inner.limit
    }

    pub fn reserved(&self) -> usize {
        self.inner.reserved.load(Ordering::Acquire)
    }

    pub fn try_reserve(&self, bytes: usize) -> Result<Reservation, MemoryError> {
        self.inner.try_acquire(bytes)?;
        Ok(Reservation {
            inner: Arc::clone(&self.inner),
            bytes,
        })
    }
}

impl PoolInner {
    fn try_acquire(&self, bytes: usize) -> Result<(), MemoryError> {
        let mut current = self.reserved.load(Ordering::Acquire);

        loop {
            let Some(next) = current.checked_add(bytes) else {
                return Err(MemoryError::Overflow);
            };

            if next > self.limit {
                return Err(MemoryError::Exhausted {
                    requested: bytes,
                    available: self.limit.saturating_sub(current),
                });
            }

            match self.reserved.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(observed) => current = observed,
            }
        }
    }
}

/// Clone을 구현하지 않아 한 grant가 두 번 release되는 것을 막는다.
#[derive(Debug)]
pub struct Reservation {
    inner: Arc<PoolInner>,
    bytes: usize,
}

impl Reservation {
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// 관리 대상 state를 grow하기 전에 grant를 늘린다.
    pub fn try_grow(&mut self, additional: usize) -> Result<(), MemoryError> {
        self.inner.try_acquire(additional)?;
        self.bytes = self
            .bytes
            .checked_add(additional)
            .expect("pool acquisition already checked reservation overflow");
        Ok(())
    }

    /// 더 이상 필요하지 않은 grant 일부를 pool에 즉시 돌려준다.
    pub fn shrink(&mut self, bytes: usize) -> Result<(), MemoryError> {
        if bytes > self.bytes {
            return Err(MemoryError::ReleaseExceedsReservation {
                requested: bytes,
                reserved: self.bytes,
            });
        }

        self.bytes -= bytes;
        let previous = self.inner.reserved.fetch_sub(bytes, Ordering::AcqRel);
        debug_assert!(previous >= bytes, "reservation counter underflow");
        Ok(())
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let previous = self.inner.reserved.fetch_sub(self.bytes, Ordering::AcqRel);
        debug_assert!(previous >= self.bytes, "reservation counter underflow");
    }
}

/// Logical byte growth가 reservation을 앞지르지 않도록 감싼 buffer 예시다.
///
/// 이 type이 보장하는 것은 `len <= reservation.bytes()`다. Allocator가 실제로
/// 확보한 capacity, metadata, fragmentation, RSS까지 같은 값이라는 뜻은 아니다.
#[derive(Debug)]
pub struct BudgetedBuffer {
    bytes: Vec<u8>,
    reservation: Reservation,
}

impl BudgetedBuffer {
    pub fn new(pool: &MemoryPool, initial_grant: usize) -> Result<Self, BudgetedBufferError> {
        // 순서가 중요하다. Application grant를 먼저 얻고 allocation을 시도한다.
        let reservation = pool.try_reserve(initial_grant)?;
        let mut bytes = Vec::new();
        bytes.try_reserve(initial_grant)?;

        Ok(Self { bytes, reservation })
    }

    pub fn try_extend(&mut self, input: &[u8]) -> Result<(), BudgetedBufferError> {
        let next_len = self
            .bytes
            .len()
            .checked_add(input.len())
            .ok_or(MemoryError::Overflow)?;
        let additional_grant = next_len.saturating_sub(self.reservation.bytes());

        // Grow-before-allocate: collection을 늘리기 전에 commitment부터 승인받는다.
        if additional_grant > 0 {
            self.reservation.try_grow(additional_grant)?;
        }

        if let Err(error) = self.bytes.try_reserve(input.len()) {
            // Allocation 자체가 실패하면 방금 얻은 grant를 rollback한다.
            if additional_grant > 0 {
                self.reservation
                    .shrink(additional_grant)
                    .expect("the same call just acquired this grant");
            }
            return Err(BudgetedBufferError::Allocation(error));
        }

        self.bytes.extend_from_slice(input);
        debug_assert!(self.bytes.len() <= self.reservation.bytes());
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.bytes.capacity()
    }

    pub fn granted_bytes(&self) -> usize {
        self.reservation.bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn checked_copy_reserves_and_copies() {
        assert_eq!(copy_checked(b"memory").unwrap(), b"memory");
    }

    #[test]
    fn vec_clear_keeps_capacity() {
        let mut values = Vec::with_capacity(16);
        values.extend(0..8);
        let capacity = values.capacity();

        values.clear();

        assert_eq!(values.len(), 0);
        assert_eq!(values.capacity(), capacity);
    }

    #[test]
    fn reservation_releases_on_drop() {
        let pool = MemoryPool::new(100);
        {
            let grant = pool.try_reserve(60).unwrap();
            assert_eq!(grant.bytes(), 60);
            assert_eq!(pool.reserved(), 60);
            assert_eq!(
                pool.try_reserve(50).unwrap_err(),
                MemoryError::Exhausted {
                    requested: 50,
                    available: 40,
                }
            );
        }
        assert_eq!(pool.reserved(), 0);
    }

    #[test]
    fn admission_returns_task_ownership_when_budget_is_exhausted() {
        let pool = MemoryPool::new(100);
        let active = pool.try_reserve(80).unwrap();

        match admit(&pool, String::from("query-42"), 30) {
            Admission::Backpressured { task, error } => {
                assert_eq!(task, "query-42");
                assert_eq!(
                    error,
                    MemoryError::Exhausted {
                        requested: 30,
                        available: 20,
                    }
                );
            }
            Admission::Admitted { .. } => panic!("query should have been backpressured"),
        }

        drop(active);
        assert!(matches!(
            admit(&pool, "query-42", 30),
            Admission::Admitted { .. }
        ));
    }

    #[test]
    fn reservation_grows_before_work_and_can_return_unused_grant() {
        let pool = MemoryPool::new(100);
        let mut grant = pool.try_reserve(40).unwrap();

        grant.try_grow(50).unwrap();
        assert_eq!(grant.bytes(), 90);
        assert_eq!(pool.reserved(), 90);
        assert_eq!(
            grant.try_grow(20).unwrap_err(),
            MemoryError::Exhausted {
                requested: 20,
                available: 10,
            }
        );

        grant.shrink(30).unwrap();
        assert_eq!(grant.bytes(), 60);
        assert_eq!(pool.reserved(), 60);
        assert_eq!(
            grant.shrink(61).unwrap_err(),
            MemoryError::ReleaseExceedsReservation {
                requested: 61,
                reserved: 60,
            }
        );

        drop(grant);
        assert_eq!(pool.reserved(), 0);
    }

    #[test]
    fn budgeted_buffer_gets_more_grant_before_growing() {
        let pool = MemoryPool::new(100);
        let mut buffer = BudgetedBuffer::new(&pool, 32).unwrap();

        buffer.try_extend(&[1; 20]).unwrap();
        assert_eq!(buffer.len(), 20);
        assert_eq!(buffer.granted_bytes(), 32);
        assert_eq!(pool.reserved(), 32);

        buffer.try_extend(&[2; 50]).unwrap();
        assert_eq!(buffer.len(), 70);
        assert_eq!(buffer.granted_bytes(), 70);
        assert_eq!(pool.reserved(), 70);
        assert_eq!(
            pool.try_reserve(31).unwrap_err(),
            MemoryError::Exhausted {
                requested: 31,
                available: 30,
            }
        );

        // Vec capacity는 allocator 정책 때문에 logical grant보다 클 수 있다.
        assert!(buffer.capacity() >= buffer.len());
        drop(buffer);
        assert_eq!(pool.reserved(), 0);
    }

    #[test]
    fn concurrent_reservations_do_not_exceed_limit() {
        let pool = MemoryPool::new(100);
        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();

        for _ in 0..2 {
            let pool = pool.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                let grant = pool.try_reserve(60);
                barrier.wait();
                grant
            }));
        }

        barrier.wait();
        let grants: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();

        assert_eq!(grants.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(pool.reserved() <= pool.limit());
    }

    #[test]
    fn arithmetic_overflow_is_reported() {
        let pool = MemoryPool::new(usize::MAX);
        let _first = pool.try_reserve(1).unwrap();
        assert!(matches!(
            pool.try_reserve(usize::MAX),
            Err(MemoryError::Overflow)
        ));
    }
}
