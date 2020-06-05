use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;

use pi::timer::Timer;
use pi::interrupt::{Controller, Interrupt};

use crate::console::kprintln;
use crate::mutex::Mutex;
use crate::param::{PAGE_SIZE, TICK, USER_IMG_BASE};
use crate::process::{Id, Process, State};
use crate::traps::TrapFrame;
use crate::IRQ;

/// Process scheduler for the entire machine.
#[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Scheduler>>);

impl GlobalScheduler {
    /// Returns an uninitialized wrapper around a local scheduler.
    pub const fn uninitialized() -> GlobalScheduler {
        GlobalScheduler(Mutex::new(None))
    }

    /// Enter a critical region and execute the provided closure with the
    /// internal scheduler.
    pub fn critical<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Scheduler) -> R,
    {
        let mut guard = self.0.lock();
        f(guard.as_mut().expect("scheduler uninitialized"))
    }


    /// Adds a process to the scheduler's queue and returns that process's ID.
    /// For more details, see the documentation on `Scheduler::add()`.
    pub fn add(&self, process: Process) -> Option<Id> {
        self.critical(move |scheduler| scheduler.add(process))
    }

    /// Performs a context switch using `tf` by setting the state of the current
    /// process to `new_state`, saving `tf` into the current process, and
    /// restoring the next process's trap frame into `tf`. For more details, see
    /// the documentation on `Scheduler::schedule_out()` and `Scheduler::switch_to()`.
    pub fn switch(&self, new_state: State, tf: &mut TrapFrame) -> Id {
        self.critical(|scheduler| scheduler.schedule_out(new_state, tf));
        self.switch_to(tf)
    }

    pub fn switch_to(&self, tf: &mut TrapFrame) -> Id {
        loop {
            let rtn = self.critical(|scheduler| scheduler.switch_to(tf));
            if let Some(id) = rtn {
                return id;
            }
            aarch64::wfi();
        }
    }

    /// Kills currently running process and returns that process's ID.
    /// For more details, see the documentaion on `Scheduler::kill()`.
    #[must_use]
    pub fn kill(&self, tf: &mut TrapFrame) -> Option<Id> {
        self.critical(|scheduler| scheduler.kill(tf))
    }

    fn ticc(tf: &mut TrapFrame) {
        Timer::new().tick_in(TICK);
        crate::SCHEDULER.switch(State::Ready, tf);
    }

    /// Starts executing processes in user space using timer interrupt based
    /// preemptive scheduling. This method should not return under normal conditions.
    pub fn start(&self) -> ! {
        let mut tf = Default::default();
        let _pid = crate::SCHEDULER.switch_to(&mut tf);
        // crate::console::kprintln!("Starting PID {}", _pid);
        IRQ.register(Interrupt::Timer1, Box::new(GlobalScheduler::ticc));
        Controller::new().enable(Interrupt::Timer1);
        Timer::new().tick_in(TICK);
        unsafe {
            asm!("mov SP, $0
                  bl context_restore
                  ldp x28, x29, [SP], #16
                  adr lr, _start
                  add lr, lr, $1
                  mov SP, lr
                  mov lr, #0
                  eret"
                :: "r"(&mut tf as *mut TrapFrame as u64),
                   "i"(PAGE_SIZE)
                :: "volatile");
        }
        loop {}
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler
    pub unsafe fn initialize(&self) {
        *self.0.lock() = Some(Scheduler::new());
        for _ in 0..4 {
            let p = Process::load("/fib.bin").expect("could not load process");
            self.add(p);
        }
    }

    // The following method may be useful for testing Phase 3:
    // * A method to load a extern function to the user process's page table.
    pub fn test_phase_3(&self, proc: &mut Process){
        use crate::vm::{VirtualAddr, PagePerm};

        let page = proc.vmap.alloc(
            VirtualAddr::from(USER_IMG_BASE as u64), PagePerm::RWX);

        let text = unsafe {
            core::slice::from_raw_parts(test_user_process as *const u8, 24)
        };

        page[0..24].copy_from_slice(text);
    }
}

#[derive(Debug)]
pub struct Scheduler {
    processes: VecDeque<Process>,
    last_id: Option<Id>,
}

