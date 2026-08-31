//! 고정 폭 record 정렬: requested Layout byte를 allocation 전에 승인한다.
//!
//! 이 예제의 budget은 row storage만 포함한다. Pool/allocator의 metadata, 입력,
//! stack, 출력 전송, allocator overhead 및 RSS는 별도 resource domain이다.
//! 범용 Vec 대체물이 아니다. Row는 Copy이며 nested allocation이 없다.

use crate::{MemoryError, MemoryPool, Reservation};
use std::alloc::{GlobalAlloc, Layout};
use std::{fmt, ptr::NonNull};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Row {
    pub key: u64,
    pub value: u64,
}

pub const WIRE_ROW_BYTES: usize = 16;
const _: () = assert!(size_of::<Row>() == 16);

#[derive(Debug, Eq, PartialEq)]
pub enum SortError {
    InvalidWireLength,
    LayoutOverflow,
    Budget(MemoryError),
    Allocation { bytes: usize },
    Cancelled,
}

impl fmt::Display for SortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sort failed: {self:?}")
    }
}

impl std::error::Error for SortError {}

impl From<MemoryError> for SortError {
    fn from(error: MemoryError) -> Self {
        Self::Budget(error)
    }
}

fn row_layout(capacity: usize) -> Result<Layout, SortError> {
    Layout::array::<Row>(capacity).map_err(|_| SortError::LayoutOverflow)
}

// ANCHOR: plan
#[derive(Clone, Copy, Debug)]
pub struct SortPlan {
    pub initial_rows: usize,
    pub initial_bytes: usize,
}

impl SortPlan {
    /// Wire format: key(u64 LE) + value(u64 LE), 1 record = 16 byte.
    /// 입력 전체 길이가 확정되면 정확한 수다. Stream metadata라면 estimate다.
    pub fn from_wire_bytes(estimated_wire_bytes: usize) -> Result<Self, SortError> {
        if !estimated_wire_bytes.is_multiple_of(WIRE_ROW_BYTES) {
            return Err(SortError::InvalidWireLength);
        }
        let initial_rows = estimated_wire_bytes / WIRE_ROW_BYTES;
        let initial_bytes = row_layout(initial_rows)?.size();
        Ok(Self {
            initial_rows,
            initial_bytes,
        })
    }
}
// ANCHOR_END: plan

/// 한 allocation의 유일한 owner다. Allocator는 block보다 오래 살아야 한다.
struct Block<'a, A: GlobalAlloc> {
    ptr: NonNull<Row>,
    layout: Layout,
    capacity: usize,
    allocator: &'a A,
    // Drop body가 dealloc한 뒤 field destructor가 grant를 반환한다.
    _reservation: Reservation,
}

impl<'a, A: GlobalAlloc> Block<'a, A> {
    // ANCHOR: allocate
    fn try_new(pool: &MemoryPool, allocator: &'a A, capacity: usize) -> Result<Self, SortError> {
        let layout = row_layout(capacity)?;
        // 이 함수는 non-empty block만 만든다. Empty buffer에는 Block이 없다.
        assert!(layout.size() > 0);
        let reservation = pool.try_reserve(layout.size())?;
        // SAFETY: Layout은 유효하고 크기는 0이 아니다. 아직 값은 읽지 않는다.
        let raw = unsafe { allocator.alloc(layout) };
        let ptr = NonNull::new(raw.cast::<Row>()).ok_or(SortError::Allocation {
            bytes: layout.size(),
        })?;
        // null이면 위의 ?가 reservation을 Drop하여 새 grant만 rollback한다.
        Ok(Self {
            ptr,
            layout,
            capacity,
            allocator,
            _reservation: reservation,
        })
    }
    // ANCHOR_END: allocate
}

// ANCHOR: release
impl<A: GlobalAlloc> Drop for Block<'_, A> {
    fn drop(&mut self) {
        // SAFETY: 같은 allocator에서 받은 live pointer와 원래 Layout이다.
        // Row는 Copy여서 element destructor가 필요 없다. Block은 복제되지 않는다.
        unsafe {
            self.allocator
                .dealloc(self.ptr.as_ptr().cast(), self.layout)
        };
        // 이 body가 끝난 다음 _reservation이 Drop된다. 순서를 뒤집지 않는다.
    }
}
// ANCHOR_END: release

