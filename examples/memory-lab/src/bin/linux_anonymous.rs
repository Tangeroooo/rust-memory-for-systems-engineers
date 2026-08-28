//! Linux에서 allocator requested byte와 anonymous RSS가 달라지는 원인을 관찰한다.
//!
//! 실행: `cargo run -p memory-lab --bin linux_anonymous`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingAllocator;

static LIVE_REQUESTED_BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: allocation과 deallocation은 System에 그대로 위임하며, counter는
// 관측용 requested byte만 기록한다. Allocator 내부에서는 unwind하지 않는다.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: caller가 전달한 유효한 Layout을 System에 그대로 전달한다.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            LIVE_REQUESTED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE_REQUESTED_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: pointer와 Layout은 이 allocator의 성공한 allocation에서 왔다.
        unsafe { System.dealloc(pointer, layout) };
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[cfg(any(target_os = "linux", test))]
fn status_field_kib(status: &str, field: &str) -> Option<usize> {
    status.lines().find_map(|line| {
        let rest = line.strip_prefix(field)?.trim();
        rest.split_whitespace().next()?.parse().ok()
    })
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{LIVE_REQUESTED_BYTES, Ordering, status_field_kib};
    use std::ffi::c_void;
    use std::fs;
    use std::io;
    use std::ptr;
    use std::thread;

    const MIB: usize = 1024 * 1024;
    const PROT_READ: i32 = 0x1;
    const PROT_WRITE: i32 = 0x2;
    const MAP_PRIVATE: i32 = 0x2;
    const MAP_ANONYMOUS: i32 = 0x20;
    const SC_PAGESIZE: i32 = 30;

    unsafe extern "C" {
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            file_descriptor: i32,
            offset: isize,
        ) -> *mut c_void;
        fn munmap(address: *mut c_void, length: usize) -> i32;
        fn sysconf(name: i32) -> isize;
    }

    #[derive(Debug)]
    struct Snapshot {
        vm_rss_kib: usize,
        rss_anon_kib: usize,
        allocator_live_requested: usize,
    }

    fn snapshot() -> io::Result<Snapshot> {
        let (vm_rss_kib, rss_anon_kib) = {
            let status = fs::read_to_string("/proc/self/status")?;
            let vm_rss_kib = status_field_kib(&status, "VmRSS:").unwrap_or(0);
            let rss_anon_kib = status_field_kib(&status, "RssAnon:").unwrap_or(0);
            (vm_rss_kib, rss_anon_kib)
        };

        Ok(Snapshot {
            vm_rss_kib,
            rss_anon_kib,
            allocator_live_requested: LIVE_REQUESTED_BYTES.load(Ordering::Relaxed),
        })
    }

    fn print_snapshot(label: &str) -> io::Result<()> {
        let value = snapshot()?;
        println!(
            "{label:>22} | allocator_live={:>10} B | RssAnon={:>8} KiB | VmRSS={:>8} KiB",
            value.allocator_live_requested, value.rss_anon_kib, value.vm_rss_kib
        );
        Ok(())
    }

    fn page_size() -> io::Result<usize> {
        // SAFETY: sysconf는 process-global 설정을 읽으며 SC_PAGESIZE는 Linux 상수다.
        let value = unsafe { sysconf(SC_PAGESIZE) };
        usize::try_from(value).map_err(|_| io::Error::last_os_error())
    }

    fn touch_heap(bytes: &mut [u8]) -> io::Result<()> {
        let page_size = page_size()?;
        for offset in (0..bytes.len()).step_by(page_size) {
            // volatile write로 page가 실제로 touch되도록 한다.
            unsafe { bytes.as_mut_ptr().add(offset).write_volatile(0xa5) };
        }
        Ok(())
    }

    struct AnonymousMapping {
        pointer: *mut u8,
        length: usize,
    }

    impl AnonymousMapping {
        fn new(length: usize) -> io::Result<Self> {
            // SAFETY: anonymous private mapping에는 null address, fd -1, offset 0을 사용한다.
            let pointer = unsafe {
                mmap(
                    ptr::null_mut(),
                    length,
                    PROT_READ | PROT_WRITE,
                    MAP_PRIVATE | MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            if pointer == usize::MAX as *mut c_void {
                return Err(io::Error::last_os_error());
            }

            Ok(Self {
                pointer: pointer.cast(),
                length,
            })
        }

        fn touch(&mut self) -> io::Result<()> {
            let page_size = page_size()?;
            for offset in (0..self.length).step_by(page_size) {
                // SAFETY: offset은 mapping length 안이며 mapping은 write 가능하다.
                unsafe { self.pointer.add(offset).write_volatile(0x5a) };
            }
            Ok(())
        }
    }

    impl Drop for AnonymousMapping {
        fn drop(&mut self) {
            // SAFETY: pointer와 length는 성공한 mmap 결과이며 한 번만 해제한다.
            let result = unsafe { munmap(self.pointer.cast(), self.length) };
            debug_assert_eq!(result, 0);
        }
    }

    #[inline(never)]
    fn touch_stack(depth: usize, on_peak: &impl Fn()) {
        let mut page = [0_u8; 4096];
        // volatile access와 black_box로 각 recursion frame의 page를 유지한다.
        unsafe { page.as_mut_ptr().write_volatile(depth as u8) };
        std::hint::black_box(&mut page);

        if depth == 0 {
            on_peak();
        } else {
            touch_stack(depth - 1, on_peak);
        }

        std::hint::black_box(page[0]);
    }

    pub fn run() -> io::Result<()> {
        println!("requested byte counter와 Linux resident memory는 같은 ledger가 아니다.");
        print_snapshot("baseline")?;

        let mut heap = vec![0_u8; 8 * MIB];
        touch_heap(&mut heap)?;
        print_snapshot("Vec heap touched")?;

        let before_mmap = LIVE_REQUESTED_BYTES.load(Ordering::Relaxed);
        let mut direct_mapping = AnonymousMapping::new(8 * MIB)?;
        direct_mapping.touch()?;
        let after_mmap = LIVE_REQUESTED_BYTES.load(Ordering::Relaxed);
        print_snapshot("direct mmap touched")?;
        println!(
            "direct mmap allocator counter delta: {} B",
            after_mmap as isize - before_mmap as isize
        );

        let worker = thread::Builder::new()
            .name("anonymous-stack-demo".into())
            .stack_size(4 * MIB)
            .spawn(|| {
                touch_stack(256, &|| {
                    print_snapshot("thread stack peak")
                        .expect("/proc/self/status should remain readable");
                });
            })?;
        worker.join().expect("demo thread should not panic");

        drop(direct_mapping);
        drop(heap);
        print_snapshot("after Drop/munmap")?;

        println!("값은 kernel/allocator/version에 따라 달라진다. 방향과 ledger 차이만 관찰한다.");
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn main() -> std::io::Result<()> {
    linux::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("이 실험은 Linux의 /proc와 MAP_ANONYMOUS를 사용한다.");
}

#[cfg(test)]
mod tests {
    use super::status_field_kib;

    #[test]
    fn parses_proc_status_memory_fields() {
        let status = "VmRSS:\t  1200 kB\nRssAnon:\t  800 kB\n";
        assert_eq!(status_field_kib(status, "VmRSS:"), Some(1200));
        assert_eq!(status_field_kib(status, "RssAnon:"), Some(800));
    }
}