impl Scheduler {
    /// Returns a new `Scheduler` with an empty queue.
    fn new() -> Scheduler {
        Scheduler {
            processes: VecDeque::new(),
            last_id: None,
        }
    }

    /// Adds a process to the scheduler's queue and returns that process's ID if
    /// a new process can be scheduled. The process ID is newly allocated for
    /// the process and saved in its `trap_frame`. If no further processes can
    /// be scheduled, returns `None`.
    ///
    /// It is the caller's responsibility to ensure that the first time `switch`
    /// is called, that process is executing on the CPU.
    fn add(&mut self, mut process: Process) -> Option<Id> {
        let new_pid = if let Some(pid) = self.last_id {
            pid.checked_add(1)
        } else {
            Some(0)
        };
        if let Some(pid) = new_pid {
            process.context.tpidr = pid;
            self.processes.push_back(process);
            self.last_id = new_pid;
            new_pid
        } else {
            None
        }
    }

    /// Finds the currently running process, sets the current process's state
    /// to `new_state`, prepares the context switch on `tf` by saving `tf`
    /// into the current process, and push the current process back to the
    /// end of `processes` queue.
    ///
    /// If the `processes` queue is empty or there is no current process,
    /// returns `false`. Otherwise, returns `true`.
    fn schedule_out(&mut self, new_state: State, tf: &mut TrapFrame) -> bool {
        let mut ind = None;
        for i in 0..self.processes.len() {
            if let Some(p) = self.processes.get_mut(i) {
                if p.context.tpidr == tf.tpidr {
                    if let State::Running = p.state {
                        ind = Some(i);
                        break;
                    }
                }
            }
        }
        if let Some(i) = ind {
            if let Some(mut p) = self.processes.remove(i) {
                let should_requeue = if let State::Dead = new_state {
                    false
                } else {
                    true
                };
                p.state = new_state;
                *p.context = *tf;
                // kprintln!("schedule_out");
                if should_requeue {
                    self.processes.push_back(p);
                }
                return true;
            }
        }
        false
    }

    /// Finds the next process to switch to, brings the next process to the
    /// front of the `processes` queue, changes the next process's state to
    /// `Running`, and performs context switch by restoring the next process`s
    /// trap frame into `tf`.
    ///
    /// If there is no process to switch to, returns `None`. Otherwise, returns
    /// `Some` of the next process`s process ID.
    fn switch_to(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        let mut ind = None;
        for i in 0..self.processes.len() {
            if let Some(p) = self.processes.get_mut(i) {
                if p.is_ready() {
                    ind = Some(i);
                    break;
                }
            }
        }
        if let Some(i) = ind {
            if let Some(mut p) = self.processes.remove(i) {
                let pid = p.context.tpidr;
                p.state = State::Running;
                *tf = *p.context;
                self.processes.push_front(p);
                // kprintln!("switch_to {}", pid);
                return Some(pid);
            }
        }
        None
    }

    /// Kills currently running process by scheduling out the current process
    /// as `Dead` state. Removes the dead process from the queue, drop the
    /// dead process's instance, and returns the dead process's process ID.
    fn kill(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        let mut ind = None;
        for i in 0..self.processes.len() {
            if let Some(p) = self.processes.get_mut(i) {
                if p.context.tpidr == tf.tpidr {
                    if let State::Running = p.state {
                        ind = Some(i);
                        break;
                    }
                }
            }
        }
        if let Some(i) = ind {
            if let Some(mut p) = self.processes.remove(i) {
                let pid = p.context.tpidr;
                p.state = State::Dead;
                drop(p);
                self.switch_to(tf);
                return Some(pid);
            }
        }
        None
    }
}

pub extern "C" fn  test_user_process() -> ! {
    loop {
        let ms = 10000;
        let error: u64;
        let elapsed_ms: u64;
        unsafe {
            asm!("mov x0, $2
              svc 1
              mov $0, x0
              mov $1, x7"
                 : "=r"(elapsed_ms), "=r"(error)
                 : "r"(ms)
                 : "x0", "x7"
                 : "volatile");
        }
    }
}

