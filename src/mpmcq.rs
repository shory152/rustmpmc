///
/// my MPMC queue
///

use std::sync::atomic::{AtomicUsize, AtomicI32, Ordering, AtomicBool};
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

struct FQueue<T> {
    sendClosed : AtomicBool,
    recvClosed : AtomicBool,
    buf : Vec<Option<T>>,
    count : AtomicUsize,
    capacity : usize,
    inIdx : usize,
    outIdx : usize,
    condRoom : MyCond,
    condElem : MyCond,
}
//unsafe impl<T: Send> Send for FQueue<T> { }
//unsafe impl<T: Send> Sync for FQueue<T> { }

impl<T> FQueue<T> {
    pub fn new(cap : usize) -> FQueue<T> {
        let mut q = FQueue {
            sendClosed : AtomicBool::new(false),
            recvClosed : AtomicBool::new(false),
            buf: Vec::<Option<T>>::with_capacity(cap),
            count: AtomicUsize::new(0),
            capacity: cap,
            inIdx: 0,
            outIdx: 0,
            condRoom: MyCond::new(),
            condElem: MyCond::new(),
        };
        for i in 0..cap {
            q.buf.push(None);
        }
        return q;
    }

    fn notify_has_room(&self) {
        let g = self.condRoom.lock();
        self.condRoom.signal();
    }

    fn notify_has_elem(&self) {
        let g = self.condElem.lock();
        self.condElem.signal();
    }

    fn notify_all(&self) {
        let g = self.condRoom.lock();
        self.condRoom.broadcast();
        drop(g);

        let g = self.condElem.lock();
        self.condElem.broadcast();
        drop(g);
    }

    pub fn en_queue(&self, e : T) -> Result<(), QueueErr>{
        if self.sendClosed.load(Ordering::SeqCst) {
            //panic!("queue broken");
            return Err(ErrSendEndClosed);
        }

        let mut g = self.condRoom.lock();
        while self.count.load(Ordering::SeqCst) == self.capacity {
            if self.recvClosed.load(Ordering::SeqCst) {
                return Err(ErrReceiveEndClosed);
            }
            // no room, queue is full, wait ...
            g = self.condRoom.wait(g);
            if self.sendClosed.load(Ordering::SeqCst) {
                //panic!("queue broken");
                return Err(ErrSendEndClosed);
            }
        }

        // has room, put element into queue
        unsafe {
            let mut nextIn = self.inIdx + 1;
            if nextIn >= self.capacity {
                nextIn = 0;
            }
            *(&self.inIdx as *const usize as *mut usize) = nextIn;

            let addr = &self.buf[self.inIdx] as *const Option<T> as *mut Option<T>;
            swap(&mut Some(e), &mut *addr);
        }


        // inc element count
        let c = self.count.fetch_add(1, Ordering::SeqCst);
        if c + 1 < self.capacity {
            // has more room after I put, notify others to enqueue
            self.condRoom.signal();
        }
        self.condRoom.unlock(g);

        if c == 0 {
            // no element before I putting, maybe someone wait, notify them.
            self.notify_has_elem();
        }

        return Ok(());
    }

    pub fn de_queue(&self) -> Result<T, QueueErr> {
        if self.recvClosed.load(Ordering::SeqCst) {
            return Err(ErrReceiveEndClosed);
        }
        let mut g = self.condElem.lock();
        while self.count.load(Ordering::SeqCst) == 0 {
            if self.sendClosed.load(Ordering::SeqCst) {
                return Err(ErrSendEndClosed);
            }
            // no element, wait...
            g = self.condElem.wait(g);
            if self.recvClosed.load(Ordering::SeqCst) {
                return Err(ErrReceiveEndClosed);
            }
        }

        // has element, take it
        let mut e : Option<T> = None;
        unsafe {
            let mut nextOut = self.outIdx + 1;
            if nextOut >= self.capacity {
                nextOut = 0;
            }
            *(&self.outIdx as *const usize as *mut usize) = nextOut;

            swap(&mut e, &mut *(&self.buf[self.outIdx] as *const Option<T> as *mut Option<T>));
        }

        // dec element count
        let c = self.count.fetch_sub(1, Ordering::SeqCst);
        if c > 1 {
            // before I take, there are more elements, notify other to take
            self.condElem.signal();
        }
        self.condElem.unlock(g);

        if c == self.capacity {
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
        MyQueue{ __queue: Arc::new(FQueue::new(DEFAULT_QUEUE_SIZE)) }
    }
    fn new_with_capacity(cap : usize) -> MyQueue<T> {
        MyQueue{ __queue: Arc::new(FQueue::new(cap)) }
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
    use crate::mpmcq;
    use std::thread;
    use crate::mpmcq::{new_mpmc, new_mpmc_capacity};

    struct S{ a: i32, b:i32}

    #[test]
    fn test_share_queue() {
        let q = mpmcq::new_share_queue_capacity::<S>(4);
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
