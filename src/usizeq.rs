use crate::mpmcq::MySender;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Condvar};

struct MyAtomicUsize {
    val: AtomicUsize,
    pad1: usize,
    pad2: usize,
    pad3: usize,
    pad4: [usize; 4],
}

impl MyAtomicUsize {
    pub fn new(v : usize) -> MyAtomicUsize {
        MyAtomicUsize{
            val : AtomicUsize::new(v),
            pad1: 0,
            pad2: 0,
            pad3: 0,
            pad4: [0;4]
        }
    }
    pub fn get_val(&self) -> usize {
        unsafe {
            let p = &self.val as *const AtomicUsize as usize as *const usize;
            *p
        }
    }
    pub fn fetch(&self) -> usize {
        self.val.load(Ordering::SeqCst)
    }
    pub fn load(&self) -> usize {
        self.val.load(Ordering::SeqCst)
    }
    pub fn store(&self, new : usize) {
        self.val.store(new, Ordering::SeqCst)
    }
    pub fn swap(&self, new : usize) -> usize {
        self.val.swap(new, Ordering::SeqCst)
    }
    pub fn fetch_add(&self, delta : usize) -> usize {
        self.val.fetch_add(delta, Ordering::SeqCst)
    }
    pub fn fetch_sub(&self, delta : usize) -> usize {
        self.val.fetch_sub(delta, Ordering::SeqCst)
    }
    pub fn compare_and_swap(&self, old : usize, new : usize) -> usize {
        self.val.compare_and_swap(old, new, Ordering::SeqCst)
    }
}

struct Position {
    val: usize,
    maxv: usize,
    pad2: usize,
    pad3: usize,
    pad4: [usize; 4],
}
impl Position {
    pub fn new(val : usize, max : usize) -> Position {
        Position {
            val,
            maxv: max,
            pad2: 0,
            pad3: 0,
            pad4: [0;4]
        }
    }
    pub fn get(&self) -> usize {self.val}
    pub fn next(&self) -> usize {
        unsafe {
            let p = &self.val as *const usize as *mut usize;
            *p += 1;
            if *p >= self.maxv {
                *p = 0;
            }
        }
        self.val
    }
    pub fn get_and_inc(&self) -> usize {
        let current = self.val;
        self.next();
        current
    }
}

const QUEUE_SIZE : usize = 100000;

pub(crate) struct UsizeQ {
    count : MyAtomicUsize,
    in_pos : Position,
    out_pos : Position,
    cap : usize,
    buf : [usize; QUEUE_SIZE],
    sem_room : (Mutex<()>, Condvar),
    sem_elem : (Mutex<()>, Condvar),
}
impl UsizeQ {
    pub fn new() -> UsizeQ {
        UsizeQ{
            count: MyAtomicUsize::new(0),
            in_pos: Position::new(0, QUEUE_SIZE),
            out_pos: Position::new(0, QUEUE_SIZE),
            cap: QUEUE_SIZE,
            buf: [0; QUEUE_SIZE],
            sem_room: (Mutex::new(()), Default::default()),
            sem_elem: (Mutex::new(()), Default::default())
        }
    }
    pub fn push(&self, e : usize) {
        let mut g = self.sem_room.0.lock().unwrap();
        while self.count.get_val() == self.cap {
            g = self.sem_room.1.wait(g).unwrap();
        }

        let i = self.in_pos.get_and_inc();
        unsafe{
            *(&self.buf[i] as *const usize as *mut usize) = e;
        }

        let c = self.count.fetch_add(1);
        if c+1 < self.cap {
            self.sem_room.1.notify_one();
        }
        drop(g);

        if c == 0 { // maybe consumer is waiting
            let _g = self.sem_elem.0.lock().unwrap();
            self.sem_elem.1.notify_one();
            drop(_g);
        }
    }
    pub fn pop(&self) -> usize {
        let mut g = self.sem_elem.0.lock().unwrap();
        while self.count.get_val() == 0 {
            g = self.sem_elem.1.wait(g).unwrap();
        }

        let i = self.out_pos.get_and_inc();
        let e = self.buf[i];

        let c = self.count.fetch_sub(1);
        if c > 1 {
            self.sem_elem.1.notify_one();
        }
        drop(g);

        if c == self.cap { // maybe producer is waiting
            let _g = self.sem_room.0.lock().unwrap();
            self.sem_room.1.notify_one();
            drop(_g);
        }

        return e;
    }
}