/// Buffer와 grant를 분리해 가져가는 API는 제공하지 않는다.
pub struct SortBuffer<'a, A: GlobalAlloc> {
    pool: &'a MemoryPool,
    allocator: &'a A,
    block: Option<Block<'a, A>>,
    len: usize,
}

impl<'a, A: GlobalAlloc> SortBuffer<'a, A> {
    pub fn new(pool: &'a MemoryPool, allocator: &'a A, plan: SortPlan) -> Result<Self, SortError> {
        // Public plan의 byte 값을 신뢰하지 않고 Layout에서 다시 계산한다.
        let block = if plan.initial_rows == 0 {
            None
        } else {
            Some(Block::try_new(pool, allocator, plan.initial_rows)?)
        };
        Ok(Self {
            pool,
            allocator,
            block,
            len: 0,
        })
    }

    pub fn capacity(&self) -> usize {
        self.block.as_ref().map_or(0, |block| block.capacity)
    }

    pub fn charged_bytes(&self) -> usize {
        self.block.as_ref().map_or(0, |block| block.layout.size())
    }

    pub fn rows(&self) -> &[Row] {
        match &self.block {
            None => &[],
            // SAFETY: 0..len만 초기화되어 있다. len <= capacity이며 borrow 중 해제하지 않는다.
            Some(block) => unsafe { std::slice::from_raw_parts(block.ptr.as_ptr(), self.len) },
        }
    }

    // ANCHOR: grow
    pub fn push(&mut self, row: Row) -> Result<(), SortError> {
        if self.len == self.capacity() {
            let next_capacity = if self.capacity() == 0 {
                1
            } else {
                self.capacity()
                    .checked_mul(2)
                    .ok_or(SortError::LayoutOverflow)?
            };
            // old는 아직 살아 있다. 차액이 아니라 new 전체를 추가 예약한다.
            let next = Block::try_new(self.pool, self.allocator, next_capacity)?;
            if let Some(old) = &self.block {
                // SAFETY: 서로 다른 live allocation이다. 초기화된 Row만 복사한다.
                // next capacity >= len, Row는 Copy, source/destination은 겹치지 않는다.
                unsafe {
                    std::ptr::copy_nonoverlapping(old.ptr.as_ptr(), next.ptr.as_ptr(), self.len);
                }
            }
            // new를 owner에 연결한 다음 old를 dealloc하고 old의 grant를 반환한다.
            drop(self.block.replace(next));
        }
        let block = self
            .block
            .as_mut()
            .expect("push has allocated a nonempty block");
        // SAFETY: len < capacity다. 미초기화 slot에 write한 뒤 len을 늘린다.
        unsafe { block.ptr.as_ptr().add(self.len).write(row) };
        self.len += 1;
        Ok(())
    }
    // ANCHOR_END: grow

    /// Logical contents만 버린다. Storage와 grant는 유지한다.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Storage를 버릴 때에만 해당 grant도 반환한다.
    pub fn release_storage(&mut self) {
        self.len = 0;
        drop(self.block.take());
    }

    pub fn finish(mut self) -> SortedRows<'a, A> {
        if let Some(block) = &mut self.block {
            // SAFETY: 초기화된 range에 대한 유일한 mutable borrow다.
            let rows = unsafe { std::slice::from_raw_parts_mut(block.ptr.as_ptr(), self.len) };
            // In-place 정렬이며 추가 heap allocation이 없다. Key도 u64다.
            rows.sort_unstable_by_key(|row| row.key);
        }
        // Task 완료 시 grant를 반환하지 않고 output으로 storage와 함께 move한다.
        SortedRows { buffer: self }
    }
}

pub struct SortedRows<'a, A: GlobalAlloc> {
    buffer: SortBuffer<'a, A>,
}

impl<A: GlobalAlloc> SortedRows<'_, A> {
    pub fn rows(&self) -> &[Row] {
        self.buffer.rows()
    }
}

