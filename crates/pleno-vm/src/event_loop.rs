use pleno_value::Value;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Timer {
    pub id: u64,
    pub deadline: Instant,
    pub callback: Value,
    pub args: Vec<Value>,
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.id == other.id
    }
}

impl Eq for Timer {}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse order so earliest deadline is at top of heap
        other
            .deadline
            .cmp(&self.deadline)
            .then_with(|| other.id.cmp(&self.id))
    }
}

pub struct EventLoop {
    pub timers: std::collections::BinaryHeap<Timer>,
    pub ready_queue: VecDeque<Task>,
    pub next_timer_id: u64,
}

#[derive(Debug)]
pub struct Task {
    pub callback: Value,
    pub args: Vec<Value>,
    pub promise_id: Option<u64>,
}

impl EventLoop {
    pub fn new() -> Self {
        Self {
            timers: std::collections::BinaryHeap::new(),
            ready_queue: VecDeque::new(),
            next_timer_id: 0,
        }
    }

    pub fn add_timer(&mut self, delay_ms: u64, callback: Value, args: Vec<Value>) -> u64 {
        let id = self.next_timer_id;
        self.next_timer_id += 1;
        let deadline = Instant::now() + Duration::from_millis(delay_ms);
        self.timers.push(Timer {
            id,
            deadline,
            callback,
            args,
        });
        id
    }

    pub fn add_task(&mut self, callback: Value, args: Vec<Value>, promise_id: Option<u64>) {
        self.ready_queue.push_back(Task {
            callback,
            args,
            promise_id,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.timers.is_empty() && self.ready_queue.is_empty()
    }

    pub fn tick(&mut self) -> Option<Task> {
        // println!("EventLoop::tick: timers={}, ready={}", self.timers.len(), self.ready_queue.len());
        // Check for expired timers
        let now = Instant::now();
        while let Some(timer) = self.timers.peek() {
            if timer.deadline <= now {
                let timer = self.timers.pop().unwrap();
                // Move timer callback to ready queue (or return immediately as a task)
                // Here we'll push to ready queue to maintain order with other tasks
                self.ready_queue.push_back(Task {
                    callback: timer.callback,
                    args: timer.args,
                    promise_id: None, // Timers generally don't resolve a promise directly in this model,
                                      // or the callback is a wrapper that does.
                });
            } else {
                break;
            }
        }

        self.ready_queue.pop_front()
    }

    pub fn next_timeout(&self) -> Option<Duration> {
        self.timers.peek().map(|t| {
            let now = Instant::now();
            if t.deadline > now {
                t.deadline - now
            } else {
                Duration::from_millis(0)
            }
        })
    }
}
