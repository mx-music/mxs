//! Worker threads that fill voices beside the audio thread, each voice into a buffer of its own, so the mixer adds them in the order of the voices whichever thread filled them.
//! The audio thread hands out a block's voices by a counter it shares with the workers, takes voices itself, and waits only for those a worker has started, so a worker the system runs late slows nothing.

use fundsp::prelude::*;
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::{JoinHandle, Thread};
use std::time::{Duration, Instant};

/// The most voices of a block the workers fill; any more are filled on the audio thread as before.
pub(crate) const SLOTS: usize = 1024;
/// The fewest voices in a block worth handing to the workers; below it the audio thread fills them alone.
pub(crate) const FEWEST: usize = 8;
/// How long a worker keeps looking for work after a block before it sleeps until woken.
const SPIN: Duration = Duration::from_micros(200);

/// A unit to fill: where it is and how many frames.
#[derive(Clone, Copy)]
struct Task {
    unit: Option<NonNull<dyn AudioUnit>>,
    frames: usize,
}

struct Shared {
    /// The block's generation, how many voices it hands out and the next to hand out, in one word so a worker never takes a voice of another block.
    ticket: AtomicU64,
    done: AtomicUsize,
    stop: AtomicBool,
    tasks: Box<[UnsafeCell<Task>]>,
    outputs: Box<[UnsafeCell<BufferVec>]>,
}

// The audio thread writes a task and its buffer only while no worker can take it, and a worker touches only the task it took.
unsafe impl Sync for Shared {}
unsafe impl Send for Shared {}

fn ticket(generation: u64, count: usize, next: usize) -> u64 {
    (generation << 32) | ((count as u64) << 16) | next as u64
}

fn parts(ticket: u64) -> (u64, usize, usize) {
    (
        ticket >> 32,
        ((ticket >> 16) & 0xffff) as usize,
        (ticket & 0xffff) as usize,
    )
}

impl Shared {
    /// Takes and fills voices of the generation `generation` until none is left; false once none was left to take.
    fn work(&self, generation: u64) -> bool {
        let mut took = false;
        loop {
            let now = self.ticket.load(Ordering::Acquire);
            let (current, count, next) = parts(now);
            if current != generation || next >= count {
                return took;
            }
            if self
                .ticket
                .compare_exchange_weak(now, now + 1, Ordering::AcqRel, Ordering::Relaxed)
                .is_err()
            {
                continue;
            }
            took = true;
            // Safety: the counter handed this task to this thread alone, and the audio thread waits for it before touching it again.
            unsafe {
                let task = *self.tasks[next].get();
                let output = &mut *self.outputs[next].get();
                let mut unit = task.unit.expect("a task handed out has a unit");
                unit.as_mut()
                    .process(task.frames, &BufferRef::empty(), &mut output.buffer_mut());
            }
            self.done.fetch_add(1, Ordering::Release);
        }
    }
}

/// Threads that fill voices beside the audio thread; with one worker, the audio thread alone, there are none.
pub(crate) struct Pool {
    shared: Arc<Shared>,
    threads: Vec<(Thread, JoinHandle<()>)>,
    generation: u64,
    count: usize,
}

impl Pool {
    /// A pool of `workers`, counting the audio thread, so `workers - 1` threads of its own.
    pub fn new(workers: usize) -> Pool {
        let shared = Arc::new(Shared {
            ticket: AtomicU64::new(0),
            done: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
            tasks: (0..SLOTS)
                .map(|_| {
                    UnsafeCell::new(Task {
                        unit: None,
                        frames: 0,
                    })
                })
                .collect(),
            outputs: (0..SLOTS)
                .map(|_| UnsafeCell::new(BufferVec::new(2)))
                .collect(),
        });
        let threads = (1..Ord::max(workers, 1))
            .map(|number| {
                let shared = shared.clone();
                let handle = std::thread::Builder::new()
                    .name(format!("mxs worker {number}"))
                    .spawn(move || run(&shared))
                    .expect("a worker thread starts");
                (handle.thread().clone(), handle)
            })
            .collect();
        Pool {
            shared,
            threads,
            generation: 0,
            count: 0,
        }
    }

    /// How many workers fill voices, counting the audio thread.
    pub fn workers(&self) -> usize {
        self.threads.len() + 1
    }

    /// Starts handing out a block; voices are then added with [`Pool::add`] before [`Pool::fill`].
    pub fn begin(&mut self) {
        self.count = 0;
    }

