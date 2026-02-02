use mio::{Events, Poll, Token};
use pleno_value::Value;
use std::collections::{HashMap, VecDeque};
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

pub struct IoHandler {
    pub callback: Value,
    pub persistent: bool,
}

pub struct EventLoop {
    pub timers: std::collections::BinaryHeap<Timer>,
    pub ready_queue: VecDeque<Task>,
    pub next_timer_id: u64,
    pub poll: Poll,
    pub events: Events,
    pub io_handlers: HashMap<Token, IoHandler>,
    pub next_token: usize,
}

#[derive(Debug)]
pub struct Task {
    pub callback: Value,
    pub args: Vec<Value>,
    pub promise_id: Option<u64>,
}

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLoop {
    pub fn new() -> Self {
        Self {
            timers: std::collections::BinaryHeap::new(),
            ready_queue: VecDeque::new(),
            next_timer_id: 1,
            poll: Poll::new().expect("Failed to create Poll"),
            events: Events::with_capacity(1024),
            io_handlers: HashMap::new(),
            next_token: 1,
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
        self.timers.is_empty() && self.ready_queue.is_empty() && self.io_handlers.is_empty()
    }

    pub fn register_io(
        &mut self,
        source: &mut dyn mio::event::Source,
        interest: mio::Interest,
        callback: Value,
        persistent: bool,
    ) -> Result<Token, std::io::Error> {
        let token = Token(self.next_token);
        self.next_token += 1;
        self.poll.registry().register(source, token, interest)?;
        self.io_handlers.insert(
            token,
            IoHandler {
                callback,
                persistent,
            },
        );
        Ok(token)
    }

    pub fn unregister_io(
        &mut self,
        source: &mut dyn mio::event::Source,
        token: Token,
    ) -> Result<(), std::io::Error> {
        self.poll.registry().deregister(source)?;
        self.io_handlers.remove(&token);
        Ok(())
    }

    pub fn tick(&mut self) -> Option<Task> {
        loop {
            // 1. Process ready queue
            if let Some(task) = self.ready_queue.pop_front() {
                return Some(task);
            }

            // 2. Check for expired timers
            let now = Instant::now();
            while let Some(timer) = self.timers.peek() {
                if timer.deadline <= now {
                    let timer = self.timers.pop().unwrap();
                    self.ready_queue.push_back(Task {
                        callback: timer.callback.clone(),
                        args: timer.args.clone(),
                        promise_id: None,
                    });
                } else {
                    break;
                }
            }

            if let Some(task) = self.ready_queue.pop_front() {
                return Some(task);
            }

            // 3. If no ready tasks and nothing to wait for, return None
            let timeout = self.next_timeout();
            if self.io_handlers.is_empty() && timeout.is_none() {
                return None;
            }

            // 4. Poll MIO
            let result = self.poll.poll(&mut self.events, timeout);

            if let Ok(()) = result {
                for event in self.events.iter() {
                    let token = event.token();
                    if let Some(handler) = self.io_handlers.get(&token) {
                        self.ready_queue.push_back(Task {
                            callback: handler.callback.clone(),
                            args: vec![],
                            promise_id: None,
                        });
                    }
                }
            }

            // If we have items now, the next iteration will return them
            if !self.ready_queue.is_empty() {
                continue;
            }

            // If poll returned because of timeout, the next iteration will process the timer
            if timeout.is_some() {
                continue;
            }

            // If we reach here and queue is empty, and we didn't have a timeout,
            // it means poll finished for some other reason (e.g. error or empty event list)
            // If it's empty, we might want to return None to avoid busy loop if no progress is possible.
            if self.ready_queue.is_empty() {
                return None;
            }
        }
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
