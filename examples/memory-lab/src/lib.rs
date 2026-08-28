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
        }
    }
}

impl std::error::Error for MemoryError {}

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
        let mut current = self.inner.reserved.load(Ordering::Acquire);

        loop {
            let Some(next) = current.checked_add(bytes) else {
                return Err(MemoryError::Overflow);
            };

            if next > self.inner.limit {
                return Err(MemoryError::Exhausted {
                    requested: bytes,
                    available: self.inner.limit.saturating_sub(current),
                });
            }

            match self.inner.reserved.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(Reservation {
                        inner: Arc::clone(&self.inner),
                        bytes,
                    });
                }
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
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let previous = self.inner.reserved.fetch_sub(self.bytes, Ordering::AcqRel);
        debug_assert!(previous >= self.bytes, "reservation counter underflow");
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