    /// Adds a unit to fill for `frames`, and gives the slot its sound will be in, or none when every slot is taken.
    pub fn add(&mut self, unit: &mut (dyn AudioUnit + 'static), frames: usize) -> Option<usize> {
        if self.count == SLOTS {
            return None;
        }
        let slot = self.count;
        // Safety: no generation is handing out tasks between blocks, so no worker reads this one.
        unsafe {
            *self.shared.tasks[slot].get() = Task {
                unit: Some(NonNull::from(unit)),
                frames,
            };
        }
        self.count += 1;
        Some(slot)
    }

    /// Fills every voice added since [`Pool::begin`], on the workers and the audio thread, and returns once all are filled.
    pub fn fill(&mut self) {
        if self.count == 0 {
            return;
        }
        self.generation = (self.generation + 1) & 0xffff_ffff;
        self.shared.done.store(0, Ordering::Relaxed);
        self.shared
            .ticket
            .store(ticket(self.generation, self.count, 0), Ordering::Release);
        for (thread, _) in &self.threads {
            thread.unpark();
        }
        self.shared.work(self.generation);
        while self.shared.done.load(Ordering::Acquire) < self.count {
            std::hint::spin_loop();
        }
    }

    /// The sound of the voice filled in `slot` by the last [`Pool::fill`].
    pub fn output(&self, slot: usize) -> &BufferVec {
        // Safety: every task of the last generation is done, and no worker writes until the next.
        unsafe { &*self.shared.outputs[slot].get() }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        for (thread, handle) in self.threads.drain(..) {
            thread.unpark();
            let _ = handle.join();
        }
    }
}

fn run(shared: &Shared) {
    raise_priority();
    let mut seen = 0;
    let mut idle = Instant::now();
    while !shared.stop.load(Ordering::Acquire) {
        let (generation, _, _) = parts(shared.ticket.load(Ordering::Acquire));
        if generation != seen {
            shared.work(generation);
            seen = generation;
            idle = Instant::now();
        } else if idle.elapsed() < SPIN {
            std::hint::spin_loop();
        } else {
            std::thread::park();
        }
    }
}

/// How many performance cores the machine has, which is how many workers an engine starts with.
pub(crate) fn performance_cores() -> usize {
    static CORES: OnceLock<usize> = OnceLock::new();
    *CORES.get_or_init(|| {
        let all = std::thread::available_parallelism().map_or(1, |n| n.get());
        #[cfg(target_os = "macos")]
        if let Some(cores) = perflevel0_cores() {
            return Ord::clamp(cores, 1, all);
        }
        all
    })
}

/// The performance cores macOS counts, asked of the kernel rather than of a `sysctl` process, whose wait was seen to hang inside the BEAM.
#[cfg(target_os = "macos")]
fn perflevel0_cores() -> Option<usize> {
    unsafe extern "C" {
        fn sysctlbyname(
            name: *const std::ffi::c_char,
            old: *mut std::ffi::c_void,
            old_length: *mut usize,
            new: *mut std::ffi::c_void,
            new_length: usize,
        ) -> i32;
    }
    let mut cores: i32 = 0;
    let mut length = std::mem::size_of::<i32>();
    // Safety: the kernel writes at most `length` bytes into `cores`, an i32 as it gives this name.
    let read = unsafe {
        sysctlbyname(
            c"hw.perflevel0.physicalcpu".as_ptr(),
            (&mut cores as *mut i32).cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    (read == 0 && cores > 0).then_some(cores as usize)
}

/// Asks macOS to run the calling thread as the audio thread is run, with a time constraint of a few milliseconds in each period.
#[cfg(target_os = "macos")]
fn raise_priority() {
    #[repr(C)]
    struct Constraint {
        period: u32,
        computation: u32,
        constraint: u32,
        preemptible: i32,
    }
    #[repr(C)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }
    unsafe extern "C" {
        fn mach_thread_self() -> u32;
        fn mach_timebase_info(info: *mut Timebase) -> i32;
        fn thread_policy_set(
            thread: u32,
            flavor: u32,
            policy: *const Constraint,
            count: u32,
        ) -> i32;
    }
    const TIME_CONSTRAINT_POLICY: u32 = 2;
    let mut timebase = Timebase { numer: 0, denom: 0 };
    // Safety: plain calls into the kernel with structs laid out as it reads them.
    unsafe {
        if mach_timebase_info(&mut timebase) != 0 || timebase.numer == 0 {
            return;
        }
        let ticks =
            |nanoseconds: f64| (nanoseconds * timebase.denom as f64 / timebase.numer as f64) as u32;
        let policy = Constraint {
            period: ticks(2_666_667.0),
            computation: ticks(1_000_000.0),
            constraint: ticks(2_666_667.0),
            preemptible: 1,
        };
        thread_policy_set(mach_thread_self(), TIME_CONSTRAINT_POLICY, &policy, 4);
    }
}

#[cfg(not(target_os = "macos"))]
fn raise_priority() {}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn the_kernel_counts_the_performance_cores_sysctl_prints() {
        let printed = std::process::Command::new("sysctl")
            .args(["-n", "hw.perflevel0.physicalcpu"])
            .output()
            .unwrap();
        let printed: usize = String::from_utf8(printed.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(perflevel0_cores(), Some(printed));
    }
}
