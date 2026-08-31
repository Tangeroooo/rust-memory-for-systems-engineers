use memory_lab::MemoryPool;
use memory_lab::tracked_sort::{Row, SortBuffer, SortError, SortPlan};
use std::alloc::System;

// ANCHOR: demo
fn main() -> Result<(), SortError> {
    let pool = MemoryPool::new(192);
    // Stream metadata는 64 B, 즉 4 records를 예상하지만 실제로는 5개가 들어온다.
    let plan = SortPlan::from_wire_bytes(64)?;
    println!(
        "plan: rows={}, initial={} B, reserved={} B",
        plan.initial_rows,
        plan.initial_bytes,
        pool.reserved()
    );
    let mut buffer = SortBuffer::new(&pool, &System, plan)?;
    println!("admitted: reserved={} B", pool.reserved());

    for key in [4, 3, 2, 1] {
        buffer.push(Row {
            key,
            value: key * 10,
        })?;
    }
    println!(
        "4 rows: capacity={}, reserved={} B",
        buffer.capacity(),
        pool.reserved()
    );
    // old 64 + new 128 = 192 B를 먼저 승인. Copy 후 old를 dealloc하여 64 B 반환.
    buffer.push(Row { key: 0, value: 0 })?;
    println!(
        "5 rows: capacity={}, reserved={} B (growth peak: 192 B)",
        buffer.capacity(),
        pool.reserved()
    );

    let output = buffer.finish();
    println!(
        "task finished: rows={}, reserved={} B",
        output.rows().len(),
        pool.reserved()
    );
    assert_eq!(output.rows()[0].key, 0);
    assert_eq!(pool.reserved(), 128); // 결과가 살아 있는 동안 grant도 살아 있다.
    drop(output);
    println!("output dropped: reserved={} B", pool.reserved());
    assert_eq!(pool.reserved(), 0);
    Ok(())
}
// ANCHOR_END: demo
