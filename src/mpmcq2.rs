///
/// my MPMC queue
///

use std::sync::atomic::{AtomicIsize, AtomicI32, Ordering, AtomicBool};
use std::mem::swap;
use std::sync::{MutexGuard, Condvar, Mutex, Arc};
use crate::QueueType::MyFastQueue;

///
/// my condition
///
struct MyCond{
    lock : Mutex<()>,
    cond : Condvar,
}

impl MyCond{
    ///
    /// create a MyCondition
    ///
    pub fn new() -> MyCond {
        MyCond{ lock: Mutex::new(()), cond: Default::default()}
    }

    pub fn lock(&self) -> MutexGuard<()> {
        self.lock.lock().unwrap()
    }

    pub fn unlock(&self, lock : MutexGuard<()>) { }

    pub fn wait<'a>(&self, lock: MutexGuard<'a, ()>) -> MutexGuard<'a, ()> {
        self.cond.wait(lock).unwrap()
    }

    pub fn signal(&self) {
        self.cond.notify_one();
    }

    pub fn broadcast(&self) {
        self.cond.notify_all();
    }
}

#[derive(Debug)]
pub enum QueueErr {
    SendEndClosed(&'static str),
    ReceiveEndClosed(&'static str),
    QueueFull(&'static str),
    QueueEmpty(&'static str),
    Timeout(&'static str),
    SendTimeout(&'static str),
    ReceiveTimeout(&'static str),
}

const ErrSendEndClosed : QueueErr = QueueErr::SendEndClosed("queue's send end has been closed");
const ErrReceiveEndClosed : QueueErr = QueueErr::ReceiveEndClosed("queue's receive end has been closed");
const ErrTimeout : QueueErr = QueueErr::Timeout("send or receive timeout");
const ErrSendTimeout : QueueErr = QueueErr::SendTimeout("send timeout");
const ErrReceiveTimeout : QueueErr = QueueErr::ReceiveTimeout("receive timeout");

struct Event {
    l : Mutex<isize>,
    c : Condvar,
}
impl Event {
    pub fn new(n: isize) -> Event {
        Event{ l: Mutex::new(n), c: Default::default() }
    }
    pub fn wait(&self) {
        let mut g = self.l.lock().unwrap();
        *g -= 1;
        if *g < 0 {
            g = self.c.wait(g).unwrap();
        }
    }
    pub fn notify(&self) {
        let mut g = self.l.lock().unwrap();
        *g += 1;
        self.c.notify_one();
    }
    pub fn notify_all(&self) {
        let mut g = self.l.lock().unwrap();
        *g += 1;
        self.c.notify_all();
    }
}

struct Indexer {
    i : Mutex<isize>,
    cap : isize,
}
impl Indexer {
    pub fn new(start : isize, cap : isize) -> Indexer {
        Indexer{ i: Mutex::new(start), cap }
    }
    pub fn next(&self) -> isize {
        let mut g = self.i.lock().unwrap();
        let mut next_i = *g + 1;
        if next_i >= self.cap {
            next_i = 0;
        }
        *g = next_i;
        next_i
    }
}

struct FQueue<T> {
    sendClosed : AtomicBool,
    recvClosed : AtomicBool,
    buf : Vec<Option<T>>,
    count : AtomicIsize,
    room: AtomicIsize,
    capacity : isize,
    inIdx : Indexer,
    outIdx : Indexer,
    condRoom : Event,
    condElem : Event,
}
//unsafe impl<T: Send> Send for FQueue<T> { }
//unsafe impl<T: Send> Sync for FQueue<T> { }

impl<T> FQueue<T> {
    pub fn new(cap : isize) -> FQueue<T> {
        assert!(cap>0);
        let mut q = FQueue {
            sendClosed : AtomicBool::new(false),
            recvClosed : AtomicBool::new(false),
            buf: Vec::<Option<T>>::with_capacity(cap as usize),
            count: AtomicIsize::new(0),
            room: AtomicIsize::new(cap),
            capacity: cap,
            inIdx: Indexer::new(-1, cap),
            outIdx: Indexer::new(-1, cap),
            condRoom: Event::new(cap),
            condElem: Event::new(0),
        };
        for i in 0..cap {
            q.buf.push(None);
        }
        return q;
    }

    fn notify_has_room(&self) {
        self.condRoom.notify();
    }

    fn notify_has_elem(&self) {
        self.condElem.notify();
    }

    fn notify_all(&self) {
        self.condRoom.notify_all();
        self.condElem.notify_all();
    }

    pub fn en_queue(&self, e : T) -> Result<(), QueueErr>{
        if self.sendClosed.load(Ordering::SeqCst) {
            //panic!("queue broken");
            return Err(ErrSendEndClosed);
        }

        // sub room
        let room = self.room.fetch_sub(1, Ordering::SeqCst);
        if room <= 0 {
            if self.recvClosed.load(Ordering::SeqCst) {
                return Err(ErrReceiveEndClosed);
            }
            self.condRoom.wait();
            if self.sendClosed.load(Ordering::SeqCst) {
                return Err(ErrSendEndClosed);
            }
        }

        // has room, put element into queue
        let i = self.inIdx.next() as usize;
        unsafe {
            let addr = &self.buf[i] as *const Option<T> as *mut Option<T>;
            swap(&mut Some(e), &mut *addr);
        }

        // inc element count
        let count = self.count.fetch_add(1, Ordering::SeqCst);

        if count < 0 {
            // no element before I putting, maybe someone wait, notify them.
            self.notify_has_elem();
        }

        return Ok(());
    }

    pub fn de_queue(&self) -> Result<T, QueueErr> {
        if self.recvClosed.load(Ordering::SeqCst) {
            return Err(ErrReceiveEndClosed);
        }

        // sub elem
        let count = self.count.fetch_sub(1, Ordering::SeqCst);
        if count <= 0 {
            if self.sendClosed.load(Ordering::SeqCst) {
                return Err(ErrSendEndClosed);
            }
            self.condElem.wait();
            if self.recvClosed.load(Ordering::SeqCst) {
                return Err(ErrReceiveEndClosed);
            }
        }

        // has element, take it
        let mut e : Option<T> = None;
        let i = self.outIdx.next() as usize;
        unsafe {
            swap(&mut e, &mut *(&self.buf[i] as *const Option<T> as *mut Option<T>));
        }

        // add room
        let room = self.room.fetch_add(1, Ordering::SeqCst);

        if room < 0 {
            // before I take, there is no room, maybe someone wait for room, notify them.
            self.notify_has_room();
        }

        match e {
            Some(v) => return Ok(v),
            None => panic!("pop None from queue! queue is corrupt"),
        };
    }

    pub fn close_send(&self) -> bool {
        let hasClosed = self.sendClosed.swap(true, Ordering::SeqCst);
        if hasClosed {
            return false;
        }
        self.notify_all();
        return true;
    }

    pub fn close_recv(&self) -> bool {
        let hasClosed = self.recvClosed.swap(true, Ordering::SeqCst);
        if hasClosed {
            return false;
        }
        self.notify_all();
        return true;
    }

    pub fn close_all(&self) {
        self.close_send();
        self.close_recv();
    }
}

const DEFAULT_QUEUE_SIZE : usize = 4096;

pub struct MyQueue<T> {
    __queue : Arc<FQueue<T>>,
}
impl<T> Clone for MyQueue<T> {
    fn clone(&self) -> Self {
        MyQueue{__queue: self.__queue.clone()}
    }
}
impl<T> MyQueue<T> {
    fn new() -> MyQueue<T> {
        MyQueue{ __queue: Arc::new(FQueue::new(DEFAULT_QUEUE_SIZE as isize)) }
    }
    fn new_with_capacity(cap : usize) -> MyQueue<T> {
        MyQueue{ __queue: Arc::new(FQueue::new(cap as isize)) }
    }

    pub fn push(&self, e : T) -> Result<(), QueueErr> {
        self.__queue.en_queue(e)
    }

    pub fn pop(&self) -> Result<T, QueueErr> {
        self.__queue.de_queue()
    }

    pub fn close_send(&self) -> bool {
        self.__queue.close_send()
    }

    pub fn close_receive(&self) -> bool {
        self.__queue.close_recv()
    }

    pub fn close(&self) {
        self.__queue.close_all();
    }
}

pub fn new_share_queue<T>() -> MyQueue<T> {
    MyQueue::new()
}

pub fn new_share_queue_capacity<T>(cap : usize) -> MyQueue<T> {
    MyQueue::new_with_capacity(cap)
}

///
/// queue sender
///
pub struct MySender<T> {
    count : Arc<AtomicI32>,
    __queue : MyQueue<T>,
}
impl<T> Clone for MySender<T> {
    fn clone(&self) -> Self {
        self.count.fetch_add(1, Ordering::SeqCst);
        //println!("clone sender: {:x}, {}", self as *const Self as usize, self.count.load(Ordering::SeqCst));
        MySender{count: self.count.clone(), __queue: self.__queue.clone()}
    }
}
impl<T> Drop for MySender<T> {
    fn drop(&mut self) {
        let n = self.count.fetch_sub(1, Ordering::SeqCst);
        if n==1 {
            self.__queue.close_send();
        }
        //println!("drop sender: {:x}, {}", self as *const Self as usize, n);
    }
}
impl<T> MySender<T> {
    pub fn push(&self, e : T) -> Result<(), QueueErr> {
        self.__queue.push(e)
    }
}

///
/// queue receiver
///
pub struct MyReceiver<T> {
    count : Arc<AtomicI32>,
    __queue : MyQueue<T>,
}
impl<T> Clone for MyReceiver<T> {
    fn clone(&self) -> Self {
        (*self.count).fetch_add(1, Ordering::SeqCst);
        MyReceiver{count: self.count.clone(), __queue: self.__queue.clone()}
    }
}
impl<T> Drop for MyReceiver<T> {
    fn drop(&mut self) {
        let n = (*self.count).fetch_sub(1, Ordering::SeqCst);
        if n==1 {
            self.__queue.close_receive();
        }
        // println!("drop receiver: {}", n);
    }
}
impl<T> MyReceiver<T> {
    pub fn pop(&self) -> Result<T, QueueErr> {
        self.__queue.pop()
    }
}

pub fn new_mpmc<T>() -> (MySender<T>, MyReceiver<T>) {
    let q = MyQueue::<T>::new();
    let nSender = Arc::new(AtomicI32::new(1));
    let nReceiver = Arc::new(AtomicI32::new(1));
    (MySender { count: nSender, __queue: q.clone() }, MyReceiver { count: nReceiver, __queue: q })
}
pub fn new_mpmc_capacity<T>(cap : usize) -> (MySender<T>, MyReceiver<T>) {
    let q = MyQueue::<T>::new_with_capacity(cap);
    let nSender = Arc::new(AtomicI32::new(1));
    let nReceiver = Arc::new(AtomicI32::new(1));
    (MySender { count: nSender, __queue: q.clone() }, MyReceiver { count: nReceiver, __queue: q })
}

#[cfg(test)]
mod tests{
    use crate::mpmcq2;
    use std::thread;
    use crate::mpmcq2::{new_mpmc, new_mpmc_capacity};

    struct S{ a: i32, b:i32}

    #[test]
    fn test_share_queue() {
        let q = mpmcq2::new_share_queue_capacity::<S>(4);
        let qq = q.clone();
        thread::spawn(move ||{
            for i in 0..10 {
                qq.push(S{a:i,b:i});
                println!("send {}", i);
            }
        });

        for i in 0..10 {
            let v = q.pop().unwrap_or(S{a:9999,b:9999});
            assert_eq!(v.a, i);
            println!("--recv {}", v.a);
        }
    }

    #[test]
    fn test_correct() {
        let q = mpmcq2::new_share_queue_capacity::<i32>(1000);

        let (rs1_send, rs1_recv) = std::sync::mpsc::channel::<i32>();
        let (rs2_send, rs2_recv) = std::sync::mpsc::channel::<i32>();

        // recv
        for i in 0..10 {
            let rq = q.clone();
            let rs1 = rs1_send.clone();
            thread::spawn(move || {
                let mut sum = 0;
                for k in 0..200000 {
                    let v = rq.pop();
                    match v {
                        Ok(x) => sum += x,
                        Err(err) => panic!("recv panic"),
                    }
                }
                rs1.send(sum);
            });
        }

        // send
        for i in 0..20 {
            let sq = q.clone();
            let rs2 = rs1_send.clone();
            thread::spawn(move || {
                let mut sum = 0;
                for k in 0..100000 {
                    if let Err(err) = sq.push(k) {
                        panic!("send panic")
                    }
                }
                rs2.send(sum);
            });
        }

        // wait send
        let mut sum_send = 0;
        for i in 0..20 {
            let v = rs2_recv.recv().unwrap();
            sum_send += v;
        }

        // wait recv
        let mut sum_recv = 0;
        for i in 0..10{
            let v = rs1_recv.recv().unwrap();
            sum_recv += v;
        }

        assert_eq!(sum_recv, sum_send);
    }

    #[test]
    fn test_mpmc() {
        let (send_q, recv_q) = new_mpmc_capacity::<S>(4);
        let sub = thread::spawn(move || {
            for i in 0..10 {
                match recv_q.pop() {
                    Ok(v) => println!("  recv {}", v.a),
                    Err(e) => {
                        println!("  recv err: {:?}", e);
                        break
                    },
                }
            }
        });

        for i in 0..8 {
            send_q.push(S{a:i, b:i});
            println!("send {}", i);
        }
        drop(send_q);

        sub.join();
    }
}