// ANCHOR: task
pub fn sort_records<'a, A: GlobalAlloc>(
    pool: &'a MemoryPool,
    allocator: &'a A,
    plan: SortPlan,
    input: impl IntoIterator<Item = Row>,
    mut should_cancel: impl FnMut() -> bool,
) -> Result<SortedRows<'a, A>, SortError> {
    let mut buffer = SortBuffer::new(pool, allocator, plan)?;
    for row in input {
        if should_cancel() {
            return Err(SortError::Cancelled); // buffer Drop → dealloc → release
        }
        buffer.push(row)?; // error에서도 동일하게 cleanup
    }
    Ok(buffer.finish()) // storage + reservation을 output으로 move
}
// ANCHOR_END: task

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::System;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Eq, PartialEq)]
    enum Event {
        Alloc { bytes: usize, reserved: usize },
        Free { bytes: usize, reserved: usize },
    }

    // Test 전용 fault injection이다. 실제 OOM이나 RSS 값에 의존하지 않는다.
    struct Probe<'a> {
        pool: &'a MemoryPool,
        events: RefCell<Vec<Event>>,
        fail_next: Cell<bool>,
    }

    impl<'a> Probe<'a> {
        fn new(pool: &'a MemoryPool) -> Self {
            Self {
                pool,
                events: RefCell::new(Vec::new()),
                fail_next: Cell::new(false),
            }
        }
    }

    // SAFETY: 유효한 요청을 같은 Layout으로 System에 위임하거나 null로 거절한다.
    // Global allocator로 등록하지 않는다. Test log의 allocation은 감사 대상 밖이다.
    unsafe impl GlobalAlloc for Probe<'_> {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            self.events.borrow_mut().push(Event::Alloc {
                bytes: layout.size(),
                reserved: self.pool.reserved(),
            });
            if self.fail_next.replace(false) {
                std::ptr::null_mut()
            } else {
                // SAFETY: Caller의 GlobalAlloc contract를 그대로 전달한다.
                unsafe { System.alloc(layout) }
            }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            self.events.borrow_mut().push(Event::Free {
                bytes: layout.size(),
                reserved: self.pool.reserved(),
            });
            // SAFETY: 이 Probe에서 성공한 allocation은 System에서 받았다.
            unsafe { System.dealloc(ptr, layout) };
        }
    }

    fn plan(rows: usize) -> SortPlan {
        SortPlan::from_wire_bytes(rows * WIRE_ROW_BYTES).unwrap()
    }

    fn row(key: u64) -> Row {
        Row {
            key,
            value: key * 10,
        }
    }

    #[test]
    fn plan_uses_wire_count_and_checked_layout() {
        let plan = SortPlan::from_wire_bytes(64).unwrap();
        assert_eq!((plan.initial_rows, plan.initial_bytes), (4, 64));
        assert!(matches!(
            SortPlan::from_wire_bytes(63),
            Err(SortError::InvalidWireLength)
        ));
        assert!(matches!(
            row_layout(usize::MAX),
            Err(SortError::LayoutOverflow)
        ));
    }

    #[test]
    fn growth_charges_overlap_and_output_keeps_grant() {
        let pool = MemoryPool::new(192);
        let allocator = Probe::new(&pool);
        let mut buffer = SortBuffer::new(&pool, &allocator, plan(4)).unwrap();
        assert_eq!(pool.reserved(), 64);
        for key in (1..=4).rev() {
            buffer.push(row(key)).unwrap();
        }
        assert_eq!(pool.reserved(), 64);
        buffer.push(row(0)).unwrap();
        assert_eq!((buffer.capacity(), buffer.charged_bytes()), (8, 128));
        assert_eq!(pool.reserved(), 128);
        let output = buffer.finish();
        assert_eq!(output.rows()[0], row(0));
        assert_eq!(pool.reserved(), 128); // task 종료만으로 반환하지 않는다
        drop(output);
        assert_eq!(pool.reserved(), 0);
        assert_eq!(
            *allocator.events.borrow(),
            [
                Event::Alloc {
                    bytes: 64,
                    reserved: 64
                },
                Event::Alloc {
                    bytes: 128,
                    reserved: 192
                },
                Event::Free {
                    bytes: 64,
                    reserved: 192
                },
                Event::Free {
                    bytes: 128,
                    reserved: 128
                },
            ]
        );
    }

    #[test]
    fn steady_size_fits_but_overlap_does_not() {
        let pool = MemoryPool::new(160);
        let allocator = Probe::new(&pool);
        let mut buffer = SortBuffer::new(&pool, &allocator, plan(4)).unwrap();
        for key in 0..4 {
            buffer.push(row(key)).unwrap();
        }
        assert_eq!(
            buffer.push(row(4)),
            Err(SortError::Budget(MemoryError::Exhausted {
                requested: 128,
                available: 96,
            }))
        );
        assert_eq!(buffer.rows(), &[row(0), row(1), row(2), row(3)]);
        assert_eq!(allocator.events.borrow().len(), 1); // allocator에 도달하지 않는다
        assert_eq!(pool.reserved(), 64);
    }

    #[test]
    fn failed_allocation_rolls_back_only_new_grant_then_retry_succeeds() {
        let pool = MemoryPool::new(192);
        let allocator = Probe::new(&pool);
        let mut buffer = SortBuffer::new(&pool, &allocator, plan(4)).unwrap();
        for key in 0..4 {
            buffer.push(row(key)).unwrap();
        }
        allocator.fail_next.set(true);
        assert_eq!(
            buffer.push(row(4)),
            Err(SortError::Allocation { bytes: 128 })
        );
        assert_eq!(
            (buffer.rows().len(), buffer.capacity(), pool.reserved()),
            (4, 4, 64)
        );
        buffer.push(row(4)).unwrap();
        assert_eq!((buffer.rows().len(), pool.reserved()), (5, 128));
        drop(buffer);
        assert_eq!(pool.reserved(), 0);
    }

    #[test]
    fn initial_denial_and_initial_null_leave_no_grant() {
        let pool = MemoryPool::new(64);
        let allocator = Probe::new(&pool);
        assert!(matches!(
            SortBuffer::new(&pool, &allocator, plan(8)),
            Err(SortError::Budget(_))
        ));
        assert!(allocator.events.borrow().is_empty());
        allocator.fail_next.set(true);
        assert!(matches!(
            SortBuffer::new(&pool, &allocator, plan(4)),
            Err(SortError::Allocation { bytes: 64 })
        ));
        assert_eq!(pool.reserved(), 0);
    }

    #[test]
    fn clear_keeps_storage_release_storage_returns_it() {
        let pool = MemoryPool::new(64);
        let mut buffer = SortBuffer::new(&pool, &System, plan(4)).unwrap();
        buffer.push(row(1)).unwrap();
        buffer.clear();
        assert_eq!(
            (buffer.rows().len(), buffer.capacity(), pool.reserved()),
            (0, 4, 64)
        );
        buffer.release_storage();
        assert_eq!((buffer.capacity(), pool.reserved()), (0, 0));
        buffer.push(row(2)).unwrap();
        assert_eq!(pool.reserved(), 16);
    }

    #[test]
    fn empty_plan_never_calls_allocator_with_zero_layout() {
        let pool = MemoryPool::new(0);
        let allocator = Probe::new(&pool);
        let output = SortBuffer::new(&pool, &allocator, plan(0))
            .unwrap()
            .finish();
        assert!(output.rows().is_empty());
        assert!(allocator.events.borrow().is_empty());
        assert_eq!(pool.reserved(), 0);
    }

    #[test]
    fn task_error_and_cooperative_cancellation_release_storage() {
        let pool = MemoryPool::new(160);
        assert!(matches!(
            sort_records(&pool, &System, plan(4), (0..5).map(row), || false),
            Err(SortError::Budget(_))
        ));
        assert_eq!(pool.reserved(), 0);
        let mut seen = 0;
        assert!(matches!(
            sort_records(&pool, &System, plan(4), (0..5).map(row), || {
                seen += 1;
                seen == 3
            }),
            Err(SortError::Cancelled)
        ));
        assert_eq!(pool.reserved(), 0);
    }

    #[test]
    fn repeated_growth_preserves_every_row_and_matches_reference_sort() {
        let pool = MemoryPool::new(2 * 1024 * 1024);
        for initial_rows in [0, 1, 3, 4, 16] {
            // Reference data는 test harness 소유로, row storage budget 밖이다.
            let mut expected: Vec<_> = (0..1025).map(|i| row((i * 37) % 257)).collect();
            let output = sort_records(
                &pool,
                &System,
                plan(initial_rows),
                expected.iter().copied(),
                || false,
            )
            .unwrap();
            expected.sort_unstable_by_key(|row| row.key);
            assert_eq!(output.rows(), expected);
            assert!(pool.reserved() >= std::mem::size_of_val(output.rows()));
            assert!(pool.reserved() <= pool.limit());
            drop(output);
            assert_eq!(pool.reserved(), 0);
        }
    }

    #[test]
    fn unwind_drops_storage_but_is_not_an_oom_recovery_mechanism() {
        let pool = MemoryPool::new(64);
        let result = std::panic::catch_unwind(|| {
            let _buffer = SortBuffer::new(&pool, &System, plan(4)).unwrap();
            panic!("ordinary task panic, not allocator OOM");
        });
        assert!(result.is_err());
        assert_eq!(pool.reserved(), 0);
    }
}
